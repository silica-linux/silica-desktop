# Roadmap

Ten milestones in three phases, with two gates that decide whether to go on: the embedder
after M1 and daily use after M6. Dates come only after gate 1: until then the cost of the
embedder is the unknown that weighs on everything else.

## Foundations (M0 – M2)

### M0 · Foundations

- Cargo workspace, `cargo xtask ci`, `AGENTS.md`, `ROADMAP.md`
- Nested Ossido shows xdg-shell windows
- Control channel with screenshots, headless tests

### M1 · Flutter embedder

- Flutter apps on Ossido through a Rust embedder: windows, input, HiDPI
- Then layer-shell for the shell and an estimate of the maintenance cost
- OpenGL renderer and a prototype of the Dock in Flutter and in Rust (iced), measured on the
  same test bench (`docs/m1.5-dock.md`)

### Gate 1 · shell toolkit

The choice comes from the prototype's measurements: shell and apps in Flutter with the custom
embedder, shell in Rust and apps in Flutter, or the official GTK embedder as a fallback.

Decided on 2026-10-13: shell and apps in Flutter with the custom embedder and Impeller
(`docs/gate1-dock.md`).
Before M2: `linux-dmabuf` in Ossido and a measurement of the Dock with the GPU on native hardware.

### M2 · Glass and windows

- Blur and refraction shaders, `silica-glass-v1` protocol
- Server-side decorations, corners, shadows, opening and closing animations

## Shell (M3 – M5)

### M3 · Base shell

- Menu bar with clock and status, Dock, Faro on the `.desktop` files, wallpaper

### M4 · Services

- `silica-daemon`: notification server and Notification Center
- Control Center: network, Bluetooth, audio, brightness, battery

### M5 · Panorama

- View of all the windows, workspaces, 3- and 4-finger touchpad gestures

## Session and apps (M6 – M9)

### M6 · Real session

- DRM/KMS, libinput, logind, lock screen, login with greetd
- XWayland, multi-monitor, fractional scaling, tests on real hardware

### Gate 2 · daily use

Silica becomes the development session: we go on if it holds up for a week.

### M7 · App integration

- Global menu: `silica-menu-v1` and a DBusMenu bridge for Qt and GTK3
- Portal: screen capture and file chooser; approximated GTK and Qt theme

### M8 · Apps

- Settings and Mustache in Flutter, on top of `silica_ui`

### M9 · Alpha 0.1

- Packages for Ubuntu, Fedora and Arch, first version name
- Measured parity published in this file

## Measured parity

Updated only from measured metrics (visual difference on the screenshots, conformance
tests), never estimated.

| Area | Metric | Value | Measured on |
| --- | --- | --- | --- |
| Windows · corners | missing pixels per corner row against the reference mask (test `window_corners_follow_the_reference`) | maximum error 0.44 px, rms 0.13 px | 2026-10-13 |
| Windows · shadow | alpha of the side profile (d 2–48 pt), ring and above/below asymmetry, active and inactive window in light (test `window_shadows_follow_the_reference`) | maximum error 2 alpha levels out of 255 | 2026-10-13 |
| Glass · color | fill on `#7a7b80`, clear (Dock) and thick (menu) material, light appearance (test `materials_on_the_reference_gray_match_the_measured_fill`) | within 3 levels out of 255 per channel | 2026-10-13 |
| Glass · blur | profile of a stripe edge in the clear material against the Dock (test `clear_glass_blurs_a_stripe_edge_like_the_dock`) | rms 5.0 levels out of 255 | 2026-10-13 |
| Glass · edge | bright line, 7-row gradient and side edge of the clear material on `#7a7b80` against the Dock (test `clear_glass_edge_matches_the_dock_border`) | within 3 levels out of 255 per channel | 2026-10-13 |
| Glass · refraction | displacement of the stripe crossings near the side edge, clear material (test `clear_glass_refracts_near_the_side_edges_like_the_dock`) | 4.6 pt at 5.4 pt from the edge (Dock: ~5 pt at 6 pt), 0 beyond 22 pt | 2026-10-13 |
| Liquid Glass · lens | sampled source as a function of the distance from the edge, `regular` material, against the curve measured on the Control Center modules (test `regular_glass_is_the_liquid_lens_of_the_control_center`) | within 1.1 pt in the 30 pt from the edge (measurement uncertainty ±3 pt) | 2026-10-13 |
| Liquid Glass · color | fill of the `regular` material on `#7a7b80` against the modules (`#6a6b70`) | within 2 levels out of 255 | 2026-10-13 |
| Dock · geometry | screenshot of headless Ossido at 2560×1440 on `#7a7b80` against `shots/dock/rest-light.png` and `hover-mail-light.png`: edges of the background at rest and magnified, label, indicators (number tests in `shell/test/dock_model_test.dart`) | edges within 1 pt, label within 1 pt | 2026-10-13 |
| Dock · background | vertical profile of the background (bright line, gradient, fill) in the column between two icons, same comparison | within 2 levels out of 255 | 2026-10-13 |
| Menu bar | screenshot at 2560×1440 on `#7a7b80` against `shots/glass/menubar-gray-light.png`: backdrop (gradient below the bar), ink rows of the text, right edge of the clock, logo | backdrop within 1 level; text on the same rows (9–18) and same starting x; clock at the same edge; logo 1 pt lower | 2026-10-13 |
| Notifications · glass | fill of the `notification` material on `#7a7b80` against the isolated banner `#c0c1c6`; two cards 6 pt apart stay separate (test `notification_glass_matches_the_isolated_banner`) | within 3 levels out of 255 | 2026-10-13 |
| Notifications · banner | screenshot at 2560×1440 on `#7a7b80` with the menu bar against `shots/notifications/banner-gray-light.png`: frame, fill (column 2530), top, bottom and side edges, title capital, shadow below | frame x 2200–2544, y 46, radius 20 like the reference; fill within 2 levels; edge within 3 (bottom 9); title on the same row (61) and same x (2258); shadow within 1 level | 2026-10-13 |
| Control Center · grid | screenshot at 2560×1440 on `#7a7b80` with the bar against `shots/controlcenter/cc-gray-light-first.png`: bright lines of every module, side edges, active Wi-Fi disc; no bridge between modules 12 pt apart (test `every_region_of_a_group_has_its_rim_on_its_own_edge`) | lines and edges on the same pixels (y 42, 118, 194, 270, 346; x 2253 and 2394); disc on the same pixels | 2026-10-13 |
| Control Center · glass | same comparison: fill, bright line and gradient on top and at the bottom, sides | fill within 1 level; lines within 6 (top) and 6 (bottom); sides within 1; gradient within 8 | 2026-10-13 |
| Control Center · shadow | profile to the left of the stack (0–160 pt), to the right and above, same comparison | within 2 levels out of 255 | 2026-10-13 |
| Volume HUD | screenshot at 2560×1440 on `#7a7b80` with the bar against `shots/misc/volume-hud-light.png`: bright lines, side edges, track, end of the fill on the level notch, title | lines and edges on the same pixels (y 40 and 103, x 2257 and 2550); track from 2290 (ref. 2289); title x 2275 (2274) and capital 1 pt higher; fill within 8 levels, top bright line 19 below | 2026-10-13 |
| Notifications · behaviours | curves of the entrance, the exit and the Center against the frame-by-frame measurements of session 8 (tests `uscita e Centro seguono le misure della sessione 8` and the entrance one) | entrance: residual ×0.83 per frame as measured; exit: the same displacements as the measurements; Center: edge within 5 pt at 35 ms, closing on the same points | 2026-10-13 |
