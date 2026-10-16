# Amalfi components

Every component of the UI kit, the binding layout metrics, and the two system surfaces with their own spec (Faro and Launcher).

## Layout metrics (binding)

Every frame x, y, width and height is a multiple of 4 pt. The only exception is vertical centring inside a
bar, and then you choose item heights so the centring lands on whole points: 52 − 16 = 36 → y 18;
52 − 28 → y 12; 52 − 20 → y 16.
- Window: corner 18, toolbar 52, title-only bar 32. Window controls: first dot centre x 26, pitch 23, so the
  group sits at x 18 (or x 19 with a 16 pt dot cell). The vertical centre is the bar's centre.
- Content padding: 20 (regular windows), 16 (compact windows and panels), 24 (documents and settings).
- Sidebar: width 240 (min 200), horizontal padding 12, row height 28, row pitch 32 (4 pt gap), section
  header 28 tall, 16 above a header (except the first, 8), and 8 between the last row of a section and the
  next header. The selection capsule is the row inset 0 horizontally, radius 8. Row icon 16 at x 12, label at x 36.
- Inspector: width 260, padding 16, groups separated by 20.
- Toolbar items are glass (Glass/Regular), centred at y 10:
  - single icon button 32 × 32 circle;
  - action capsules 32 tall, made of 32 pt segments with 1 × 16 separators (Back/Forward 65);
  - search capsule 32 tall;
  - 8 between capsules; trailing padding 16.
  Small 24 (segment 24, icon 14), mini 20 (segment 20, icon 12). Amalfi's own sizes: do not copy Silica's
  measured 37/38 pt values.
- Icon grids (file manager): tile 104 × 104 (icon 64, label below), gap 16, first tile at the content
  padding.
- Lists and tables: row 28 (regular), 24 (compact), 44 (two-line). Separators are 1 pt Line/Separator,
  inset 16.
- Calculator keypad: keys 48 × 48 (capsule or circle), gap 8 (pitch 56), padding 16, display height 72.
- Terminal: content padding 12, line height 16 (Mono 12/16).
- Text baselines: put text frames on whole points, with the line height from the text style.

## Component inventory (3015 symbols, every one in Light and Dark)

Names follow `Category/Component/Variant/Size/State/Appearance`. States are always
Rest · Hover · Pressed · Disabled · Focused unless listed. Sizes: Mini 20 · Small 24 · Regular 28 · Large 32.
Read each one from the kit for exact geometry (see README, "How to read the kit").

