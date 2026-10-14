# Gate 1 · Dock in Flutter and in Rust, measured

M1.5 report (`docs/m1.5-dock.md`): the same Dock written in Flutter (`apps/dock`) and in
Rust with iced (`crates/dock-iced`), measured by Ossido on the same test bench
(`cargo xtask bench dock`). Data from 2026-10-12.

## Decision (2026-10-13)

**Option A: shell and apps in Flutter**, with the custom embedder. Decided by the user after
the measurements on native hardware (section further below) and a hands-on trial of the two
Docks.

Why, compared with the initial recommendation below:

- iced's smoothness advantage does not repeat on native Linux: with the same renderer
  (llvmpipe) the two versions drop the same frames.
- Visually iced renders worse: the canvas shapes have no antialiasing (MSAA off by
  default, turning it on has a cost to be measured again) and the text of
  cosmic-text/cryoglyph, without hinting or gamma correction, is less sharp than Skia's with
  the same Inter.
- iced asks for more code (2.3 times), hand-written things that Flutter already provides, and
  a second design system to keep aligned with the apps' one.
- Flutter's costs (release engine, the embedder's GPU path, dmabuf in Ossido) are paid
  anyway for the apps.

Conditions:

- `linux-dmabuf` in Ossido, then a new measurement of the Flutter Dock with the real GPU on
  native hardware: slow frames under 1%.
