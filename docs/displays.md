# Displays: scaled modes (#216)

A display has a mode (what it scans out) and, as macOS has it, a size it *looks like*. In the
plain mode these are the same, and the fractional scale (`scale`, 1–4 in quarter steps, apps
draw at that scale) stays available. In a **scaled mode** the display looks like a smaller
size, say 1536 x 960 on a 1920 x 1200 panel, and Ossido gets there the way macOS does:

1. the output is the size it looks like times 2 (3072 x 1920) with scale 2: apps and the shell
   see an ordinary 2x display (`wl_output` scale 2, so integer buffers and the 48 px Adwaita
   cursor) and draw crisp at 2x;
2. Ossido composites the whole scene, glass passes included, at that size into an offscreen
   texture;
3. a separable Lanczos filter reduces the texture to the display's mode.

Everything that works in the output's logical or physical pixels (layout, hit testing, the
pointer, the cursor, window placement) is unchanged: it is a 2x output. Only the backend knows
the display's own size.

## Switching

```
output.configure {"logical": {"width": 1536, "height": 960}}   # a scaled mode
output.configure {"logical": null}                             # the plain mode again
output.configure {"logical": {"width": 1920, "height": 1200}}  # the same (the native size)
output.configure {"scale": 1.25}                               # fractional scale; leaves the scaled mode
```

Over the control channel (`--control`), or from Settings through silica-daemon
(`ConfigureOutput`, `output.configure` as JSON). The choice is saved in `displays.json` under
the display's identity (`"logical": {"width": ..., "height": ...}`) and applied at start, when
the file changes and when the display is plugged in. `logical` and `scale` exclude each other:
setting one clears the other.

`output.list` tells what a display can look like, for Settings:

```json
{"current": {"width": 1920, "height": 1200, "refresh": 60.0},
 "logical": {"width": 1536, "height": 960}, "hidpi": true, "scale": 1.0,
 "looksLike": [{"width": 1920, "height": 1200, "scaled": false},
               {"width": 1680, "height": 1050, "scaled": true},
               {"width": 1536, "height": 960, "scaled": true},
               {"width": 1440, "height": 900, "scaled": true},
               {"width": 1280, "height": 800, "scaled": true}]}
```

`current` is always the display's mode, `scale` the fractional scale the user chose (1 in a
scaled mode, where the output's own scale is the fixed 2). `looksLike` is the display's size
times 7/8, 4/5, 3/4 and 2/3, and on a panel 2560 or more across also 3/5 and 1/2, keeping the
whole sizes only. Any other size is accepted by `output.configure` if it is not larger than the
display, not smaller than half of it, at least 640 x 400, and has the display's shape within 1%.
The ratio of the reduction is then between 1 and 2.

The sizes are those of the unrotated mode (a rotated display is not specially treated yet).

## The filter

`ossido_render::scaler`, `shaders/scale.frag`. Two passes, horizontal into an intermediate
texture as wide as the display and as tall as the 2x image, then vertical into the buffer to
scan out. Every output pixel is a normalised sum of the source pixels under a kernel scaled
by the ratio, centred on the pixel's place in the source (the edge pixels repeat); the result
is clamped to the valid premultiplied range (Lanczos has negative lobes).

Candidates, measured on text-like content (1 px lines every 2 and 3 pt, 1 pt checker, thin
diagonals, a disc, edges between pixels), the scene rasterised at 2x with 4x4 samples per pixel
and reduced, against the same scene rasterised straight at the display's size with 8x8
samples (PSNR, mean over ratios 1.33, 1.5, 1.6 and 1.75, `scaler_tests.rs`):

| Filter | PSNR | Taps per axis at ratio 1.6 |
| --- | --- | --- |
| box (mean of the pixels under it) | 23.1 dB | 2–3 |
| bilinear (the triangle; what a plain GPU blit does) | 25.0 dB | 4 |
| Catmull-Rom (bicubic) | 31.2 dB | 7 |
| **Lanczos, 2 lobes** | 30.9 dB | 7 |
| Lanczos, 3 lobes | 29.7 dB | 10 |

Lanczos-2 is chosen. Box and bilinear blur 1 px lines and, below a ratio of 2, alias them.
Lanczos-3 is sharper in the frequency domain but its second lobe rings next to hard edges,
which is what UI is made of, and costs 40% more taps; it is worse here at most ratios.
Catmull-Rom is as good as Lanczos-2 on the numbers; Lanczos-2 keeps a sharper edge and the
same support (2 lobes), and the shader is one formula for 2 or 3 lobes (`DOWNSCALE_LOBES`).
The two cost the same on the GPU.

