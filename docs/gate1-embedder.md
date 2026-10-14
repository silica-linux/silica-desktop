# Gate 1 · Custom Flutter embedder

> M1.5 update: the measured comparison between Flutter and Rust for the shell is in
> `docs/gate1-dock.md`, which replaces the recommendation below for the shell.
> Decision of 2026-10-13: shell and apps in Flutter.
> The OpenGL renderer and the release engine (open risks below) are now there.

Recommendation: **go**. The custom embedder in Rust brings a Flutter app onto Ossido with
rendering, pointer, keyboard, text, HiDPI and layer-shell in about 2,400 hand-written lines,
with the engine API stable and the version update automated. Two risks remain open and must
be measured before M3: the GPU renderer and the keyboard protocol.

The decision is yours (AGENTS.md, "Session start"). Data as of 2026-10-12, Flutter
3.47.7, engine `deb287481e3ce9468f3434937ced4240a70539ca`.

## What works, verified by tests

Every row has a test that runs in `cargo xtask ci`, without a GPU, on headless Ossido.

| Capability | How | Test |
| --- | --- | --- |
| First frame | the engine's software renderer → `wl_shm` | `demo_renders_reacts_to_clicks_and_closes` |
| Pointer | `FlutterPointerEvent` (add, hover, down, move, up, scroll) | same test: click → color changes |
| HiDPI | `wl_surface` buffer scale + `pixel_ratio` | `demo_renders_at_scale_two` |
| Keys | `FlutterKeyEvent` + GTK message on `flutter/keyevent` | `keyboard_reaches_text_field_and_hardware_keyboard` |
| Text | `flutter/textinput` with an editing model (UTF-16) | same test + 5 unit tests |
| Layer-shell | `zwlr_layer_surface_v1`: anchors, exclusive zone, exclusive keyboard | `layer_bar_reserves_space_takes_keyboard_and_clicks` |
| Shutdown | `FlutterEngineShutdown`, clean exit | `demo_renders_reacts_to_clicks_and_closes` |

Also checked by eye, nested in WSLg: app in a window, bar at the top with a window
below.

## What it costs to maintain

| Item | Measure |
| --- | --- |
| Hand-written code | 2,430 lines in `crates/embedder/src` (unit tests included), plus 378 of e2e tests |
| Generated code | 3,027 lines of bindings (`cargo xtask bindings`), 427 of key tables (`cargo xtask keymap`) |
| Engine API used | 11 functions (below) |
| `unsafe` | 28 blocks, all in `src/ffi/mod.rs`, each with `SAFETY` |
| Engine | `libflutter_engine.so` 43 MB, loaded at runtime |

Functions used: `FlutterEngineRun`, `FlutterEngineShutdown`, `FlutterEngineRunTask`,
`FlutterEngineGetCurrentTime`, `FlutterEngineSendWindowMetricsEvent`,
`FlutterEngineSendPointerEvent`, `FlutterEngineSendKeyEvent`,
`FlutterEngineSendPlatformMessage`, `FlutterEngineSendPlatformMessageResponse`,
`FlutterPlatformMessageCreateResponseHandle`, `FlutterPlatformMessageReleaseResponseHandle`.

**Updating Flutter.** The bundles run only on the engine of the same SDK, so at every
update:

1. update `crates/embedder/ENGINE_VERSION` (`cargo xtask engine` refuses versions different from the SDK's);
2. `cargo xtask engine && cargo xtask bindings && cargo xtask keymap`;
3. `cargo xtask ci`: the e2e tests tell whether something broke.

`flutter_embedder.h` is stable by construction: every struct carries `struct_size` and
`FLUTTER_ENGINE_VERSION` has been 1 since it exists. The risk is not the ABI, it is the
channel protocols (below).

## Open risks

| Risk | Impact | What to do |
| --- | --- | --- |
| Software renderer only: cost per frame not measured; at 4K or with animations it may not be enough | High | M1.5: OpenGL renderer (EGL + `wl_egl_window` or dmabuf) and measurement of the frame time at 1280×800 and 2560×1600 |
| Keyboard: the framework queues the `KeyEvent`s until it receives the raw `flutter/keyevent` message, deprecated since 3.18. When they remove it the protocol changes | Medium | Follow the changes of the official GTK embedder at every update; the e2e test flags it right away |
| Functions of the GTK embedder not yet redone | Medium | List below; each one is a channel or a Wayland protocol, small on its own |

Still to do compared with the official GTK embedder, with a rough estimate:

- Clipboard (`flutter/platform` Clipboard) and cursors (`flutter/mousecursor`): 1 day.
- Repeat of held keys: half a day.
- IME (`text-input-v3`) for languages with composition: 2-3 days.
- Accessibility (semantics → AT-SPI): 1-2 weeks, the biggest item.
- Fractional scale (`wp_fractional_scale_v1`): 1 day.
- Popups and menus as xdg_popup surfaces: 2-3 days, needed for the menu bar (M3).

## The no-go alternative

The official GTK embedder (`libflutter_linux_gtk.so`) with `gtk-layer-shell` for bar and Dock.
It works today, but Silica's glass goes through its own Wayland protocol
(`silica-glass-v1`): with GTK a plugin is needed to get the `wl_surface` from GDK, and the
control over frames, input and scale stays with GTK. The headless harness is lost: the shell
tests would require a display.
