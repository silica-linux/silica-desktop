# Screenshots, force quit and the character viewer

Interfaces opened by system shortcuts (K-02, K-05, K-06). Measured on the grey wallpaper,
in light appearance unless noted.

## ⇧⌘3: whole screen, and the thumbnail

- **Files:** one file per display.
- **Thumbnail** at the bottom right:

  | | Value |
  |---|---|
  | Image | the screen at 0.1× (256 × 144 for 2560 × 1440) |
  | Frame | **4 pt**, so the outer size is **264 × 152**: white in light appearance; in dark, black with a **1 pt** light line (about `#949494`) on its inner edge |
  | Corner radius | about 4 pt |
  | Margin | **12 pt** from the right and bottom screen edges |
  | Height | follows the image: 264 wide for a landscape image, e.g. 264 × 136 for a 2:1 one |
  | Transparent parts of the image | render black in dark appearance |
  | Shadow | soft, black, about symmetric: −10 levels at the edge, −8 at 2 pt, −5 at about 12 pt, −2/−3 at 24–26 pt |
- **Appearing:** the thumbnail is **already in place and at full size**, with no slide or
  scale. It **fades in over about 380 ms**, slowing towards the end, and the shadow widens
  with it.
- **Staying:** about **5.4 s**. The timer **pauses while its context menu is open**.
- **Leaving:** it **slides off to the right** in about **250 ms**.
  - The motion speeds up, then holds a near-constant speed.
  - Movement per 13 ms frame, in pt: 9, 10, 12, 14, 15, 17, 19, 20, 20, 24, 25, 24, 26, 24.
- **Saving:** the file is written when the thumbnail leaves.
- **Dragging the thumbnail:** it hides, and a **frameless** copy of the image follows the
  pointer, shrinking from the thumbnail's size to **128 pt on its long side** in about
  **300 ms**, then held roughly centered on the pointer (the pointer 10–15 pt above and to the
  left of the center). No count badge, no name pill.
  - **Onto a folder or the desktop:** the file is **moved** there. No copy is saved in the
    default location. Onto the desktop it appears at a free spot near the drop point.
  - **Into an app** (a rich-text document): the image is inserted at full size and **no file
    remains**.
- **Swipe to the right** (a quick horizontal flick): it is not a drag and drop. The thumbnail
  follows the pointer, leaves in about 220 ms, and the file is **saved at once**.
- **Context menu** (secondary click), in groups: **Save to Desktop / Documents / Clipboard**,
  **Open in Mail / Preview / Photos**, **Show in the file manager / Delete**, **Markup /
  Close**.
  - The Save items pick the place for this one capture. The Open items hand the image to
    that app. Close dismisses the thumbnail and saves.
  - **Delete** leaves no file anywhere, plays no sound and does not touch the Trash.
- **Clicking the thumbnail** opens the screenshot editor and the thumbnail disappears.
  - **Window:** 90 % of the screen, centred.
  - **Top bar:** close on the left; on the right edit, delete and share (36 × 24 each), plus
    Done (42 × 22).
  - **Bottom right:** an image-analysis button of 28 × 28.
  - **Delete** closes the editor and removes the file.

### Window capture file

A window captured with ⇧⌘4 + Space is the window **plus a transparent shadow margin**:
**22 pt on the left, 23 on the right, 15 on top and 30 at the bottom** (a 1050 × 480 window
gives a 1096 × 526 file). The corners of the window are transparent outside its rounded shape.
The shadow fades from about 0.35 alpha under the bottom edge to 0 over 30 pt, about 0.21 at the
sides and 0.08 above.

### Shutter sound

Two soft clicks about **230 ms** apart, each a short broadband tick that decays in 100–150 ms
(about 370 ms in all), starting about 0.2 s after the selection is released.

## ⇧⌘4: selection and window

- **Pointer:** it becomes a **crosshair**, a thin black cross of about 20 pt with a small
  translucent grey disc in the middle.
  - Next to it, at the bottom right, are the **point's coordinates in pt**: x above, y below,
    in small black text with a light halo.
  - The coordinates follow the pointer, with the origin at the top left of the display.
  - The crosshair appears within about 230 ms of the key.
- **Space** switches to window mode: the pointer becomes a camera, and the window under it
  gets a **tint**.
  - **Tint colour:** out = 0.514 · C + 0.486 · background, with **C = `#88bae7`**, so about
    **51 %** opacity.
  - It covers the window's **rectangle**, corners included, not its rounded shape. The shadow
    is not tinted.
  - Over the desktop, **the whole screen** is tinted, the Dock included.
  - The tint appears within one frame of Space and jumps between windows with no fade.
