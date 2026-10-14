# Control channel

Ossido opens the channel with `--control PORT` on `127.0.0.1` (`0` picks a free port) and
announces the address on stdout with a line `OSSIDO_CONTROL=127.0.0.1:PORT`.
The channel is for agents and tests: it is not authenticated, so it stays on localhost.

## Protocol

One JSON line per message. The client sends a request, Ossido answers with the same `id`.
Requests on the same connection are served in order.

```
→ {"id":1,"command":"window.move","params":{"x":10,"y":20}}
← {"id":1,"ok":true,"result":null}
← {"id":2,"ok":false,"error":"no window with id 9"}
```

A request over 1 MiB closes the connection. Unknown parameters are an error.

## Commands

The catalog is in `crates/control/src/commands.rs`; `engine.commands` returns it with the
parameter schemas. Every catalog id is run by Ossido (test
`every_catalog_command_is_implemented`).

| Command | Parameters | Result |
| --- | --- | --- |
| `ui.screenshot` | `path?` | `width`, `height`, `path` or `png_base64` |
| `ui.animation_scale` | `scale` (0–1000) | — (multiplies the animation durations; 0 turns them off) |
| `ui.glass_quality` | `quality` (`auto`, `full`, `frosted`, `opaque`) | `quality` in use |
| `engine.commands` | — | the catalog |
| `engine.execute` | `id`, `params?` | the command's result |
| `window.list` | — | windows from top to bottom, then the minimized ones: `id`, `title`, `app_id`, `x`, `y`, `width`, `height` (of the content, without the bar), `focused`, `minimized`, `fullscreen`, `space` |
| `input.gesture` | `kind` (`swipe`, `pinch`), `fingers`, `phase?` (`begin`, `update`, `end`; without: the whole gesture), `dx`, `dy`, `scale`, `cancelled` | `command` run at the end (null if none). Provisional (PA-20…22): 3–4 fingers ←/→ space, ↑ Panorama, ↓ the app's windows or closes the view; thumb and three fingers: spreading the desktop, pinching the launcher |
| `space.list` | — | spaces: `index`, `active`, `windows` (ids from bottom to top) |
| `space.next`, `space.previous` | — | `switched`: false past the last or before the first (no bounce); ⌃→ and ⌃← |
| `space.goto` | `index` | `switched` |
| `space.add` | — | `index` of the new space (at most 16) |
| `space.remove` | `index` | — ; the windows move to the previous space (or to the first) |
| `window.move_to_space` | `id?`, `index` | — |
| `panorama.toggle` | — | `open`: opens or closes Panorama (⌃↑; Esc closes). When open, a click on a window brings it to the front, on empty space closes, on a space of the bar goes there, on the "+" adds a space |
| `panorama.state` | — | `mode` (`panorama`, `app`, `desktop`; absent if closed), `open`, `progress` (0–1), `windows`: `id`, `x`, `y`, `width`, `height` of the arranged frames; `expansion` of the spaces bar (0–1), `thumbnails`: rectangles of the thumbnails when visible |
| `panorama.app` | — | `open`: App Exposé, the windows of the frontmost app at the center with the title below (⌃↓; Esc closes) |
| `desktop.show` | — | `open`: Show Desktop, the windows move out towards the nearest edge (F11; again or a click puts them back) |
| `panorama.spaces_bar` | `enabled` | — ; expands or collapses the spaces bar (like the pointer at the top). With the bar expanded a window dragged onto a thumbnail moves to that space; a click on a thumbnail goes there |
| `switcher.next`, `switcher.previous` | — | `forwarded`: opens the app switcher or moves the selection (Logo+Tab, Logo+Shift+Tab); while it is open, releasing the Logo key runs `switcher.commit` and Esc `switcher.cancel` |
| `switcher.commit`, `switcher.cancel` | — | `forwarded`: closes the switcher activating the chosen app, or without changing app |
| `launcher.toggle` | — | `forwarded`: opens or closes the app launcher (Faro in grid mode) |
| `app.quit` | — | `closed`: asks all the windows of the frontmost app to close (Logo+Q) |
| `faro.toggle` | — | `forwarded`: shell components that received it (Logo+Space) |
| `notifications.toggle` | — | `forwarded`: opens or closes the Notification Center (click on the clock) |
| `controlcenter.toggle` | — | `forwarded`: opens or closes the Control Center (icon in the bar) |
| `system.status` | — | state from silica-daemon (`docs/daemon.md`, System1): `wifi.*`, `bluetooth.*`, `battery.*`, `brightness`, `audio.*`, `dnd`; error if the daemon does not answer |
| `system.wifi`, `system.bluetooth`, `system.mute`, `system.dnd` | `enabled` | `queued`: the call runs on a thread, without waiting for the daemon |
| `system.volume`, `system.brightness` | `value` (0–1) | `queued` |
| `system.volume_up`, `system.volume_down`, `system.mute_toggle` | — | `queued`; steps of 1/16, raising unmutes (volume and mute keys) |
| `system.brightness_up`, `system.brightness_down` | — | `queued`; steps of 1/16 (brightness keys) |
| `system.volume_up_fine`, `system.volume_down_fine`, `system.brightness_up_fine`, `system.brightness_down_fine` | — | `queued`; a quarter step, 1/64 (⌥⇧ with the keys) |
| `glass.list` | — | surfaces with glass (`silica-glass-v1`): `surface`, `x`, `y`, `regions` (`x`, `y`, `width`, `height`, `radius`, `material`, `opacity`) |
| `layer.list` | — | shell layers: `namespace`, `layer`, `x`, `y`, `width`, `height`, `exclusive_zone`, `focused` |
| `window.focus` | `id?` | — (also brings back a minimized window) |
| `window.close` | `id?` | — |
| `window.move` | `id?`, `x`, `y` | — |
| `window.minimize` | `id?` | — (Logo+M) |
| `window.fullscreen` | `id?` | — (Logo+Ctrl+F; again to go back) |
| `app.launch` | `command`, `shell?` | `pid` (with `shell` the process sees the shell protocols) |
| `session.quit` | — | — |
| `input.pointer_move` | `x`, `y` | — |
| `input.pointer_button` | `button?`, `pressed` | — |
| `input.click` | `x`, `y`, `button?` | — |
| `input.key` | `key` (xkb name or character), `pressed?` | — ; like a real key, catalog shortcuts included (⌃→ switches space) |
| `input.type` | `text` | — |
| `perf.frames` | `reset?` | per surface: `surface`, `pid`, `frames`, `mean_ms`, `p50_ms`, `p95_ms`, `p99_ms`, `max_ms`, `slow`, `period_ms`, `startup_ms` |
| `perf.process` | `pid` | `pid`, `rss_kib`, `pss_kib` |
| `perf.render` | `reset?` | Ossido's rendering (winit): `frames`, `mean_ms`, `p50_ms`, `p95_ms`, `p99_ms`, `max_ms` |

