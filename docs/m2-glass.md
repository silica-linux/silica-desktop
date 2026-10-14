# M2 · Glass

How Ossido draws the glass of `silica-glass-v1` (`docs/protocols.md`), where the parameters
come from and what it costs. Code in `crates/ossido-render/src/glass.rs` and in the shaders
`blur.frag` and `material.frag`.

## How it works

For every surface with glass regions, on every frame in which what lies behind it changes:

1. the elements behind the surface (and the desktop color) are redrawn into an offscreen
   texture as large as the regions plus 3σ;
2. the texture is blurred with a separable Gaussian (at half or a quarter of the resolution
   when σ exceeds 12 or 24 pixels) and mixed with the measured sharp share;
3. every region is drawn with the material shader: refraction near the edge (sampling
   displaced along the normal), color matrix on the sRGB values, bright edge on top and at
   the bottom and dark on the sides, circular-arc corners.

If nothing changes behind (same elements, same commits, same geometry) the textures are
reused. A window's glass does not see its own bar or its own shadow.

## Materials (light appearance)

| Material | From | σ | Sharp share | Color | Refraction |
| --- | --- | ---: | ---: | --- | --- |
| `clear` | Dock background | 2.75 pt | 0.28 | free matrix of the Dock, b = (37, 39, 37) | ~5 pt on the sides at 6 pt from the edge, ~1 on top, ~3 at the bottom |
| `thick` | menus | 9 pt | 0 | menu matrix, b = (142, 140, 140) | ≤ 1.5 pt on the sides |

Values and uncertainties in `plan/reference/glass.md`, `dock.md`, `menubar.md`. The dark
appearance is not there yet.

## Quality levels

| Level | What the glass sees | When |
| --- | --- | --- |
| `full` | everything behind it | default with a GPU |
| `frosted` | only the desktop wallpaper (background and bottom layers): rarely recomputed | default on a software rasterizer (llvmpipe) |
| `opaque` | nothing: the material over the desktop color | on request |

`--glass auto|full|frosted|opaque` at startup, `ui.glass_quality` during the session.

## Cost (2026-10-13)

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
queries, not yet in Ossido.
