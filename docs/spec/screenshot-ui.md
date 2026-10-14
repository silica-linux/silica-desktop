# Screenshots, force quit and the character viewer

Interfaces opened by system shortcuts (K-02, K-05, K-06). Measured on the grey wallpaper,
in light appearance unless noted.

## ⇧⌘3: whole screen, and the thumbnail

- **Files:** one file per display.
- **Thumbnail** at the bottom right:

  | | Value |
  |---|---|
  | Image | the screen at 0.1× (256 × 144 for 2560 × 1440) |
  | Frame | **4 pt** white, so the outer size is **264 × 152** |
  | Corner radius | about 4 pt |
  | Margin | **12 pt** from the right and bottom screen edges |
  | Shadow | soft, black, about symmetric: −10 levels at the edge, −8 at 2 pt, −5 at about 12 pt, −2/−3 at 24–26 pt |
- **Appearing:** the thumbnail is **already in place and at full size**, with no slide or
  scale. It **fades in over about 400 ms**, slowing towards the end, and the shadow widens
  with it.
- **Leaving:** after about **5.1 s** it **slides off to the right** in about **220–250 ms**.
  - The motion speeds up, then holds a near-constant speed.
  - Movement per 13 ms frame, in pt: 9, 10, 12, 14, 15, 17, 19, 20, 20, 24, 25, 24, 26, 24.
- **Saving:** the file is written when the thumbnail leaves.
- **Clicking the thumbnail** opens the screenshot editor and the thumbnail disappears.
  - **Window:** 90 % of the screen, centred.
  - **Top bar:** close on the left; on the right edit, delete and share (36 × 24 each), plus
    Done (42 × 22).
  - **Bottom right:** an image-analysis button of 28 × 28.
  - **Delete** closes the editor and removes the file.

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
  button's, **210 pt** wide.
- **Rows:** 24 pt items with checkmarks, and 11 pt separators.
- **Sections**, each with a grey header that cannot be chosen:
  - **Save to:** Desktop ✓, Documents, a few apps, other location…
  - **Timer:** none ✓, 5 s, 10 s.
  - **Options:** show floating thumbnail ✓, remember last selection ✓.
  - **Microphone:** none ✓.
  - **Recording:** show mouse clicks, include system audio.
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
- The shutter sound.
- Screen recording.
- ⇧⌘4 in dark appearance.
- Dragging the thumbnail.
- The full character panel.