The `perf.frames` measurements are the compositor's, the same for every toolkit: a frame is
a commit with a new buffer, the interval is the time between two frames of the same
surface, slow if it exceeds 1.5 refresh periods of the output. `startup_ms` goes from the
launch with `app.launch` (or `--spawn`) to the first buffer of the process or of one of its
descendants.

Without `id`, the window commands act on the focused one.

## CLI and MCP

```
cargo xtask run --nested --control 7979 --spawn foot
cargo xtask run --nested --shell --control 7979   # with wallpaper, menu bar and Dock
silica-cli window.list
silica-cli window.move x=100 y=80
silica-cli screenshot /tmp/silica.png
silica-cli mcp            # MCP server on stdio, configured in .mcp.json
```

Nested in GNOME (or in another desktop) the combinations with Super are taken by the desktop
below: for this reason with `--backend winit` the **right Ctrl acts as the Logo key** (right
Ctrl+Space opens Faro, right Ctrl+Tab the app switcher). `--logo-key rightalt` uses the right
Alt, `--logo-key none` leaves the keys as they are (default in headless).

The appearance (light or dark) of windows, bars, shadows and glass is decided by
`silica-daemon` (`appearance.dark`, changed with `system.dark` / `system.dark_toggle`); Ossido
reads it again on every frame. Without the daemon (headless, tests) it is light:
`--appearance dark` (or `light`) forces it and wins over the daemon, for deterministic
screenshots in dark:

```
ossido --backend headless --appearance dark --control 0
```

Via MCP the commands are tools with `_` instead of `.` (`window_list`); `ui_screenshot`
without `path` returns the image as `image` content.
