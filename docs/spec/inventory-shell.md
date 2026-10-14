# Shell inventory

Every feature of the shell seen on the reference, with its state in Silica (INV-01). The ID
is stable and is the same as in the measurements and the issues.

- **Silica:**
  - `done`: implemented.
  - `partial`: part of it, or some states, are missing.
  - `missing`: not started.

  The linked issue tracks the work, and a closed issue counts as done.
- **Spec:** the file in this directory that holds the item's measurements. A "—" means the
  item is specified in another file, or not measured yet.

The table is generated from the inventory and the issue tracker on 2026-10-14. The issue
tracker is the live source.

## Menu bar and app menus

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-MB-01` | Menu bar (bar, text, logo, height) | done | [menubar.md](menubar.md) |
| `SH-MB-02` | Logo menu | done ([#20](https://github.com/silica-linux/silica-desktop/issues/20)) | [menubar.md](menubar.md) |
| `SH-MB-03` | Logo menu submenu | done ([#21](https://github.com/silica-linux/silica-desktop/issues/21)) | [menubar.md](menubar.md) |
| `SH-MB-04` | Logo menu with ⌥ held (variants) | done ([#22](https://github.com/silica-linux/silica-desktop/issues/22)) | [menubar.md](menubar.md) |
| `SH-MB-05` | Shut Down / Restart / Log Out confirmation window (countdown) | missing ([#23](https://github.com/silica-linux/silica-desktop/issues/23)) | [menubar.md](menubar.md) |
| `SH-MB-06` | Mustache app menus (File, Edit, View, Go, Window, Help) | partial ([#24](https://github.com/silica-linux/silica-desktop/issues/24)) | [menubar.md](menubar.md) |
| `SH-MB-07` | Menus of other apps (Calculator, Clock, Terminal) | partial ([#25](https://github.com/silica-linux/silica-desktop/issues/25)) | [menubar.md](menubar.md) |
| `SH-MB-08` | Open menu: geometry, border, radius, shadow, fill, text, separators | done | [menubar.md](menubar.md) |
| `SH-MB-09` | Menu item highlight (capsule in accent color) | done | [menubar.md](menubar.md) |
| `SH-MB-10` | Menu icons, checkmark, submenu arrow, shortcut glyph column | partial ([#26](https://github.com/silica-linux/silica-desktop/issues/26)) | [menubar.md](menubar.md) |
| `SH-MB-11` | Menu material (blur, color, refraction) | done | [menubar.md](menubar.md) |
| `SH-MB-12` | Menu behaviors: open on press, drag-select, switching between items, submenu delay and triangle, keyboard, type-ahead, closing fade | partial ([#27](https://github.com/silica-linux/silica-desktop/issues/27)) | [menubar.md](menubar.md) |
| `SH-MB-13` | Menu near the screen edge: off-screen, submenu on the left, scroll arrows | missing ([#28](https://github.com/silica-linux/silica-desktop/issues/28)) | [menubar.md](menubar.md) |
| `SH-MB-14` | Help menu search field | missing ([#29](https://github.com/silica-linux/silica-desktop/issues/29)) | [menubar.md](menubar.md) |
| `SH-MB-15` | Desktop context menu | missing ([#30](https://github.com/silica-linux/silica-desktop/issues/30)) | [menubar.md](menubar.md) |

## Menu bar status items

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-MB-20` | Status area layout (icon boxes, ink sizes, spacing, 90 % white color) | done | [menubar.md](menubar.md) |
| `SH-MB-21` | Capsule highlight of the open status/app item | done | [menubar.md](menubar.md) |
| `SH-MB-22` | Wi-Fi status item and panel | done | [menubar.md](menubar.md) |
| `SH-MB-23` | Input source status item and menu | missing ([#31](https://github.com/silica-linux/silica-desktop/issues/31)) | [menubar.md](menubar.md) |
| `SH-MB-24` | Bluetooth status item and menu | missing ([#32](https://github.com/silica-linux/silica-desktop/issues/32)) | [menubar.md](menubar.md) |
| `SH-MB-25` | Sound status item and menu | missing ([#33](https://github.com/silica-linux/silica-desktop/issues/33)) | [menubar.md](menubar.md) |
| `SH-MB-26` | Focus status item and menu | done ([#34](https://github.com/silica-linux/silica-desktop/issues/34)) | [menubar.md](menubar.md) |
| `SH-MB-27` | Battery status item and menu | partial ([#35](https://github.com/silica-linux/silica-desktop/issues/35)) | [menubar.md](menubar.md) |
| `SH-MB-28` | Faro status item | done | [menubar.md](menubar.md) |
| `SH-MB-29` | Control Center status item | done | [menubar.md](menubar.md) |
| `SH-MB-30` | Clock / date item | done | [menubar.md](menubar.md) |
| `SH-MB-31` | Voice assistant status item | missing ([#36](https://github.com/silica-linux/silica-desktop/issues/36)) | [menubar.md](menubar.md) |
| `SH-MB-32` | Screen Mirroring / display status item | missing ([#37](https://github.com/silica-linux/silica-desktop/issues/37)) | [menubar.md](menubar.md) |
| `SH-MB-33` | Now Playing status item | done ([#38](https://github.com/silica-linux/silica-desktop/issues/38)) | [menubar.md](menubar.md) |
| `SH-MB-34` | Backup, VPN, fast user switching, Accessibility shortcuts status items | missing ([#39](https://github.com/silica-linux/silica-desktop/issues/39)) | [menubar.md](menubar.md) |
| `SH-MB-35` | Third-party status items (menu, badge, dot) | missing ([#40](https://github.com/silica-linux/silica-desktop/issues/40)) | [menubar.md](menubar.md) |
| `SH-MB-36` | Purple/blue dot right of the clock (recording/privacy indicator, hypothesis) | missing ([#41](https://github.com/silica-linux/silica-desktop/issues/41)) | [menubar.md](menubar.md) |
| `SH-MB-37` | Many status items: overflow, ⌘-drag reorder | missing ([#42](https://github.com/silica-linux/silica-desktop/issues/42)) | [menubar.md](menubar.md) |
| `SH-MB-38` | Menu bar in full screen (reveal at top edge) and auto-hide bar | missing ([#43](https://github.com/silica-linux/silica-desktop/issues/43)) | [menubar.md](menubar.md) |

## Control Center

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-CC-01` | Stack layout (grid, spacing, margins, with and without privacy pill) | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-02` | Module material (shape, luminous border, fill, stack shadow) | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-03` | Wi-Fi module (140 × 64) | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-04` | Bluetooth module | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-05` | Nearby sharing module | missing ([#44](https://github.com/silica-linux/silica-desktop/issues/44)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-06` | Focus module | done ([#45](https://github.com/silica-linux/silica-desktop/issues/45)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-07` | Window grouping toggle (64 × 64) | missing ([#46](https://github.com/silica-linux/silica-desktop/issues/46)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-08` | Mirror display toggle (64 × 64) | missing ([#47](https://github.com/silica-linux/silica-desktop/issues/47)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-09` | Dark mode toggle (64 × 64) | done ([#48](https://github.com/silica-linux/silica-desktop/issues/48)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-10` | Screen capture toggle (64 × 64) | missing ([#49](https://github.com/silica-linux/silica-desktop/issues/49)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-11` | Now Playing media module (140 × 140) and transport buttons | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-12` | Display module with brightness slider (292 × 64) | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-13` | Sound module with volume slider (292 × 64) | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-14` | Wi-Fi detail pane | partial ([#50](https://github.com/silica-linux/silica-desktop/issues/50)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-15` | Display detail pane | partial ([#51](https://github.com/silica-linux/silica-desktop/issues/51)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-16` | Sound detail pane | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-17` | Bluetooth detail pane | done ([#52](https://github.com/silica-linux/silica-desktop/issues/52)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-18` | Nearby sharing detail pane | missing ([#53](https://github.com/silica-linux/silica-desktop/issues/53)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-19` | Focus detail pane | done ([#54](https://github.com/silica-linux/silica-desktop/issues/54)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-20` | Now Playing detail pane | done ([#55](https://github.com/silica-linux/silica-desktop/issues/55)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-21` | Edit Controls: gallery panel | missing ([#56](https://github.com/silica-linux/silica-desktop/issues/56)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-22` | Edit Controls: Center in edit mode (remove badges, empty slots, dimming) | missing ([#57](https://github.com/silica-linux/silica-desktop/issues/57)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-23` | Edit Controls: drag to add, remove, move, resize; "done" after a change | missing ([#58](https://github.com/silica-linux/silica-desktop/issues/58)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-24` | Module context menu (size, remove from bar) | missing ([#59](https://github.com/silica-linux/silica-desktop/issues/59)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-25` | Privacy pill | missing ([#60](https://github.com/silica-linux/silica-desktop/issues/60)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-26` | Open / close and module-to-detail animations | done | [controlcenter.md](controlcenter.md) |
| `SH-CC-27` | Other modules offered by the gallery (battery, keyboard brightness, accessibility shortcuts, hearing, music recognition, shortcuts, user switching, …) | partial ([#61](https://github.com/silica-linux/silica-desktop/issues/61)) | [controlcenter.md](controlcenter.md) |
| `SH-CC-28` | Volume HUD | done | [hud.md](hud.md) |
| `SH-CC-29` | Brightness HUD | done | [hud.md](hud.md) |
| `SH-CC-30` | Keyboard brightness HUD | missing ([#62](https://github.com/silica-linux/silica-desktop/issues/62)) | [hud.md](hud.md) |

## Notification Center and banners

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-NC-01` | Banner: geometry, texts, material, shadow | done | [notifications.md](notifications.md) |
| `SH-NC-02` | Banner lifecycle (entrance, dwell, exit, replacement) | done | [notifications.md](notifications.md) |
| `SH-NC-03` | Banner interactions (hover, click, drag) | done | [notifications.md](notifications.md) |
| `SH-NC-04` | Banner with actions, inline reply, long press | partial ([#63](https://github.com/silica-linux/silica-desktop/issues/63)) | [notifications.md](notifications.md) |
| `SH-NC-05` | Alerts style (stay until action) | partial ([#64](https://github.com/silica-linux/silica-desktop/issues/64)) | [notifications.md](notifications.md) |
| `SH-NC-06` | Banners from different apps | done | [notifications.md](notifications.md) |
| `SH-NC-07` | Banner with Focus active | partial ([#65](https://github.com/silica-linux/silica-desktop/issues/65)) | [notifications.md](notifications.md) |
| `SH-NC-08` | Notification Center panel (layout, scrim, stacked cards, second notification with actions) | done | [notifications.md](notifications.md) |
| `SH-NC-09` | Notification groups by app, expanding and collapsing | done | [notifications.md](notifications.md) |
| `SH-NC-10` | Clear button (×) in Notification Center | done | [notifications.md](notifications.md) |
| `SH-NC-11` | Empty state | done | [notifications.md](notifications.md) |
| `SH-NC-12` | Scrolling with 20+ notifications, dragging out | missing ([#66](https://github.com/silica-linux/silica-desktop/issues/66)) | [notifications.md](notifications.md) |
| `SH-NC-13` | "choose widgets" / edit widgets pill | missing ([#67](https://github.com/silica-linux/silica-desktop/issues/67)) | [notifications.md](notifications.md) |
| `SH-NC-14` | Widget gallery (add, remove, sizes) | missing ([#68](https://github.com/silica-linux/silica-desktop/issues/68)) | [notifications.md](notifications.md) |
| `SH-NC-15` | Widgets in Notification Center (calendar, weather, battery…) | missing ([#69](https://github.com/silica-linux/silica-desktop/issues/69)) | [notifications.md](notifications.md) |
| `SH-NC-16` | Desktop widgets (placement, styles, inactive look) | missing ([#70](https://github.com/silica-linux/silica-desktop/issues/70)) | [notifications.md](notifications.md) |

## Dock

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-DK-01` | Dock geometry at rest, background, indicators | done | [dock.md](dock.md) |
| `SH-DK-02` | Setting: Size | missing ([#71](https://github.com/silica-linux/silica-desktop/issues/71)) | [dock.md](dock.md) |
| `SH-DK-03` | Setting: Magnification (profile, lateral expansion, active zone) | done | [dock.md](dock.md) |
| `SH-DK-04` | Setting: Position, bottom | done | [dock.md](dock.md) |
| `SH-DK-05` | Setting: Position, left | missing ([#72](https://github.com/silica-linux/silica-desktop/issues/72)) | [dock.md](dock.md) |
| `SH-DK-06` | Setting: Position, right | missing ([#73](https://github.com/silica-linux/silica-desktop/issues/73)) | [dock.md](dock.md) |
| `SH-DK-07` | Setting: Automatically hide and show | missing ([#74](https://github.com/silica-linux/silica-desktop/issues/74)) | [dock.md](dock.md) |
| `SH-DK-08` | Setting: Minimize effect, Genie | done | [dock.md](dock.md) |
| `SH-DK-09` | Setting: Minimize effect, Scale | missing ([#75](https://github.com/silica-linux/silica-desktop/issues/75)) | [dock.md](dock.md) |
| `SH-DK-10` | Setting: Minimize windows into the application icon | missing ([#76](https://github.com/silica-linux/silica-desktop/issues/76)) | [dock.md](dock.md) |
| `SH-DK-11` | Setting: Animate opening applications (bounce) | done | [dock.md](dock.md) |
| `SH-DK-12` | Setting: Show indicators for open applications | partial ([#77](https://github.com/silica-linux/silica-desktop/issues/77)) | [dock.md](dock.md) |
| `SH-DK-13` | Setting: Show suggested and recent apps | done | [dock.md](dock.md) |
| `SH-DK-14` | Hover label | done | [dock.md](dock.md) |
| `SH-DK-15` | Click on a non-running app (bounce, ⌘/⌥ click) | partial ([#78](https://github.com/silica-linux/silica-desktop/issues/78)) | [dock.md](dock.md) |
| `SH-DK-16` | Click on a running app | partial ([#79](https://github.com/silica-linux/silica-desktop/issues/79)) | [dock.md](dock.md) |
| `SH-DK-17` | Context menu: running app / pinned app / app with documents | partial ([#80](https://github.com/silica-linux/silica-desktop/issues/80)) | [dock.md](dock.md) |
| `SH-DK-18` | Context menu: separator (Dock options) | partial ([#81](https://github.com/silica-linux/silica-desktop/issues/81)) | [dock.md](dock.md) |
| `SH-DK-19` | Context menu: Downloads / folder and minimized window | done | [dock.md](dock.md) |
| `SH-DK-20` | Trash: context menu, empty confirmation, empty/full icon | done | [dock.md](dock.md) |
| `SH-DK-21` | Dragging icons (reorder gap, removal, add, file on app / Trash) | partial ([#82](https://github.com/silica-linux/silica-desktop/issues/82)) | [dock.md](dock.md) |
| `SH-DK-22` | Folder stack, Fan view | done | [dock-stacks.md](dock-stacks.md) |
| `SH-DK-23` | Folder stack, Grid view | done | [dock-stacks.md](dock-stacks.md) |
| `SH-DK-24` | Folder stack, List view | done | [dock-stacks.md](dock-stacks.md) |
| `SH-DK-25` | Folder stack, Automatic view and stack context menu | done | [dock-stacks.md](dock-stacks.md) |
| `SH-DK-26` | Stack close button and dark appearance | done | [dock-stacks.md](dock-stacks.md) |
| `SH-DK-27` | Stack sorting (date, kind), dragging a file out, scrolling a full grid | partial ([#83](https://github.com/silica-linux/silica-desktop/issues/83)) | [dock-stacks.md](dock-stacks.md) |
| `SH-DK-28` | Minimized windows and recent apps in the Dock | done | [dock.md](dock.md) |
| `SH-DK-29` | Badges (1, 2, 3 digits, 99+) | missing ([#84](https://github.com/silica-linux/silica-desktop/issues/84)) | [dock.md](dock.md) |
| `SH-DK-30` | Download progress and attention bounce | missing ([#85](https://github.com/silica-linux/silica-desktop/issues/85)) | [dock.md](dock.md) |
| `SH-DK-31` | Continuity item | missing ([#86](https://github.com/silica-linux/silica-desktop/issues/86)) | [dock.md](dock.md) |
| `SH-DK-32` | Dock with full-screen windows and multiple monitors | partial ([#87](https://github.com/silica-linux/silica-desktop/issues/87)) | [dock.md](dock.md) |

## Faro

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-SP-01` | Panel: position, size, background, border, shadow | done | [faro.md](faro.md) |
| `SH-SP-02` | Search field and inline completion | done | [faro.md](faro.md) |
| `SH-SP-03` | First-launch panel (suggestions, privacy notice, continue button) | missing ([#88](https://github.com/silica-linux/silica-desktop/issues/88)) | [faro.md](faro.md) |
| `SH-SP-04` | Steady-state empty panel (after continue button) | done | [faro.md](faro.md) |
| `SH-SP-05` | Four circular buttons (apps, files, actions, clipboard) | partial ([#89](https://github.com/silica-linux/silica-desktop/issues/89)) | [faro.md](faro.md) |
| `SH-SP-06` | Result kind: applications | done | [faro.md](faro.md) |
| `SH-SP-07` | Result kind: documents and files (thumbnail with badge) | missing ([#90](https://github.com/silica-linux/silica-desktop/issues/90)) | [faro.md](faro.md) |
| `SH-SP-08` | Result kind: folders | done ([#91](https://github.com/silica-linux/silica-desktop/issues/91)) | [faro.md](faro.md) |
| `SH-SP-09` | Result kind: mail, messages, contacts, calendar | missing ([#92](https://github.com/silica-linux/silica-desktop/issues/92)) | [faro.md](faro.md) |
| `SH-SP-10` | Result kind: web / browser suggestions, knowledge answers, definitions | missing ([#93](https://github.com/silica-linux/silica-desktop/issues/93)) | [faro.md](faro.md) |
| `SH-SP-11` | Result kind: System Settings panes, images, music, events | missing ([#94](https://github.com/silica-linux/silica-desktop/issues/94)) | [faro.md](faro.md) |
| `SH-SP-12` | Filter bar (pills) and "search in …" chip | missing ([#95](https://github.com/silica-linux/silica-desktop/issues/95)) | [faro.md](faro.md) |
| `SH-SP-13` | Calculations | done ([#96](https://github.com/silica-linux/silica-desktop/issues/96)) | [faro.md](faro.md) |
| `SH-SP-14` | Unit and currency conversions | missing ([#97](https://github.com/silica-linux/silica-desktop/issues/97)) | [faro.md](faro.md) |
| `SH-SP-15` | Actions and quick commands | missing ([#98](https://github.com/silica-linux/silica-desktop/issues/98)) | [faro.md](faro.md) |
| `SH-SP-16` | Clipboard history | missing ([#99](https://github.com/silica-linux/silica-desktop/issues/99)) | [faro.md](faro.md) |
| `SH-SP-17` | Keyboard: ↑ ↓ ⏎ ⌘⏎ Tab, Esc | partial ([#100](https://github.com/silica-linux/silica-desktop/issues/100)) | [faro.md](faro.md) |
| `SH-SP-18` | No results / error state | done ([#101](https://github.com/silica-linux/silica-desktop/issues/101)) | [faro.md](faro.md) |
| `SH-SP-19` | Opening animation | done | [faro.md](faro.md) |
| `SH-SP-20` | Closing and typing animations | missing ([#102](https://github.com/silica-linux/silica-desktop/issues/102)) | [faro.md](faro.md) |

## Launcher, Panorama, spaces, app windows view

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-LP-01` | Apps launcher: panel, header, search field | done | [launcher.md](launcher.md) |
| `SH-LP-02` | Launcher: category capsules and token in the field | done | [launcher.md](launcher.md) |
| `SH-LP-03` | Launcher: icon grid, hover, keyboard selection | partial ([#103](https://github.com/silica-linux/silica-desktop/issues/103)) | [launcher.md](launcher.md) |
| `SH-LP-04` | Launcher: scrollbar | done | [launcher.md](launcher.md) |
| `SH-LP-05` | Launcher: search and results | done | [launcher.md](launcher.md) |
| `SH-LP-06` | Launcher: animations (open, category change, search, Esc) | partial ([#104](https://github.com/silica-linux/silica-desktop/issues/104)) | [launcher.md](launcher.md) |
| `SH-LP-07` | Launcher: drag icons into folders / reorder, delete | missing ([#105](https://github.com/silica-linux/silica-desktop/issues/105)) | [launcher.md](launcher.md) |
| `SH-MC-01` | Panorama desktop (windows layout) | done | [panorama.md](panorama.md) |
| `SH-MC-02` | Spaces bar compact and expanded | done | [panorama.md](panorama.md) |
| `SH-MC-03` | Panorama animations (open, close, curves) | done | [panorama.md](panorama.md) |
| `SH-MC-04` | Thumbnails: hover and click; keyboard | done | [panorama.md](panorama.md) |
| `SH-MC-05` | Spaces: switching ⌃→ / ⌃←, bounce at the edge | done | [panorama.md](panorama.md) |
| `SH-MC-06` | Spaces: add (+) and remove (×) | done | [panorama.md](panorama.md) |
| `SH-MC-07` | Full-screen app as a space; dragging a window to the bar; moving to an existing space | partial ([#106](https://github.com/silica-linux/silica-desktop/issues/106)) | [panorama.md](panorama.md) |
| `SH-MC-08` | Panorama with two displays | partial ([#107](https://github.com/silica-linux/silica-desktop/issues/107)) | [panorama.md](panorama.md) |
| `SH-MC-09` | App windows view | done | [panorama.md](panorama.md) |
| `SH-MC-10` | Show Desktop | done | [panorama.md](panorama.md) |
| `SH-MC-11` | Reduce Motion and dark in Panorama | partial ([#108](https://github.com/silica-linux/silica-desktop/issues/108)) | [panorama.md](panorama.md) |
| `SH-MC-12` | Notification Center / Panorama gestures (trackpad) | partial ([#109](https://github.com/silica-linux/silica-desktop/issues/109)) | [panorama.md](panorama.md) |

## Window grouping, hot corners, app switcher

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-SM-01` | Window grouping (strip with one thumbnail) | missing ([#110](https://github.com/silica-linux/silica-desktop/issues/110)) | [stage-view.md](stage-view.md) |
| `SH-SM-02` | Window grouping: several groups, switching, animation, thumbnail tilt | missing ([#111](https://github.com/silica-linux/silica-desktop/issues/111)) | [stage-view.md](stage-view.md) |
| `SH-HC-01` | Hot corners (delay, repeat visit) | missing ([#112](https://github.com/silica-linux/silica-desktop/issues/112)) | [panorama.md](panorama.md) |
| `SH-HC-02` | Hot corner actions: Notification Center, Launcher, quick note, lock screen, screen saver | missing ([#113](https://github.com/silica-linux/silica-desktop/issues/113)) | [panorama.md](panorama.md) |
| `SH-AS-01` | App switcher ⌘⇥: panel, icons, selection | done | [app-switcher.md](app-switcher.md) |
| `SH-AS-02` | App switcher: many apps, actions inside (Q, H, arrows, ⌘`) | partial ([#114](https://github.com/silica-linux/silica-desktop/issues/114)) | [app-switcher.md](app-switcher.md) |

## Lock, login, screen saver

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-LK-01` | Lock screen | missing ([#115](https://github.com/silica-linux/silica-desktop/issues/115)) | [misc.md](misc.md) |
| `SH-LK-02` | Login window (several users) | missing ([#116](https://github.com/silica-linux/silica-desktop/issues/116)) | [misc.md](misc.md) |
| `SH-LK-03` | Wrong-password shake, unlock animation | missing ([#117](https://github.com/silica-linux/silica-desktop/issues/117)) | [misc.md](misc.md) |
| `SH-LK-04` | Screen saver | missing ([#118](https://github.com/silica-linux/silica-desktop/issues/118)) | [misc.md](misc.md) |

## Focus, night light, screenshots, Force Quit, About

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-FO-01` | Focus modes (Do Not Disturb on: moon icon, white disc) | partial ([#119](https://github.com/silica-linux/silica-desktop/issues/119)) | [misc.md](misc.md) |
| `SH-FO-02` | Focus: other modes, schedules, menu bar status | missing ([#120](https://github.com/silica-linux/silica-desktop/issues/120)) | [misc.md](misc.md) |
| `SH-NS-01` | Night light tile in the Display detail | missing ([#121](https://github.com/silica-linux/silica-desktop/issues/121)) | [misc.md](misc.md) |
| `SH-SS-01` | ⇧⌘3 screenshot and thumbnail | done | [screenshot-ui.md](screenshot-ui.md) |
| `SH-SS-02` | ⇧⌘4 selection and window (Space) with tint | done | [screenshot-ui.md](screenshot-ui.md) |
| `SH-SS-03` | ⇧⌘5 toolbar and options menu | done | [screenshot-ui.md](screenshot-ui.md) |
| `SH-SS-04` | Screen recording and its menu bar control | missing ([#122](https://github.com/silica-linux/silica-desktop/issues/122)) | [screenshot-ui.md](screenshot-ui.md) |
| `SH-SS-05` | Screenshot editor (thumbnail click) | missing ([#123](https://github.com/silica-linux/silica-desktop/issues/123)) | [screenshot-ui.md](screenshot-ui.md) |
| `SH-FQ-01` | Force Quit window | done | [screenshot-ui.md](screenshot-ui.md) |
| `SH-AB-01` | About this computer | missing ([#124](https://github.com/silica-linux/silica-desktop/issues/124)) | [misc.md](misc.md) |
| `SH-EM-01` | Characters and emoji (⌃⌘Space) | missing ([#125](https://github.com/silica-linux/silica-desktop/issues/125)) | [screenshot-ui.md](screenshot-ui.md) |

## Keyboard shortcuts catalogue

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-KS-01` | System shortcuts from Settings (Dock, Screen, Panorama, Keyboard, Input sources, Screenshots, Services, Faro, Accessibility, App, Overlay mode, Window management) | partial ([#126](https://github.com/silica-linux/silica-desktop/issues/126)) | [shortcuts.md](shortcuts.md) |
| `SH-KS-02` | Mustache menu shortcuts | partial ([#127](https://github.com/silica-linux/silica-desktop/issues/127)) | [shortcuts.md](shortcuts.md) |
| `SH-KS-03` | Known shortcuts not in Settings (⌘⇥, ⌘Space, ⌃⌘Q, 🌐, media keys, ⇧⌘3/4/5) | partial ([#128](https://github.com/silica-linux/silica-desktop/issues/128)) | [shortcuts.md](shortcuts.md) |
| `SH-KS-04` | Function keys with the fn flag, key repeat in ms | missing ([#129](https://github.com/silica-linux/silica-desktop/issues/129)) | [shortcuts.md](shortcuts.md) |

## Accessibility shell features

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-AX-01` | Reduce Transparency | partial ([#130](https://github.com/silica-linux/silica-desktop/issues/130)) | [misc.md](misc.md) |
| `SH-AX-02` | Increase Contrast | missing ([#131](https://github.com/silica-linux/silica-desktop/issues/131)) | [misc.md](misc.md) |
| `SH-AX-03` | Reduce Motion | missing ([#132](https://github.com/silica-linux/silica-desktop/issues/132)) | [misc.md](misc.md) |
| `SH-AX-04` | Zoom | missing ([#133](https://github.com/silica-linux/silica-desktop/issues/133)) | [misc.md](misc.md) |
| `SH-AX-05` | Screen reader presence (menu bar item, cursor, panel) | missing ([#134](https://github.com/silica-linux/silica-desktop/issues/134)) | [misc.md](misc.md) |
| `SH-AX-06` | Accessibility shortcuts panel | missing ([#135](https://github.com/silica-linux/silica-desktop/issues/135)) | [misc.md](misc.md) |
| `SH-AX-07` | Always show scroll bars; sidebar text size; accent and highlight colors | missing ([#136](https://github.com/silica-linux/silica-desktop/issues/136)) | [misc.md](misc.md) |
| `SH-AX-08` | Show borders (accessibility) | not tracked | [misc.md](misc.md) |

## Windows and dialogs seen from the shell

| ID | Item | Silica | Spec |
|---|---|---|---|
| `SH-WN-01` | Window frame, corners, shadow (active, inactive, full screen) | done | [windows.md](windows.md) |
| `SH-WN-02` | Window buttons (traffic lights) all states | done | [windows.md](windows.md) |
| `SH-WN-03` | Green button tiling popover | done | [windows.md](windows.md) |
| `SH-WN-04` | Window animations (open/close, minimize genie, full screen) | done | [windows.md](windows.md) |
| `SH-WN-05` | Alert dialog | done | [misc.md](misc.md) |
| `SH-WN-06` | Save card (compact, expanded), Open panel | missing ([#137](https://github.com/silica-linux/silica-desktop/issues/137)) | [dialogs.md](dialogs.md) |
