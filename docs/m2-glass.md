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
   `materials.md`: saturation, tint fill, inside rim, rim light on the top inner edge, the drop
   shadow outside the shape (only when the client asks Ossido to draw it), circular-arc corners
   and the smooth union of nearby regions of the same kind.

If nothing changes behind (same elements, same commits, same geometry) the textures are
reused. A window's glass does not see its own bar or its own shadow. Glass is never stacked on
glass by the shell (items on a glass surface are plain fills), and Ossido never animates a
blur radius: σ is fixed per kind; a fading panel changes only the glass's opacity, which
reuses the blurred texture.

## Materials

| | Glass/Regular | Glass/Thick |
| --- | --- | --- |
| Blur | σ 6 pt | σ 16 pt |
| Saturation | 1.5 | 1.5 |
| Tint, light / dark | `#FFFFFF` 60 % / `#2A2B30` 60 % | `#FFFFFF` 76 % / `#2A2B30` 76 % |
| Inside rim (1 pt) | `#000000` 12 % / 40 % | same |
| Rim light (inner shadow 0, 1, blur 1 → σ 0.5 pt) | `#FFFFFF` 70 % / 22 % | same |
| Drop shadow | `shadows.2`: y 4, σ 6 pt, black 12 % / 32 % | `shadows.3`: y 12, σ 16 pt, black 18 % / 36 % |
| Lens | none | none |

The kit's background blur radius is read as the σ of a CSS `blur()`; a shadow's blur as a CSS
`box-shadow` blur (σ = blur / 2). Glass/Clear (the lens over a pressed or dragged control, σ
2 + refraction) is drawn by `silica_ui` over the control; its parameters are in
`glass_profile.rs` too, but no kind of surface uses it in Ossido.

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
- **The Panorama spaces bar** keeps its own material (`bar_material.rs`: the wallpaper under it
  through a compressive tone curve and a mirrored blur of σ ≈ 26 pt): `materials.md` does not
  list it, so it is unchanged.

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
average and 3/255 at any pixel. With Amalfi's tint covering 60–76 % of the backdrop the
differences are smaller than before (worst case over the backdrops, mean / max levels):

| Material | 1× | 1.5× | 2× |
| --- | --- | --- | --- |
| Thick (σ 16 pt), f8 | 0.13–0.25 / 1 | 0.10–0.19 / 1 | 0.08–0.26 / 1 |
| Regular (σ 6 pt) | f2: 0.24–0.33 / 2 | f2: 0.17–0.22 / 1 | f4: 0.17–0.34 / 2 |

At 1× Glass/Regular cannot go below half resolution (f4 would take σ 6 px under the 1.5 px
floor of a reduced blur).

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
resolution and skip the lens and the measured edge in the shader, so they should cost less;
not re-measured yet.