| Family | Components and variants | Sizes (pt) |
|---|---|---|
| Alerts | Alert: one / two / three buttons, critical, with field, glass | 280 wide |
| Buttons | Push: Primary, Secondary, Destructive, Borderless, Glass × 4 sizes × 5 states (200); Icon: Round, Square × 4 sizes × 5 states; Help; Link; Toolbar: Icon, Icon + Label; Gradient add/remove; Pop-up and Pull-down × 4 sizes × 5 states + Open | push 48×20 … 88×32; icon 20–32; toolbar 32×32, 56×44 |
| Color Wells | Color Well: Minimal, Default, Expanded × states; Color Panel swatch grid; Swatch Chip; Swatch Row | 44×24, 72×28, 188×28 |
| Combo Boxes | Combo Box: Small, Regular, Large × Rest/Focused/Disabled + Open; Combo Button | 200×24/28/32 |
| Control Center | Panel; Module: Toggle Off/On/On Hover, Tile, Slider (Display, Sound, Keyboard), Now Playing, Privacy Pill, Edit Button; Detail: Wi-Fi, Wi-Fi Off, Bluetooth, Nearby Sharing, Focus, Display, Sound, Now Playing; Row: Item, Network Secure/Open, Slider, Footer, Section; Status: Wi-Fi, Bluetooth, Battery; Status Menu: Battery | panel 292×484; detail 312 wide; modules 64×64, 140×64, 140×140, 292×64 |
| Dialogs | Dialog: Save Panel, Open Panel, Generic; Browser; Footer | 600×348, 600×308, 400×432 |
| Disclosure | Triangle and Button: Collapsed/Expanded × states; Group; Group Header | 24×24; group 280 |
| Dock | Shelf: Default, With Label; Tile (Rest, Running) per app + Stack, Trash Empty/Full; Separator; Indicator | shelf 528×60; tile 52×60 |
| Fields | Text Field: Placeholder, Filled, Focused, Disabled, Error × 3 sizes; Secure Field; Text Area; Token Field; Token; Search Field: Empty, Typing, Focused, Disabled × 3 sizes + Token; Inline Button; Clear Button | 200×24/28/32; area 240×88 |
| Forms | Layout; Section: Profile, Preferences, Validation; Row: Text, Popup, Check, Switch, Help, Error; Footer | 400×496 |
| Group Boxes | Box: Titled, Titled Disabled, Untitled, Nested | |
| Image Wells | Empty (Rest, Drag Over, Disabled), Filled (Rest, Focused, Disabled) × Small/Regular/Large | 72, 120, 168 |
| Menu Bar | Bar: Closed, Menu Open; Item: Status, Menu, Name, Clock × Rest/Active | 28 tall, items 24 |
| Menus | Menu: Simple, Full, Context, Submenu, Cascade; Item: Plain, Shortcut, Submenu, Icon, Destructive, Checkmark × Rest/Highlighted/Checked/Disabled; Header; Separator | items 24 tall; separator 8 |
| Notifications | Banner: Basic, With Actions, Panel, Panel With Actions; Stack Grouped; Centre Panel; App Icon | 360×76, 360×108; centre 380×416 |
| Pointers | Arrow, I-beam, Link Hand, Resize ↔ ↕ ⤡ ⤢, Crosshair, Busy, Not Allowed, Copy, Context Menu, Drag | 32×32 |
| Popovers | Popover: Form, List, Text × Top/Bottom/Leading/Trailing × Start/Center/End + Detached; Content | ~204–268 wide |
| Progress | Bar: Determinate (25/60/100/Disabled), Indeterminate (3 phases) × Small/Regular/Large; Circular; Spinner | bar 4/8 tall; spinner 16/24/32 |
| Scrollbars | Overlay: Thin, Expanded, Dragging × V/H; Legacy: Rest, Hover, Disabled × V/H; Corner; Area | overlay 12; legacy 16 |
| Segmented | Action segments (toolbar, glass): Icon 2–5, Text 2–3 × Large/Small × Rest/Hover/Pressed/Disabled/Inactive (240); Control (select one/any): 2–5 Text/Icon × Small/Regular/Large × Rest/Pressed/Disabled (176); Icon Item; Text Item; Align | segments 32 (Large), 24 (Small) |
| Sidebars | Panel; Row: Plain, Badge × Rest/Hover/Selected/Disabled; Header; Group Open/Closed; Badge; Footer | panel 240; rows 216×28 |
| Sliders | Horizontal: Plain, Ticks, Glyphs × states + Dragging; Vertical; Circular dial; Thumb: Round × states, Lens Dragging | 200×20/28/36 |
| Steppers | Stepper × 4 sizes × Rest/Hover Up/Down/Pressed Up/Down/Disabled/Focused; Value Field; With Field | 16×20 … 28×32 |
| Titlebars and toolbars | Bar: Unified, Title Only, Compact × Active/Inactive; Section: Sidebar, Content, Inspector, List; Piece (modular): Controls, Title, Title + Subtitle, Title + Document Icon, Nav, Sidebar Toggle, Item Group 2/3/4, Segmented, Segmented 4, Segmented Text, Search, Share, More, Spacer, Add, app pieces; Controls; Dot: Close/Minimise/Zoom × Rest/Hover/Inactive; Title Bar; Toolbar; Tab; Tab Bar | toolbar 52; title-only 32; compact 40 |
| Toggles | Checkbox: Off/On/Mixed × 3 sizes × 5 states + label + group; Radio: Off/On × 3 sizes × 5 states + label + group; Switch: Off/On × Mini/Small/Regular × states + Dragging + label row | checkbox 16/20; switch 24×16, 32×20, 40×24 |
| Tooltips | One Line, Two Lines, Shortcut; Placement Below/Above/Leading/Trailing | 24 tall |
| Windows | Window: Document, Sidebar Inspector, Inactive, Panel, Utility, Sheet Attached, Full Screen Strip; Unified Toolbar (+ Sidebar), Split Toolbar (+ Inspector, Three Columns), Sidebar Collapsed, Compact Toolbar, Title Only, Sidebar Body | |
| Apps | Faro 50, Mustache 68, Mail 40, Notes 52, Preview 60, Settings 104, Calculator 24, Clock 13, Terminal 18, Text Editor 14, Screenshot 28, Mission Control 8, Wallpaper | full screens, toolbars and parts |

## Faro (search panel)

Behaviour copies the desktop's search panel exactly (state machine and timings below); the measures are Amalfi's.

**Envelope.** Faro never exceeds **640 pt** in width in any state (bar, bar + buttons, results, scopes). Every
Faro symbol is 640 wide, the Launcher included. Top edge fixed at 30 % of the screen
height; the panel only grows downwards. All values on the 4 pt grid.

| Element | Amalfi value |
|---|---|
| Bar (rest) | 640 × 56 capsule, radius 28, Glass/Thick |
| Bar beside the buttons | 384 × 56 capsule |
| Round buttons | 4 circles 56 × 56, same glass as the bar, gap 8 (pitch 64); 8 pt between bar and first button: 384 + 8 + 4·56 + 3·8 = 640 |
| Button glyphs | 24 pt (Icon Kit, Regular/24), order: Applications, Files, Actions, Clipboard |
| Magnifier / scope glyph | 24 pt at x 20, centred on the 56 pt header |
| Field text | Title 1 size (24/32 style), starting at x 56 |
| Handler icon at the right | 28 pt, 20 from the right edge |
| Panel with results | 640 wide, radius 28; header 56; 1 pt separator inset 20; filter row 40 (pills 24 tall, gap 8, inset 20); rows 56; bottom padding 8 |
| Result list | at most 6 rows visible (panel max 440 tall), then it scrolls |
| Row | icon 32 at x 20, title at x 64 (Body Emphasized), subtitle below (Caption); selection inset 8, radius 16 |
| Selected-row extras | hint (Caption, secondary) + key capsule "Tab" 20 tall, both right-aligned, 20 from the row end, vertically centred |
| Scopes (Files, Actions) | fixed 640 × 544; the list scrolls inside |
| Clipboard opt-in, No results | 640 wide, height from content |

