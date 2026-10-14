# Silica protocols

The XMLs are in `crates/protocols/xml/`; the Rust bindings in `silica-protocols` (feature
`server` for Ossido, `client` for the embedder and the tests).

## silica-glass-v1

A client declares regions of one of its surfaces to render as glass: Ossido samples what lies
behind the surface and shows it blurred, with the material's color and refraction. The
client leaves transparent the pixels where the glass must show and draws text and icons on
top.

- `silica_glass_manager_v1.get_glass(id, surface)`: one glass object per surface
  (a second one is the `already_has_glass` error).
- `silica_glass_v1.clear()`, `add_region(x, y, width, height, radius, material)`: pending
  state, current at the next `wl_surface.commit`. Surface coordinates in logical pixels;
  circular-arc corners of radius `radius`, like the measured glass (Dock, menus,
  notifications: windows instead have superellipse corners).
- Materials: `clear` (clear glass, like the Dock), `thick` (thick glass, like menus and
  search) and `regular` (the Liquid Glass of the controls, like the Control Center modules:
  a strong lens that at the edge also samples the outside background), from the measurements
  in `plan/reference/glass.md`. Since version 2 also `notification` (banners and Notification
  Center: very wide blur, almost opaque, dark edge line on the first pixel of the region,
  which the client widens by 1 pt per side; the regions do not merge). A client bound to
  version 1 that asks for it gets an `invalid_region` error; the embedder falls back to
  `thick` there.
- Errors: `invalid_region` (non-positive size, negative or non-finite radius, unknown
  material), `too_many_regions` (over 64). The client is disconnected; Ossido goes on.

- Since version 3: `add_translucent_region(…, opacity)`, the same region composited with an
  opacity 0–1. A panel that fades in or out carries the glass with it; the opacity does not
  redo the blur (only the drawing changes). Outside 0–1 it is `invalid_region`. `silica_ui`
  derives it from the widget's `Opacity` ancestors.

- Since version 4: `separate()` closes the current group. The following regions do not merge
  with the previous ones, even when overlapping, and are drawn on top (a submenu on its menu:
  in `menubar.md` they are two panels, each with its own edge and the submenu's shadow on the
  menu). `clear` goes back to the first group. `add_tip(x, y, width, height, material,
  opacity)`: a tip pointing down, with the base on the top side of the rectangle and the
  rounded apex at the center of the bottom side, which merges with the body of its group into
  a single shape: the edge runs continuously from the body to the tip (the tip of the Dock's
  menus, `misc.md`). `silica_ui` sends them with `Glass.group` and `Glass.tip`; an embedder
  bound to an earlier version ignores groups and tips.
- Since version 5: `add_upward_tip(...)`, the same tip upside down (base on the bottom side,
  apex at the top): the arrangement popover below the green button. `GlassTip.up` in
  `silica_ui`.

The current regions are read with `glass.list` (`docs/control.md`).

## silica-shell-v1 and the window list

Reserved for the shell components: processes with `SILICA_SHELL=1` in the environment, which
Ossido sets when it launches them (`--spawn-shell`, `app.launch` with `shell: true`). Ossido
reads it on connection (pid from `SO_PEERCRED`, environment from `/proc`). It is not a barrier
against other processes of the same user: that will come with the app sandbox.

- `ext-foreign-toplevel-list-v1` (standard): the open windows, with title, app id and an
  identifier.
- `silica_shell_manager_v1.activate(identifier)`: focus to the window (and brings it back if
  it is minimized).
- `silica_shell_manager_v1.launch(command)`: starts an app as a child of Ossido.
- `silica_shell_manager_v1.execute(id)`: runs a catalog command without parameters
  (the bar's magnifier opens Faro with `faro.toggle`).
- event `silica_shell_manager_v1.command(id)`: a catalog command that belongs to the
  shell (`faro.toggle`, Logo+Space), forwarded to all the components.
- event `silica_shell_manager_v1.focused(identifier)`: the focused window (empty if none),
  on connection and at every change.
- event `silica_shell_manager_v1.window_menu(identifier, x, y)` (version 3): long press on
  the green button (provisional: 0.8 s, W-08); the menu bar opens the arrangement popover with
  the tip at x, y, and the entries run `window.tile_*`, `window.arrange_*` and
  `window.fullscreen` on the focused window.
- `silica_shell_manager_v1.broadcast(topic, value)` and the event of the same name
  (version 4): a component's state for the others (`controlcenter.open`,
  `notifications.open`: the capsules in the bar). Ossido sends it back to everyone, sender
  included, and sends the last value of every topic to whoever connects later.
- `silica_shell_manager_v1.execute_with(id, params)` (version 5): like `execute`, with the
  parameters in a JSON object (screenshots: `screenshot.region`, `screenshot.window_at`).
  Ossido announces every screenshot with the broadcast `screenshot` = "<path> <w> <h>".

## Drag between apps (`wl_data_device`)

Standard protocol, no extension. Ossido draws the drag icon (`start_drag`) with its corner on
the pointer, above everything, until the drop.

The embedder offers it to the apps on the `silica/drag` channel (JSONMethodCodec, `SilicaDrag`
in silica_ui ≥ 0.16):

- `start {uris, icon?: {width, height, rgba}}` from the app: a `text/uri-list` source
  (copy and move actions) with the serial of the last press. When Ossido takes the pointer,
  the Flutter gesture in progress receives a cancellation.
- Towards the app: `enter`/`motion {x, y}` and `leave` for drags with `text/uri-list`
  over the window (accepted, copy preferred), `drop {x, y, uris}` with the data already read
  (on a thread: the source may be the window itself), `finished {action}` for those that left
  the window.