- **Impeller, not Skia** (the user's decision). Impeller is the renderer Flutter invests
  in; Skia is the fallback that will be removed. So Impeller's crash on llvmpipe and the
  frames dropped in WSL (2–6%, worse than Skia) must be understood and fixed, not worked
  around. The measurement with the native GPU is done with flutter-gl-impeller, and Impeller
  on Vulkan (the embedder API has a Vulkan renderer) must be evaluated as well as on
  OpenGL ES.

Outcome of the conditions (2026-10-13, section "Measurements with the GPU on native
hardware"): dmabuf in Ossido done; Impeller with the GPU drops 0.17% of the frames; the crash
on llvmpipe is worked around with the fallback to Skia. Memory: decided on 2026-10-13, ~226
MiB of PSS are accepted for now for the complete shell (wallpaper, bar, Dock, Faro, one
engine per component with Impeller); a single engine with several views remains a possible
optimization.
- The shell's memory limit: see the outcome above (~226 MiB accepted for now).

## Initial recommendation (2026-10-12, superseded)

**Option B: shell in Rust (iced), apps in Flutter.** On today's measurements iced is smoother
(no frame dropped with the GPU, against Flutter's 1.4%) and takes 45 MiB less.
The shell always runs, and it is the part where a slowdown shows the most. The price is more
code (about 2.3 times), a less mature toolkit and two design systems to keep aligned.

Two caveats, before deciding:

- **Flutter's delay with the GPU comes from our embedder, not from the drawing.** In the
  dropped frames the raster thread finishes in less than 12 ms; Flutter in software drops no
  frames. The time is lost between vsync, swap and compositor, in a path that in WSL goes
  through a GPU → CPU copy (Ossido does not offer dmabuf yet). It is probably fixable, and
  must be fixed anyway for Flutter apps, in any option.
- **Everything is measured in WSL**, with OpenGL translated to D3D12. The numbers hold for the
  comparison, not as absolute values. The measurements on native Linux with the GPU are
  further below.

If UI development speed matters more, option A remains viable: the smoothness gap is in our
code, the memory one (about 45 MiB, only once if the shell uses a single engine) is not.

## Test bench

Ossido nested in WSLg, 2560×1600 output at scale 2, measured refresh about 56 Hz
(nominal period 16.7 ms). For each variant 3 runs, each with a new Ossido:

1. launch of the Dock with `app.launch`; startup = from the launch to the first buffer;
2. 10 s of pointer back and forth over the icon row, 120 events/s;
3. the pointer leaves and the Dock returns to rest with the spring (1.5 s);
4. process memory after the scrolling (`/proc/<pid>/smaps_rollup`).

The measurements are Ossido's (`perf.frames`, `perf.process`), the same for every toolkit. A
frame is slow if it arrives more than 1.5 periods after the previous one. Everything in
release builds: Rust `--release`, Flutter with a release engine built from source and Dart
AOT.

| Variant | What |
| --- | --- |
| flutter-gl | Flutter, custom embedder, OpenGL ES (Skia), GPU via D3D12 |
| flutter-gl-impeller | as above, with Impeller |
| iced-gl | iced 0.14, wgpu on OpenGL ES, GPU via D3D12 |
| flutter-software | Flutter, software renderer (CPU) |
| iced-software | iced 0.14, tiny-skia (CPU) |

## Results

Median of 3 runs. Times in ms, memory in MiB.

| Variant | Startup | Frames | Mean | p50 | p95 | p99 | Max | Slow | PSS | RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| flutter-gl | 361 | 561 | 17.77 | 17.61 | 20.81 | 30.57 | 33.2 | 8 | 106.5 | 176.1 |
| flutter-gl-impeller | 421 | 561 | 17.78 | 17.66 | 21.63 | 30.95 | 34.1 | 22 | 128.9 | 198.6 |
| iced-gl | 297 | 561 | 17.83 | 17.77 | 19.60 | 21.39 | 23.3 | 0 | 60.8 | 118.9 |
| flutter-software | 20 | 560 | 17.84 | 17.81 | 19.61 | 21.86 | 24.6 | 0 | 33.3 | 39.1 |
| iced-software | 10 | 563 | 17.79 | 17.78 | 19.37 | 21.05 | 22.3 | 0 | 11.4 | 15.0 |

Slow frames in the single runs: flutter-gl 4, 8, 10; flutter-gl-impeller 36, 12,
22; iced-gl 0, 2, 0; flutter-software 0, 0, 3; iced-software 1, 0, 0.

With the thresholds proposed in `docs/m1.5-dock.md`:

| Threshold | flutter-gl | iced-gl |
| --- | --- | --- |
| Slow frames < 1% | no (1.4%) | yes (0%) |
| p99 ≤ 1.5 periods (25 ms) | no (30.6 ms) | yes (21.4 ms) |
| No interval over 50 ms | yes (33.2 ms) | yes (23.3 ms) |
| Memory | to be set: 106.5 MiB | to be set: 60.8 MiB |

## What the numbers say

- **Smoothness.** The two versions keep the compositor's pace (mean about 17.8 ms).
  Flutter with the GPU drops 0.7–1.8% of the frames (Impeller 2–6%), spread over the whole
  scroll and not only at the start. In a run with the raster log
  (`silica_embedder::ffi::gl` in debug) those frames have a drawing time under 12 ms.
  Impeller, the new renderer, does worse than Skia on this path.
- **Memory.** Flutter costs about 45 MiB more with the GPU and 22 MiB more in software, per
  process. PSS includes the graphics driver libraries, shared between processes.
- **Startup.** With the GPU the initialization of the D3D12 driver dominates (200–400 ms, very
  variable) for both. In software Flutter AOT starts in 20 ms, iced in 10.
- **Look.** The screenshots (`target/bench/dock/*-hover.png`) have the same geometry.
  In WSL iced's surface is opaque: wgpu 27 on OpenGL declares only `Opaque`
  (`wgpu-hal/src/gles/adapter.rs`, a TODO), and so does tiny-skia. Transparency is there with
  Vulkan, which in WSL is only lavapipe (CPU); on real hardware with Vulkan it has to be
  checked.

## Measurements on native hardware (2026-10-12)

Same bench, same PC, with native Ubuntu 26.04 instead of WSL: Core i9-13900H
(20 threads), Iris Xe (Raptor Lake-P) and RTX 4070 Laptop, Mesa 26.0.8, NVIDIA driver 595.91.
Ossido nested in the GNOME session (Wayland), 1920×1200 screen at 59.88 Hz.

**The "GPU" variants here do not use the GPU.** Ossido does not offer `zwp_linux_dmabuf_v1`
(nor `wl_drm`), and without one of the two Mesa does not know which device to use for a
Wayland surface: the clients' EGL falls back to llvmpipe (`libEGL warning: failed to get
driver name for fd -1`, and the embedder writes `renderer OpenGL ES: llvmpipe`). With Vulkan
wgpu picks lavapipe; with only the Intel driver or only the NVIDIA one no Vulkan surface is
usable and iced falls back to tiny-skia (PSS 14–16 MiB). In WSL the problem did not show
because Mesa's d3d12 driver does not need dmabuf and always copies the frame into shared
memory.

**Scale 1 instead of 2.** GNOME shrinks Ossido's 2560×1600 window to 1920×1082, and the
bench's pointer fell outside the Dock (0 frames in every variant). The bench now has
`--scale N` (default 2): here it runs with `--scale 1`, 1280×800 pixel output. The logical
output, the geometry and the pointer path are the same; the pixels to draw are a quarter.

Median of 3 runs, times in ms, memory in MiB. In parentheses the value in WSL.

| Variant | Renders with | Startup | Frames | p95 | p99 | Max | Slow | PSS |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| flutter-gl | llvmpipe | 1794 (361) | 597 | 19.03 | 20.74 (30.57) | 33.6 | 2 (8) | 113.1 (106.5) |
| flutter-gl-impeller | llvmpipe | — | — | — | — | — | crash | — |
| iced-gl | llvmpipe | 1868 (297) | 597 | 19.14 | 20.89 (21.39) | 35.0 | 3 (0) | 103.3 (60.8) |
| iced-vulkan | lavapipe | 143 | 597 | 19.05 | 20.60 | 32.5 | 2 | 81.3 |
| flutter-software | CPU | 44 (20) | 598 | 19.03 | 20.29 (21.86) | 26.0 | 1 (0) | 25.7 (33.3) |
| iced-software | tiny-skia | 27 (10) | 598 | 19.29 | 21.12 (21.05) | 29.4 | 1 (0) | 11.7 (11.4) |

Slow frames in the single runs: flutter-gl 2, 2, 2; iced-gl 2, 3, 3; iced-vulkan
3, 2, 2; flutter-software 3, 1, 1; iced-software 1, 1, 1. Startup of the llvmpipe and
lavapipe variants is very variable (113–2072 ms): in some runs the EGL or Vulkan loader
tries the NVIDIA drivers first.

Answers to the three questions, as far as these measurements allow:

- **Does Flutter with the GPU still drop frames?** Not measurable until Ossido offers dmabuf.
  With llvmpipe flutter-gl drops 0.3% of the frames (2 out of 597) with p99 20.7 ms, against
  1.4% and 30.6 ms in WSL, and it is now on par with iced-gl (3 frames, p99 20.9 ms). The
  comparison holds (same renderer for both), but it says nothing about the real GPU path.
  It is consistent with the report's hypothesis: the drops in WSL came from the frame path,
  not from Flutter's drawing.
- **Does Impeller change anything?** On llvmpipe the engine hits a segmentation fault during
  the scroll in 4 attempts out of 4 (`embedder_surface_gl_impeller.cc` declares the OpenGLES
  backend, then the process dies and the bench no longer finds the surface).
  The crash is inside the engine; there is no measurement to compare.
- **Is iced on Vulkan transparent?** Yes, but with lavapipe, not with the GPU: the screenshot
  `iced-vulkan-hover.png` shows Ossido's background around the Dock. iced-gl on llvmpipe is
  also transparent here, unlike in WSL. iced-software stays opaque (black around the Dock).
  With Vulkan on Intel or NVIDIA the question stays open for the same reason: dmabuf is
  needed.

What changes for gate 1: the native measurements do not contradict the recommendation, but
they remove its main argument. iced's smoothness advantage in WSL does not repeat here (on
the same renderer the two versions are even); the memory advantage remains, which in
software is 14 MiB (25.7 against 11.7) and with the GPU drivers depends on which driver gets
loaded. A measurement with the real GPU requires `linux-dmabuf` in Ossido, which is needed
anyway (see "What remains open").

## Measurements with the GPU on native hardware (2026-10-13)

Ossido now offers `zwp_linux_dmabuf_v1` version 4 with feedback on the render node of its
GPU (`crates/ossido/src/dmabuf.rs`, winit backend only). The nested clients render on
Ossido's GPU, the Iris Xe (`renderer OpenGL ES: Mesa Intel(R) Iris(R) Xe Graphics`), and
pass the buffers without copies. Same PC and same bench as the previous section, with
`--scale 1`. The embedder has `--renderer impeller` (OpenGL ES with Impeller), which the bench
uses for flutter-gl-impeller.

Median of 3 runs, times in ms, memory in MiB.

| Variant | Renders with | Startup | Frames | p95 | p99 | Max | Slow | PSS | RSS |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| flutter-gl-impeller | Iris Xe | 72 | 598 | 18.30 | 19.23 | 27.5 | 1 | 72.1 | 144.9 |
| flutter-gl | Iris Xe | 79 | 598 | 18.23 | 18.98 | 28.3 | 1 | 60.7 | 132.6 |
| iced-gl | Iris Xe | 79 | 600 | 18.26 | 19.01 | 28.6 | 1 | 50.0 | 122.8 |
| iced-vulkan | Vulkan (GPU chosen by wgpu) | 273 | 598 | 18.05 | 19.75 | 30.5 | 2 | 99.1 | 166.1 |
| flutter-software | CPU | 45 | 597 | 19.10 | 20.70 | 31.5 | 2 | 25.6 | 29.3 |
| iced-software | tiny-skia | 27 | 598 | 19.21 | 21.19 | 25.9 | 1 | 11.7 | 16.0 |

Slow frames in the single runs: flutter-gl-impeller 0, 1, 1; flutter-gl 1, 1, 1;
iced-gl 0, 1, 2; iced-vulkan 2, 1, 2; flutter-software 2, 2, 3; iced-software 1, 1, 0.
The isolated slow frame shows up in all the variants, software included: it belongs to the
path between Ossido and GNOME, not to the toolkits.

Gate 1 conditions:

| Condition | Outcome |
| --- | --- |
| Slow frames < 1% with the GPU | yes: Impeller 0.17% (1 out of 598), p99 19.2 ms, max 31.8 ms |
| No interval over 50 ms | yes |
| Impeller as the renderer | yes, on OpenGL ES; no crash with the GPU |
| Memory | Dock with Impeller: 72 MiB of PSS (Skia 61, iced 50). The limit is yours to set |

With the real GPU the Flutter delay seen in WSL disappears: the three GPU variants have the
same pace. Impeller costs about 11 MiB more than Skia. The screenshot
`flutter-gl-impeller-hover.png` has the same geometry as the others and the surface is
transparent.

Impeller on a software GL driver (llvmpipe) still crashes inside the engine: with
`--renderer impeller` the embedder now recognizes the driver and falls back to Skia, with a
warning in the log. The test `impeller_renderer_draws_and_reacts_to_clicks` covers the case
(headless and in CI the driver is llvmpipe). Impeller on Vulkan is not measured: the embedder
does not have a Vulkan renderer yet, and on OpenGL ES Impeller already meets the conditions.

## Development cost

| | Flutter | iced |
| --- | --- | --- |
| Lines (without tests) | 188 | 429 |
| Integration problems | 1: fonts outside the package (solved with `packages/silica_ui`) | 4: `wp_viewporter` mandatory in Ossido, winit-core to be pinned to a beta, opaque surface on GL, `CursorEntered` without movement |
| Spring, HSL colors | from the framework | hand-written (`geometry.rs`) |
| Agent time | about 5 minutes | about 15 minutes, plus the investigations |

The times are those of the agent that wrote the prototypes: indicative, not a measurement.

## Costs discovered during M1.5

- **Release engine.** Google publishes the engine for custom embedders only in debug. The
  release one is built from source (`docs/engine-release.md`): 9.4 GB of checkout, about
  15 minutes, at every Flutter update. It is needed for Flutter apps in every option.
- **OpenGL path in WSL.** Mesa (d3d12) applies the resize of the EGL window only from the
  next buffer: the embedder now asks for one more frame after every resize.
- **Frame pace.** The embedder follows the compositor's frame callbacks
  (`vsync_callback`), as iced does; before, the engine ran on its own 60 Hz timer.

## Impeller on Vulkan (2026-10-13)

`--renderer vulkan`: the embedder creates the Vulkan instance and device (`ash`), a
`VK_KHR_wayland_surface` swapchain on the app's surface with premultiplied alpha, MAILBOX or
IMMEDIATE presentation (the pace stays the engine's vsync), and gives everything to the engine
with `FlutterVulkanRendererConfig` and `--enable-impeller`. It prefers the integrated GPU
(`SILICA_VULKAN_GPU=discrete` for the discrete one); without Vulkan it falls back to OpenGL ES
with Impeller. It is the shell's renderer in `cargo xtask run --shell`.

Why: with Impeller on OpenGL ES this driver does not provide multisampling and the paths
(glyphs, outlines) stay without antialiasing; on Vulkan it does, with the same quality as Skia.

Measurement: Ossido nested in GNOME (XPS 15, Iris Xe, Mesa), the shell's Dock 1600 × 1000, the
pointer back and forth over the Dock for ~6 s (`perf.frames`, `perf.process`), two runs:

| Renderer | Frames | p50 | p95 | p99 | Max | Slow | PSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Skia (GL) | 431 | 16.7 | 20.1 | 31.6 | 43.7 | 7 | 75 MiB |
| Impeller (GL) | 423 | 16.7 | 19.0 | 20.7 | 29.5 | 1 | 80 MiB |
| Impeller (Vulkan) | 427–429 | 16.7 | 19.0–19.4 | 20.0–20.6 | 23–29 | 0–1 | 78 MiB |

A first run with Impeller on GL had a 3 s hitch (102 frames), not repeated. Vulkan is even or
better on everything and has antialiasing.

## What remains open

- `wp_presentation` for the frame pace (dmabuf in Ossido is done).
- Impeller on Vulkan in a real session (DRM/KMS) and on the discrete GPU.
- The measurement in a real session (DRM/KMS, M6), not nested in GNOME.
- A single engine with several views for the shell, if memory becomes a problem.

## Repeating the measurements

```bash
SILICA_ENGINE_SRC=~/build/flutter-engine cargo xtask engine --release
cargo xtask bundle dock --release
cargo xtask bench dock                 # --runs N --seconds S --only VARIANT --scale N
```

Results in `target/bench/dock/results.json`, screenshots next to it.