**When the four buttons exist** (presence/absence, same as the reference behaviour):

| State | Buttons |
|---|---|
| Just opened, pointer away from the bar | none: single 640 capsule |
| Pointer enters the bar (first time in this opening) | they come out: bar 640 → 384, buttons slide out from behind it; they then stay while Faro is open, also with the pointer outside |
| Typing a query / results shown | none: the bar is 640 and grows into the results panel |
| Query cleared (Esc) | panel shrinks back to the 56 pt bar; buttons return only if they were out (not verified on the reference: mark as assumption) |
| Inside a scope (⌘1–⌘4 or a button) | none: magnifier becomes the scope glyph (back button), placeholder becomes the scope name |
| Closed and reopened | plain capsule again (buttons out only after the pointer enters); the last scope is kept |

Buttons have no hover or pressed look and no tooltip; the scope opens on release. Shortcuts ⌘1 Applications
(the Launcher), ⌘2 Files, ⌘3 Actions, ⌘4 Clipboard, also switching directly between scopes.

**Animations** (timings from the reference behaviour, distances scaled to Amalfi):

| Event | Motion |
|---|---|
| Open | opacity 0.22 → 1 in ~60 ms; height starts at ~0.97, grows with +2 pt overshoot, settled ~190 ms |
| Buttons out (pointer enters) | starts ~60 ms after entry; the capsule narrows while the buttons are still merged into it, splits at ~235 ms; bar undershoots ~20 pt below 384, then +2, settled ~690 ms; button group overshoots ~12 pt to the right, settled ~800 ms; damped spring |
| Results appear | ~500 ms after the last key; bottom edge grows in ~270 ms, +2 pt overshoot, settled ~400 ms; top edge never moves; result content swaps in one frame, no fade |
| Buttons away while typing | reverse of "buttons out" (merge back into the bar, bar widens to 640) — not measured on the reference, use the same spring |
| Enter / leave a scope | height grows downward in ~450 ms with +4 pt overshoot (leave: −4 undershoot), width unchanged |
| Esc with a query | shrinks back to 56 starting ~150 ms after the key, −4 undershoot, settled ~530 ms |
| Close | starts ~35 ms after the key: brightens and shrinks toward the centre, gone at ~100 ms; no slide |

**Launcher (Faro in apps mode, ⌘1 or the Dock).** Same 640 envelope as Faro (user decision 2026-10-16).

| Element | Amalfi value |
|---|---|
| Panel | 640 × 576, radius 28, Glass/Thick, top edge at the same 30 % line as Faro |
| Header | 56, same as Faro (apps glyph / back at x 20, field text at x 56, "…" view menu 20 from the right) |
| Category row | 40 tall, capsules 24 tall, gap 8, inset 20; the row scrolls horizontally and is clipped by the panel edge |
| Grid | 5 columns, cell 120 × 104 (pitch 120), first column at x 20; icon 64 centred, 8 below the cell top; label Caption, centred, one line, truncated with "…", 8 below the icon |
| Recents | first grid row = recent apps, then a 1 pt separator inset 20, 8 above and below, then all apps A–Z |
| Visible | recents + 3 rows; the grid scrolls; overlay scroller 4 pt from the right |
| Selection | ring 3 pt Accent around the 64 icon (radius as the icon body + 3); completion target gets a gray ring |
| List mode | already 640: rows 56 like Faro |
| Esc | width never changes now: the panel only shrinks in height to the 56 bar (same spring as "Esc with a query") |

## Launcher

Faro in apps mode (⌘1 or the Dock). Same 640 envelope as Faro (user decision 2026-10-16).

| Element | Amalfi value |
|---|---|
| Panel | 640 × 576, radius 28, Glass/Thick, top edge at the same 30 % line as Faro |
| Header | 56, same as Faro (apps glyph / back at x 20, field text at x 56, "…" view menu 20 from the right) |
| Category row | 40 tall, capsules 24 tall, gap 8, inset 20; the row scrolls horizontally and is clipped by the panel edge |
| Grid | 5 columns, cell 120 × 104 (pitch 120), first column at x 20; icon 64 centred, 8 below the cell top; label Caption, centred, one line, truncated with "…", 8 below the icon |
| Recents | first grid row = recent apps, then a 1 pt separator inset 20, 8 above and below, then all apps A–Z |
| Visible | recents + 3 rows; the grid scrolls; overlay scroller 4 pt from the right |
| Selection | ring 3 pt Accent around the 64 icon (radius as the icon body + 3); completion target gets a gray ring |
| List mode | already 640: rows 56 like Faro |
| Esc | width never changes now: the panel only shrinks in height to the 56 bar (same spring as "Esc with a query") |
