# Collecting the references on macOS 27

Procedure for the sessions on the Mac that observe macOS 27 and produce the references for
parity. It applies to people and agents. The clean-room rules are in `AGENTS.md`: they apply
here.

## Where the references go

macOS screenshots and notes never enter the Silica repo (`plan/` is gitignored).
They live in the private repo `silica-os/silica-reference`, cloned into
`plan/reference/`:

```bash
gh repo clone silica-os/silica-reference plan/reference
```

Structure:

```
plan/reference/
  setup.md              Mac, macOS build, display, settings used
  <area>.md             measurements and observations of an area (dock.md, glass.md, …)
  (captures)            screenshots and recordings cited by the notes (private)
```

## Fixed conditions

The measurements are compared with Ossido's screenshots: the conditions must be repeatable
and written in `setup.md`.

- Display: model, native resolution, scale (e.g. 2×), refresh.
- "Default" resolution in System Settings › Displays.
- Wallpaper: solid color (System Settings › Wallpaper › Colors), the same gray for all the
  sessions; then a capture with a striped wallpaper generated in code, to see blur and
  refraction.
- Light and dark appearance, default accent color, "Reduce transparency" and "Reduce
  motion" off.
- Dock: size and magnification noted (sliders in System Settings › Desktop & Dock).

## What is allowed

- Using the UI, taking screenshots (`screencapture -x`, `-R x,y,w,h`, `-l <windowid>`) and
  screen recordings (`screencapture -v` or QuickTime).
- Measuring on the screenshot pixels with our own scripts (Python + Pillow).
- Reading the geometry and hierarchy of the visible elements with the accessibility API
  (e.g. position and size of the Dock icons): it is observation of the running UI.

## What is forbidden

- Reading, copying or disassembling system files: `/System`, `/Library`, bundles in
  `/Applications`, frameworks, resources, Apple plists, `defaults read` of Apple domains.
- Extracting Apple icons, images, sounds, fonts or texts to use them in Silica.
- Putting macOS screenshots in the Silica repo or in issues and PRs.

## Note format

Every measurement has a value, unit, method and source capture (cited only inside
silica-reference; public code, docs and issues cite the note or the inventory ID instead). Sizes in points
(pixels / scale), durations in ms, colors in hexadecimal sRGB with alpha.

```markdown
## Dock · icons at rest

- Icon side: 48 pt (96 px at 2×), method: alpha edge on the capture of the Dock at rest
- Spacing between icons: 8 pt
- macOS 27.0 (build …), 2026-10-13
```

When something cannot be measured (e.g. the exact curve of an animation), write what was
observed and with which uncertainty, without estimating numbers.

## Sessions

In order of priority, following the roadmap.

1. **Dock** (needed for M1.5): sizes and spacings at rest, maximum magnification, scale of
   every icon as a function of the distance from the pointer (screenshots with the pointer at
   known points, or accessibility), radius of influence, return to rest (60 fps video:
   duration and bounce), label (font, sizes, position), background (radius, color, edge,
   shadow), indicators of the open apps.
2. **Glass** (M2): blur, tint, edge and highlights of the material on a solid and a striped
   background, light and dark; refraction at the edges.
3. **Windows** (M2): corner radii, active and inactive shadows, title bar, window buttons
   (sizes, spacings, colors, states), opening, closing and minimize animations.
4. **Menu bar** (M3): height, spacings, font, status area, open menus.
5. **Control Center and notifications** (M4), **Mission Control and spaces** (M5).