- **Esc** cancels, and no file is written.

## ⇧⌘5: the screenshot bar

- **Bar:** a horizontal glass bar of about **545 × 52 pt**, 4 pt above the Dock zone.
  - It remembers its last position, so it is not always centred (*provisional*: Silica
    centres it the first time).
- **Buttons, left to right:**
  1. close: a disc with ×, 18 × 50;
  2. capture the whole screen, a window, a selection: 44 × 50 each, at a 48 pt pitch;
  3. a separator;
  4. record the whole screen, a window, a selection: at a 57 pt pitch;
  5. a separator;
  6. an Options button with an arrow, 76 × 50;
  7. the action button, 80 × 50, filled blue: "Capture" or "Record".
- **States:**
  - The chosen mode sits on a **rounded pill**.
  - The action button **exists only while there is a target**, a window or the desktop under
    the pointer.
  - In window mode the window under the pointer gets the same tint as ⇧⌘4 + Space.

| | Light (under the desktop tint) | Dark (no tint) |
|---|---|---|
| Bar background | `#b5c7e1` | `#7b7c81` over `#7a7b80`, almost clear glass |
| Top edge | `#62738d` | 1 pt `#595a5e` with `#76777c` above |
| Glyphs and the Options label | `#4f617b` | `#c7c8cd` |
| Separators (1 pt) | `#a1b3cd` | `#8f9095` |
| Chosen mode: pill / glyph | `#a1b3cd` / dark | `#8f9095` / `#dbdde2` |
| Chosen mode while pressed | — | pill `#8a8b90`, glyph `#c3c4c9` |
| Action button | `#006fdc` | — |

- **Hovering a mode that is not chosen:** nothing changes.
- **Dark appearance with the desktop tint:**
  - background `#798eab`;
  - glyphs `#c5daf8`;
  - chosen pill `#8da2bf`.

### Options menu

- **Position and size:** it opens **above** the button, its left edge aligned with the
  button's. It is **216 pt** wide in the capture modes (about 417 tall) and **210 pt** in the
  record modes (about 476 tall).
- **Rows:** 24 pt items with checkmarks, and 11 pt separators.
- **The menu depends on the bar's mode.** Sections, each with a grey header that cannot be
  chosen:
  - **Capture modes:**
    - **Save to:** Desktop ✓, Documents, Clipboard, Mail, Preview, other location…
    - **Timer:** none ✓, 5 s, 10 s.
    - **Options:** show floating thumbnail ✓, remember last selection ✓.
    - **Capture:** show mouse pointer (off).
  - **Record modes:**
    - **Save to:** Desktop ✓, Documents, Mail, QuickTime Player, other location…
    - **Timer** and **Options** as above.
    - **Microphone:** none ✓.
    - **Capture:** show mouse clicks, include system audio (both off).
- **Closing:** Esc closes the menu, and a second Esc closes the bar.

| | Light | Dark |
|---|---|---|
| Background | `#c8c9ce` | `#707176` |
| Items | `#1f1f1f` | `#e9e9ea` |
| Headers | `#939396` | `#939498` |
| Separators | `#b3b4b9` | `#858689` |
| Border, outer / inner hairline | `#626267` / `#e1e2e8` | `#121212` / `#9a9ba0` |

## ⌘⌥⎋: force quit

- **Window:** a **370 × 310 pt** window, above everything, titled with the force-quit name.
  Close and zoom are active; minimise is disabled.
- **Content:**
  - two lines of text at the top;
  - a bordered list of **313 × 200** with **22 pt rows**, each with a small app icon and the
    app's name; the front app is selected;
  - a two-line hint at the bottom left;
  - a blue button at the bottom right.
- **Button:**
  - It reads "relaunch" (65 × 30) when the front app or the file manager is selected, and
    "force quit" (115 × 30) otherwise.
  - Its fill is `#007aff` with white text.
- **Position:** remembered; a first-time position has not been measured.

| | Dark |
|---|---|
| Panel / title bar | `#272727` / `#2f2f2f` |
| Border | 1 pt `#060606` outside, `#3d3d3d` inside |
| Title (not the key window) | `#9e9e9e` |
| Text and row names | `#dedede` |
| List border | 1 pt `#353535` |
| Selected row | `#0059d1`, with a white name |
| Shadow below | `#353537` at 1 pt, `#48484c` at 13, `#5c5d60` at 25, `#6c6d71` at 37, `#75767b` at 49: about **50 pt** |