The shader and the CPU reference (`scaler::resample`) share the weights; the test
`the_shader_matches_the_cpu_reference` compares them on llvmpipe (at most 1/255 apart).

## Damage

A frame redraws only what changed: the damage tracker draws into the 2x texture (its
contents are kept from frame to frame, age 1) and returns the damaged rectangles. Each one
becomes a rectangle of the display grown by the filter's reach (the lobes, plus one for the
rounding), and the intermediate texture is refreshed for the rows that rectangle reads
(`scaler::plan`). Overlapping rectangles are merged, and a damage with more than 16 rectangles
becomes its bounding box.

The swapchain has several buffers of different ages: a buffer of age N also misses the damage of
the last N - 1 frames, which `scaler::DamageHistory` remembers (age 0, a new buffer, is redrawn
in full). The damage handed to KMS is that of the frame alone.

## Input, cursor and captures

* Pointer: libinput deltas and `input.pointer_move` are in logical points, as always, so the
  pointer, hit testing and window placement need nothing new. A point is larger than a pixel of
  the display in a scaled mode (1.25 pixels when 1920 x 1200 looks like 1536 x 960), so the
  same hand movement crosses 1.25 times as much of the screen; the pointer speed setting
  compensates if it matters.
* Cursor: drawn into the 2x texture with the theme at scale 2 (48 px for Adwaita), its hotspot
  at the pointer, and reduced with the rest.
* Screenshots (`ui.screenshot`, the screen, region and clipboard captures): drawn at 2x and
  reduced with the same filter, so the file is what the display shows, at the display's own size.
  A region is given in logical points and cut in display pixels (the pointer drawn into it
  scaled by display pixels per point). The capture of one window is not reduced: it has the
  window's 2x pixels.
* Winit (nested) does not offer scaled modes: the host window has its own scale.

## Cost

Measured with `cargo test -p ossido --release --lib hidpi_cost -- --ignored --nocapture` (the
reduction alone, fed with the 2x image already drawn) and `--test hidpi_cost` (a busy desktop:
three 600 x 400 windows, two with glass, full glass quality, a full frame drawn and read back).
On this laptop (Intel Iris Xe and NVIDIA RTX 4070 Max-Q; EGL picks the first GPU) and on
llvmpipe.

The reduction, median of six:

| Display <- drawn | llvmpipe, whole frame | llvmpipe, 300 x 200 damage | GPU, whole frame | GPU, 300 x 200 damage |
| --- | --- | --- | --- | --- |
| 1920 x 1200 <- 3072 x 1920 | 37.8 ms | 1.9 ms | 0.79 ms | 0.17 ms |
| 1920 x 1200 <- 3360 x 2100 | 41.2 ms | 1.8 ms | 0.99 ms | 0.29 ms |
| 3840 x 2160 <- 5120 x 2880 | 107.8 ms | 1.8 ms | 2.08 ms | 0.43 ms |

A whole frame of the busy desktop on llvmpipe (typical of the run; the first frames compile
shaders), ms:

| Mode | Frame |
| --- | --- |
| native 1920 x 1200 | 10–14 |
| 2x scene only, 3072 x 1920, no reduction | 25–30 |
| looks like 1536 x 960 (3072 x 1920 scene + reduction) | 64–69 |
| looks like 1680 x 1050 | 63–75 |
| looks like 1280 x 800 | 53–58 |

So on a software rasterizer the scene costs about 2.5x for 2.56x the pixels, and the reduction
about as much again; the real cost is the 2x scene, and it is the glass blurs that scale with
the pixels. On a GPU the reduction is under a millisecond for the whole frame and a fraction of
that for typical damage, and the scene is the 2.56x of a 2x output. (The GPU rows of a full
frame are not given: with the readback the frame is dominated by it, 8–10 ms at both sizes.)
Measure on the VM's virtio-gpu with `cargo xtask os` when it matters; an animation that
redraws the whole screen is the worst case, as it is in the plain mode.

## Not done

* Rotated displays and several displays: Ossido drives one output.
* A wp_fractional_scale global (scale 2 is an integer `wl_output` scale and
  `preferred_buffer_scale`).
* Hardware cursor planes: the pointer is part of the frame, as in the plain mode.
