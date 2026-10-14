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
  search) and `regular` (the lens glass of the controls, like the Control Center modules:
  a strong lens that near the edge folds in the background from well inside), from the measurements
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
- Since version 6: `add_side_tip(x, y, width, height, side, material, opacity)`, the tip
  turned sideways: `left` has the apex at the middle of the left side and the base on the
  right one, `right` the other way round; the base is the rectangle's height, the depth its
  width. `silica_ui` sends `tip-left` and `tip-right`.

The tip is a rounded isosceles triangle: the rectangle's base side is the triangle's base, its
depth is where the rounded apex (radius ~2.5 pt) ends, and the sides join the body with a
~2 pt fillet. The Dock menu's measured tip is 24 × 10 pt (`misc.md`; widths per row 23, 18,
16, 15, 14, 12, 10, 8, 6, 2).

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
- `silica_shell_manager_v1.set_transient(surface, region, press, scroll)`,
  `clear_transient(surface)` and the event `dismissed(surface)` (version 6, #149): a menu or
  panel of a shell surface is open. The surface keeps an input region as small as the menu,
  so the windows below keep the pointer (hover, window button glyphs). A press outside every
  open transient (not on the owner surface or its popups, not inside `region`) makes Ossido
  send `dismissed`; then the press is consumed (`consume`) or also delivered
  (`pass_through`). Scrolling outside is dropped or delivered, and never ends it. Keyboard
  routing stays with the layer surface's keyboard interactivity.
- `set_owned_transient(surface, region, owner, owner_rule, other_rule)` (version 7): the same
  transient with the outside split in two targets, the surfaces of the owner app and every
  other surface, each with a `rule`:
  - `modal`: no hover (the pointer leaves those surfaces: no tooltips, no text cursor), no
    wheel; a press of any button ends the transient and stops there;
  - `pass`: hover and wheel work; a press ends the transient and is delivered as with nothing
    open (activation and the click's own effect, a right click opens that app's menu, a drag
    selects);
  - `activate`: hover and wheel work; a left press ends the transient and only activates the
    window under the pointer (focus and raise, or the desktop's owner on the desktop): the
    client sees neither the press nor its drag. Other buttons pass.

  `owner` is an app id (its windows with their popups and subsurfaces, and its desktop
  surface `desktop-icons:<app id>`), `desktop-icons` for whichever app owns the desktop
  surface, `desktop-icons:<app id>` for that app, or `""` for the system (every surface follows
  `other_rule`). With several transients open, the strictest outcome of those a press ends
  wins (modal, then activate, then pass). The wheel never ends a transient, and while one is
  open Ossido shows the arrow over every surface outside it: the window below does not have
  the keyboard, so no text cursor.

  The rules reproduce the reference's measured model (INPUT-01): menus are modal only for
  their owning app, system panels never block other windows, the wheel never closes
  anything. Keys always go to the open surface (menus type-select, Faro and the launcher type
  into their field, Control Center, status menus and Notification Center swallow them),
  through the layer's keyboard interactivity.

  | Shell surface | Owner | Owner rule | Other rule |
  | --- | --- | --- | --- |
  | Menu-bar app menus | the frontmost app | `modal` | `pass` |
  | Logo menu | system (`""`) | — | `pass` |
  | Desktop menu | `desktop-icons` | `modal` | `pass` |
  | Dock icon menu and stacks | the app that was active | `activate` | `pass` |
  | Faro, launcher, Control Center, status menus (Wi-Fi, battery…), Notification Center | system | — | `pass` |
  | Window arrangement popover | the frontmost app | `pass` | `pass` |

  The Dock menu's `activate` stands for the reference's "a click outside is delivered as to an
  inactive window": text views there only activate (the caret does not move), while views
  that accept the first click act on it. Ossido cannot tell the two apart, and in Silica a
  click on an inactive window always reaches it, so only the app that was active (the one
  whose caret would move) gets the focus-only activation; other apps get the click.

  App popups with `xdg_popup.grab` (third-party toolkits' context menus and popovers) are
  menus of their client: `modal` for every surface of that client except the popup and its
  sibling popups on the same window, `pass` for everything else; the popup has the keyboard
  while open and gets `popup_done` when a press ends it.

  The shell keeps the policies in one place (`TransientPolicy` in
  `shell/lib/system/layer_control.dart`: `menu`, `dockMenu`, `panel`, `popover`) and the
  embedder exposes them on `silica/layer` (`setTransient {rects, owner, ownerRule,
  otherRule, press, scroll}`, where `press` and `scroll` are the version 6 fallback;
  `clearTransient`) and `silica/shell` (`transientDismissed`).

## Drag between apps (`wl_data_device`)

Standard protocol, no extension. Ossido draws the drag icon (`start_drag`) with its corner on
the pointer, moved by the offsets of its commits (`wl_surface.attach` dx/dy or
`wl_surface.offset`, added up), above everything, until the drop.

The embedder offers it to the apps on the `silica/drag` channel (JSONMethodCodec, `SilicaDrag`
in silica_ui ≥ 0.16):

- `start {uris, icon?: {width, height, rgba, x?, y?}}` from the app: a `text/uri-list` source
  (copy and move actions) with the serial of the last press. The icon's point (`x`, `y`) stays
  under the pointer (the corner without them): an image of the dragged items keeps where the
  pointer grabbed them. At most 1024 pixels per side. When Ossido takes the pointer,
  the Flutter gesture in progress receives a cancellation.
- Towards the app: `enter`/`motion {x, y}` and `leave` for drags with `text/uri-list`
  over the window (accepted, copy preferred), `drop {x, y, uris}` with the data already read
  (on a thread: the source may be the window itself), `finished {action}` for those that left
  the window.

## Pseudo-terminals (`silica/pty`)

The embedder runs processes on pseudo-terminals for apps (the Terminal app), so apps need no
native code (`crates/embedder/src/pty.rs`; the one unsafe step, making the slave the
controlling terminal between fork and exec, is in `ffi/pty.rs`). JSONMethodCodec:

- From the app: `spawn {command?: [argv…], cwd?, env?: {k: v}, rows, cols}` → `{id}` (without
  `command`, the user's login shell from `$SHELL` or the passwd entry, as a login shell;
  `TERM=xterm-256color`, `COLORTERM=truecolor`, then `env`; `cwd` defaults to `$HOME`);
  `write {id, data: base64}`, `resize {id, rows, cols, pixelWidth?, pixelHeight?}`,
  `signal {id, signal: "INT"|"TERM"|"HUP"|"KILL"}`, `close {id}` (SIGHUP, SIGKILL after 2 s,
  reaped; no `exit` is sent) → null. Errors come back as a PlatformException with code `pty`.
- To the app: `output {id, data: base64}`, coalesced, at most 64 KiB and one message per frame
  per pty; `exit {id, code?, signal?}` after the last output.
- Limits: 64 ptys per window, rows and columns 1–1000, 256 arguments and environment
  variables of at most 4 KiB each. More than 1 MiB of output waiting for the app pauses
  reading from the pty (backpressure); more than 4 MiB of input waiting refuses `write`.
  Closing the window hangs up every pty.