## ⌃⌘Space: characters and emoji

With a text cursor active, a **small floating bar** of **152 × 26 pt** appears just below the
insertion point. It has three buttons and an arrow that expands it into the full panel.

## Not measured yet

- The lock screen shortcut.
- Screen recording beyond the basics: a click on the window starts it, a stop button appears
  in the menu bar, and the stop shortcut writes the file at once with no thumbnail.
- ⇧⌘4 in dark appearance.
- The full character panel.

### What Silica does

The thumbnail, its menu, the Options menu, the window capture file and the shutter follow the
SCREENSHOT-2 measurements above (#87). `shell/lib/screenshot/` marks what is still a guess as
*provisional*; the rest of the values were measured earlier or are listed here.

- **Thumbnail:**
  - The frame, timings (380 ms, 5.4 s, 250 ms) and sizes are the measured ones; the frame is
    black with the 1 pt light line in dark appearance.
  - **Dragging** starts a real drag and drop of a file that exists only for the drag (in
    `$XDG_CACHE_HOME/silica/screenshots`). The image on the pointer is the capture alone, 128
    pt on its long side, held 12 pt above and to the left of its center. A drop that a target
    takes (a folder, the desktop, an app) deletes that file once the target has its copy, so
    the capture is moved there and nothing stays on the Desktop. Dropped nowhere, the thumbnail
    comes back and the capture goes to its place when it leaves.
    - The ~300 ms shrink is not animated: the drag image is a static Wayland surface, so it
      is already 128 pt when the drag starts.
  - A **two-finger swipe to the right** of 40 pt (*provisional*: the threshold; the measured
    flick is a mouse gesture of at least 100 pt in 100 ms, not handled) saves the capture at
    once and sends the thumbnail away.
  - The **menu** has Save to Desktop / Documents / Clipboard; Open in Mail / the default image
    viewer (as Preview) / Photos, each only if the app is installed; Show in the file
    manager; Delete; Close. **Markup is hidden** until the editor exists (#154). The timer
    stops while the menu is open and goes on from where it stopped. Delete removes the
    capture and plays no sound.
  - A click opens the image with the default app (no editor yet, #154).
  - The pointer on the thumbnail keeps it; when the pointer leaves, it stays another 5.4 s
    (*provisional*: not measured).
- **⇧⌘4:**
  - While dragging, the label shows the selection's size.
  - Shift keeps one side fixed, Option grows from the center, Space moves the selection.
  - Window captures are the window plus the transparent margin of 22 / 23 / 15 / 30 pt, whatever
    the shadow on the screen reaches (Option: the frame only). Over the desktop, the whole
    screen is captured.
- **⇧⌘5 bar:**
  - The separators, Options and the action button sit at 192, 372, 383 and 463 pt from the
    bar's left edge. The blue pill is 76 × 30 (*provisional*).
  - In screen mode a click anywhere captures. The first time, the selection is half the
    screen, centered.
  - The selection is drawn with a 35 % black dim outside, a white border and eight 9 pt
    handles.
  - The bar's background drags it, and its place is remembered.
  - The timer shows a 96 × 96 dark square with the seconds left, at the center of the
    target. Apps stay usable during the countdown.
  - **Options menu (capture modes):** Save to (Desktop, Documents, Clipboard, Mail and the
    image viewer if installed, folders chosen before, Other Folder…), Timer (none, 5 s,
    10 s), Options (show floating thumbnail, remember last selection) and Capture (show mouse
    pointer, which applies to ⇧⌘3 as well). The menu's width follows its content, not the
    measured 216 pt.
  - **Recording is not available** (#153): its three buttons are dimmed and do nothing, so the
    record-mode menu is not built.
- **Clipboard:** ⌃ captures go straight to the clipboard, with no thumbnail and no file.
- **Files:** the name is the capture time, "Screenshot 2026-10-14 at 04.12.33.png"; in Italian
  "Istantanea schermo 2026-10-14 alle 04.12.33.png" (the Italian wording is provisional).
- **Sound:** an original synthesized shutter (`screen-capture`, made by `tools/sounds/synth.py`,
  played by silica-daemon `Sound1`) plays **once** per capture, unless the interface sound
  effects are off. The two clicks 230 ms apart are the sound file's job, which is being redone.
