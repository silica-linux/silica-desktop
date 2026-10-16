# M2 · Glass

How Ossido draws the glass of `silica-glass-v1` (`docs/protocols.md`), where the parameters
come from and what it costs. Code in `crates/ossido-render/src/glass.rs`,
`crates/ossido-render/src/glass_profile.rs` and in the shaders `blur.frag` and
`material.frag`. Since UI 1.0.0 "Amalfi" the parameters are the Amalfi materials
([`design/amalfi/materials.md`](design/amalfi/materials.md)); the measurements of the
reference glass the earlier materials were fitted on stay in [`spec/glass.md`](spec/glass.md).

## How it works

For every surface with glass regions, on every frame in which what lies behind it changes:

1. the elements behind the surface (and the desktop color) are redrawn into an offscreen
   texture as large as the regions plus 3σ;
2. the texture is blurred with a separable Gaussian, at the fraction of the resolution the
   kind was validated at (below), and mixed with the kind's sharp share;
3. every group of regions is drawn with the material shader, in the order of
   `materials.md`: refraction at the edge (the backdrop sample is bent over a band along the
   shape's edge, with a small chromatic split), saturation and brightness, tint fill, inside
   rim, rim light on the top inner edge, the drop shadow outside the shape (only when the
   client asks Ossido to draw it), circular-arc corners and the smooth union of nearby regions
   of the same kind.

If nothing changes behind (same elements, same commits, same geometry) the textures are
reused. A window's glass does not see its own bar or its own shadow. Glass is never stacked on
glass by the shell (items on a glass surface are plain fills), and Ossido never animates a
blur radius: σ is fixed per kind; a fading panel changes only the glass's opacity, which
reuses the blurred texture.

## Materials

| | Glass/Regular | Glass/Thick |
| --- | --- | --- |
| Blur | σ 6 pt | σ 10 pt |
| Refraction: distortion / depth / chromatic aberration | 0.30 / 0.35 / 0.03 | 0.22 / 0.28 / 0.015 |
| Saturation | 1.1 | 1.1 |
| Backdrop brightness, light / dark | ×1.25 / ×1.10 | ×1.12 / ×1.05 |
| Tint, light / dark | `#FFFFFF` 60 % / `#2A2B30` 60 % | `#FFFFFF` 58 % / `#2A2B30` 58 % |
| Inside rim (1 pt) | `#000000` 12 % / 40 % | same |
| Rim light (inner shadow 0, 1, blur 1 → σ 0.5 pt) | `#FFFFFF` 70 % / 22 % | same |
| Drop shadow | `shadows.2`: y 4, σ 6 pt, black 12 % / 32 % | `shadows.3`: y 12, σ 16 pt, black 18 % / 36 % |

The kit's background blur radius is read as the σ of a CSS `blur()`; a shadow's blur as a CSS
`box-shadow` blur (σ = blur / 2). Glass/Clear (the lens over a pressed or dragged control, σ
2 + refraction) is drawn by `silica_ui` over the control; its parameters are in
`glass_profile.rs` too, but no kind of surface uses it in Ossido.

### Refraction

Regular and Thick are real glass: the shape bends the backdrop at its edge. It is an edge
effect, not a magnifier: the lens code path of the shader (the `(t, s)` curve: a pixel at distance
t inside the edge samples the backdrop at distance s along the outward normal, the normal taken
on the SDF of the union of the regions, so capsules, smooth-cornered rectangles and tips all
work) drives it with these parameters:

- the band is `depth × half the shorter side` of the region, at most 24 pt (`REFRACTION_BAND_PT`;
  past it the pixel samples its own place);
- inside the band the pixel samples `distortion × band × (1 − t / band)²` closer to the edge
  (s = t − that), so the backdrop is pulled in towards the rim and the centre is untouched;
- the red and blue channels sample ±`chromatic × band × (1 − t / band)` further along the same
  normal than green: a split of a fraction of a pixel to a pixel at the rim, none in the centre;
- it samples the blurred backdrop at full resolution of the glass texture (after the upsample
  of a reduced blur), so the blur-scale equivalence test covers it as part of the glass;
- the texture margin grows by the outward reach (`distortion × 24 pt`, and the split), so the
  backdrop beyond the edge exists; the cache key is unchanged (the lens is part of the material
  compared by `same_shape`, the blurred texture still depends only on σ, sharp share and factor).

The specular highlight on the rim is the rim light of the table (inner shadow, top inner edge)
plus the 1 pt inside rim. The reduced-transparency (`opaque`) quality draws neither the
refraction nor the brightness: the fallback is unchanged.

## Kinds of surface

Every region belongs to a kind (`set_profile`, or the default of its declared material) and
the kind picks the material ("Where each material goes" in `materials.md`). The radius is the
region's own: the shell's surfaces send it (and the tips' outlines), Ossido hardcodes none.

| Kind | Shell surfaces | Material | Sharp share | Blur resolution below 2× / from 2× |
| --- | --- | --- | ---: | --- |
| `menu` (default of `thick`) | menus, context menus, submenus, status menus | Thick | 0 | 1/8 / 1/8 |
| `popover` | popovers | Thick | 0 | 1/8 / 1/8 |
| `panel` | Faro, the launcher, Dock stacks, Control Center detail views | Thick | 0 | 1/8 / 1/8 |
| `module` (default of `regular`) | Control Center modules (and today the HUDs) | Thick | 0.04 | 1/8 / 1/8 |
| `tooltip` | tooltips, the Dock's labels | Thick | 0 | 1/8 / 1/8 |
| `sheet` | sheets, system alerts (the Trash alert, authentication) | Thick | 0 | 1/8 / 1/8 |
| `dock` (default of `clear`) | the Dock, the app switcher | Regular | 0.28 | 1/2 / 1/4 |
| `control` | toolbar controls, the Screenshot bar, small controls | Regular | 0.04 | 1/2 / 1/4 |
| `banner` (default of `notification`) | notification banners and cards | Regular | 0.03 light, 0.02 dark | 1/2 / 1/4 |

Not glass in Ossido:

- **The menu bar** requests no glass: it is a transparent strip on the wallpaper (Amalfi:
  "none"). Ossido keeps only the gradient that compresses the wallpaper's highlights under it
  (`backdrop.rs`, measured in `spec/glass.md` › Menu bar gradient), which is not a material.
- **The Panorama spaces bar** is Glass/Thick (design decision of the Amalfi session): the
  wallpaper under it, and only that, goes through the Thick material (σ 10, saturation 1.1, tint
  58 %, refraction, inside rim, rim light) via the ordinary glass pass (`panorama::bar_region`). The bar's
  own shadow below it stays; the wallpaper behind Mission Control is not dimmed or blurred.

### The sharp share

On top of the spec Ossido mixes a share of the sharp backdrop into the blurred one (`out = w ·
sharp + (1 − w) · blur`, before the saturation): the glass "blurs a little and keeps part of
the backdrop sharp". It is one parameter per kind's material (`MaterialParams::sharp`, set by
`GlassKind::sharp`) and keeps, for now, the value each kind had before Amalfi. It will be tuned
against the kit's exported images of each material, which are not exported yet; until then
these values are provisional.

### Blur resolution (#204)

Each kind blurs at a fraction of the resolution (`BlurScale`): the source is averaged over
f × f pixels, blurred there with the σ that gives the same total spread, drawn back at full
size. The factor is the one the equivalence test passes with
(`ossido::glass_equivalence::every_kind_blurs_at_its_scale_like_at_full_resolution`; the
ignored `table_of_factors` prints every factor): over wallpapers, text and sharp edges, light
and dark, the reduced glass may differ from the full-resolution one by at most 1/255 on
average and 3/255 at any pixel. The test draws the glass with its refraction, on both sides, so
the edge band is part of what is compared: the refraction samples the blurred texture after it is
drawn back at full size, a displacement of a few pixels over an image that is already smooth, and
the table shows no change from it (worst case over the backdrops, mean / max levels):

| Material | 1× | 1.5× | 2× |
| --- | --- | --- | --- |
| Thick (σ 10 pt), f8 (f4 at 1×) | 0.33–0.39 / 2 | 0.09–0.45 / 2 | 0.10–0.46 / 2 |
| Regular (σ 6 pt) | f2: 0.26–0.40 / 2 | f2: 0.18–0.30 / 2 | f4: 0.18–0.37 / 2 |

At 1× Glass/Thick cannot go to an eighth (σ 10 px would be 1.15 px reduced, under the 1.5 px
floor of a reduced blur): `effective_factor` halves it to a quarter. Glass/Regular cannot go below half resolution at 1× (f4 would take σ 6 px under the same floor).

## Quality levels

| Level | What the glass sees | When |
| --- | --- | --- |
| `full` | everything behind it | default with a GPU |
| `frosted` | only the desktop wallpaper (background and bottom layers): rarely recomputed | default on a software rasterizer (llvmpipe) |
| `opaque` | nothing: the Amalfi fallback, the tint over `Surface/Window` (`#FFFFFF` / `#1B1C1F`), rims and shadow kept | reduced transparency, on request |

`--glass auto|full|frosted|opaque` at startup, `ui.glass_quality` during the session.

## Cost (2026-10-13, before Amalfi)

`cargo xtask bench glass`: Ossido nested in GNOME on Ubuntu 26.04, Iris Xe (Raptor
Lake-P), 1280×800 output at scale 1. The Flutter Dock (Impeller, clear glass) sits on a
top layer; behind it a window scrolls on every frame, so the glass background always
changes; the pointer passes over the icons. 5 s per run, median of 3.

| Glass | Ossido render: mean | p95 | p99 | max | Dock: frames | p99 | slow |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| full | 1.88 ms | 2.84 | 3.24 | 3.4 | 298 | 20.0 ms | 1 |
| frosted | 1.58 ms | 2.30 | 3.02 | 3.2 | 299 | 21.5 ms | 1 |
| opaque | 1.51 ms | 2.17 | 2.99 | 3.2 | 299 | 20.7 ms | 1 |

Full glass with the background changing on every frame costs about 0.4 ms more per frame out
of 16.7: the Dock stays at 60 fps. The measurement is the rendering time on the CPU
(building the elements, GL commands, submit), not the GPU time: that needs OpenGL timer
queries, not yet in Ossido. The Amalfi materials blur the thick kinds at an eighth of the
resolution and skip the measured edge in the shader, so they cost less; not re-measured with
`bench glass` since.

### Refraction (2026-10-16)

`cargo test -p ossido --release glass_cost -- --ignored --nocapture` (software GL, llvmpipe: a
600 × 400 pt glass over a 960 × 600 pt photo-like backdrop, made from scratch every time, median
of 15, mean over the kinds of a material; it prints the same glass without the refraction too).
It is noisy (±0.5 ms between runs at 1×, more at 2×), so only the order of magnitude counts:

| Material, scale | Before (σ 16 / σ 6, no refraction) | After, with refraction | After, same run without it |
| --- | ---: | ---: | ---: |
| Thick 1× | 1.3 ms | 0.6–1.4 ms | 0.6–1.2 ms |
| Thick 2× | 4.5–9 ms | 1.7–2.2 ms | 1.7–2.4 ms |
| Regular 1× | 0.5–0.8 ms | 0.6–1.6 ms | 0.6–1.7 ms |
| Regular 2× | 6–6.6 ms | 2.3–2.8 ms | 2.1–3.0 ms |

Refraction (a normal from four distance evaluations and three backdrop samples in the band's
pixels instead of one) is below the noise of the measurement on software GL; Thick got cheaper
because σ 10 blurs at a quarter or an eighth of the resolution with a smaller kernel than σ 16.
