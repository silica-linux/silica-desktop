# Amalfi materials: glass, blur, surfaces

The official material spec of Silica 1.0 "Amalfi". Values are in `tokens.json` → `materials`, `shadows`,
`colors.*.Glass/*`. This file says **what each material is, how to render it, and where it goes**. When the
UI kit and this file disagree, this file wins and the kit is fixed later.

## The idea

- Windows are **flat and opaque**: regions (sidebar, content, inspector, toolbar) separated by 1 pt
  `Line/Separator` lines. Only the window as a whole has a shadow (`shadows.3`). No glass inside windows
  except the **controls** that sit in the toolbar.
- Everything that **floats above** windows or the desktop is glass: Dock, menus, popovers, Faro, Control
  Center, notifications, HUDs, tooltips, sheets.
- Glass is a material, not a colour: it blurs what is behind it, lifts saturation, adds a tint and a soft
  rim. Glass is never stacked on glass: items inside a glass surface are plain fills on it (selection,
  pills, rows), never a second blur.
- Interactive glass behaves as a **lens**: when a glass control is pressed or dragged, a `Glass/Clear`
  capsule grows over it and magnifies what is under it.

## The three materials

| | Glass/Regular | Glass/Thick | Glass/Clear (lens) |
|---|---|---|---|
| Purpose | light objects close to content | panels you read from | transient lens over a control |
| Background blur (Gaussian radius, pt) | 6 | 16 | 2 + refraction |
| Saturation boost | 1.5 | 1.5 | 1.0 |
| Tint light / dark | `#FFFFFF` 60 % / `#2A2B30` 60 % | `#FFFFFF` 76 % / `#2A2B30` 76 % | none |
| Inside rim (1 pt) light / dark | `#000000` 12 % / `#000000` 40 % | same | same |
| Rim light (inner shadow 0, 1, blur 1) | `#FFFFFF` 70 % / `#FFFFFF` 22 % | same | same |
| Drop shadow | `shadows.2` (y 4, blur 12) | `shadows.3` (y 12, blur 32) | none |
| Refraction | — | — | distortion 0.3, depth 0.5, chromatic aberration 0.08, magnification ≈ 1.1×, specular highlight on the top edge; brightness ×1.04 light / ×0.98 dark |
| Shape | capsule or smooth-cornered rectangle (smoothing 0.6) | same | capsule only |

Render order for Regular and Thick: backdrop → blur → saturation → tint fill → inside rim → rim light
(top inner edge) → content → drop shadow outside the shape. Blur and tint are clipped to the shape's smooth
corners. In the compositor (Ossido) the blur is the existing glass pass ([`docs/m2-glass.md`](../../m2-glass.md),
[`docs/spec/glass.md`](../../spec/glass.md)): reuse its kernel and only change the parameters to these.

Fallback when blur is unavailable (software rendering, reduced transparency setting): replace the blur with
an opaque fill = tint over `Surface/Window`, keep rim and shadow.

## Where each material goes

| Surface | Material | Radius | Notes |
|---|---|---|---|
| Menu bar | **none** (transparent strip on the wallpaper) | — | items are text/glyphs; an open menu title gets an `Accent/Amalfi Soft` capsule |
| Menus, context menus, submenus | Thick | 12 | highlighted item = `Accent/Amalfi` fill, full item width inside the menu padding, radius 7 |
| Status menus (Wi-Fi, Bluetooth, Battery) | Thick | 12 | the kit uses Regular in a few: use Thick |
| Popovers (all edges, detached) | Thick | 16 | the kit's rebuilt popovers lost the glass and look opaque white: use Thick |
| Tooltips | Thick | 8 | |
| Dock shelf | Regular | 20 | tiles are app icons, no glass on tiles |
| Faro, Launcher | Thick | 28 | round buttons beside the Faro bar: same Thick glass |
| Control Center panel | none (transparent container) | — | each module is a Thick tile (toggle modules are capsules, radius = height / 2); detail views are Thick |
| Notification banners | Regular | 18 | the Notification Centre panel is Thick, radius 24 |
| Alerts | Thick (glass variant) or `Surface/Raised` opaque | 20 | system alerts use glass, app alerts in a window use opaque |
| Sheets attached to a window | Thick | 20 | |
| HUDs (volume, brightness), Screenshot bar | Regular | capsule / 20 | |
| Toolbar controls (icon buttons, action segments, search capsule) | Regular | capsule | the toolbar itself is flat (window surface) |
| Window, sidebar, inspector, content | **opaque** `Surface/Window` / `Surface/Sidebar` | window 18 | separators 1 pt, inset as in the layout rules |
| Fields, push buttons, checkboxes, switches at rest | opaque fills (`Surface/Control`, `Accent/Amalfi`) | per control | |
| Pressed segmented control, dragged slider thumb, dragged switch thumb | Clear (lens) | capsule | see the lens rule |
| Media overlays (video controls, Quick preview controls) | Clear | capsule | |

## Lens rule

- Trigger: pointer down on a glass segment, or drag of a slider/switch thumb.
- Shape: `Glass/Clear` capsule, 8 pt larger than the control on every side (a 72 × 20 segment gets an
  88 × 44 lens... i.e. width + 16, height + 16 for segments; switch thumb + 12 at Regular), radius = height / 2 − 1.
- Content under the lens is magnified ≈ 1.1× and refracted; the label/glyph under it stays readable.
- Motion: grows in with `motion.spring.snappy`, follows the pointer 1:1 while dragging, settles into the
  selected segment on release, fades out in 120 ms.

## Surfaces (opaque)

| Token | Light | Dark | Use |
|---|---|---|---|
| Surface/Window | `#FFFFFF` | `#1B1C1F` | window body, content |
| Surface/Sidebar | `#F5F6F8` | `#232428` | sidebars, source lists |
| Surface/Raised | `#FFFFFF` | `#2A2B30` | cards, fields, alerts in windows |
| Surface/Sunken | `#EEF0F3` | `#141518` | wells, slider tracks, segmented tracks |
| Surface/Control | `#FFFFFF` | `#3A3B40` | secondary buttons, segment thumb |
| Surface/Control Pressed | `#E9ECF0` | `#46474D` | pressed secondary controls |

No warm/yellowish greys anywhere: all neutrals are cool-neutral (user decision).

## Dark mode

Same structure, dark tokens. Glass tint switches to `#2A2B30`, rim dark gets stronger (40 %) and rim light
weaker (22 %); shadows are ~2× denser (`shadows.dark`).

## Implementation in Ossido

The compositor's glass pass draws Glass/Regular and Glass/Thick for every glass region a surface
declares (`crates/ossido-render/src/glass_profile.rs`, details and the kind → material table in
[`docs/m2-glass.md`](../../m2-glass.md)):

- The kit's background blur radius is the σ of a CSS `blur()`: σ 6 pt (Regular), 16 pt (Thick),
  2 pt (Clear). A drop shadow's blur is a CSS `box-shadow` blur: σ = blur / 2.
- Each kind of surface of `silica-glass-v1` maps to one material by the table above; the radius
  and shape come from the region the shell declares.
- Ossido keeps its **sharp share**: before the saturation the blurred backdrop is mixed with a
  little of the sharp one, so the glass blurs a little and keeps part of the backdrop sharp. It
  is one parameter per material of a kind and keeps for now the share each kind had before
  Amalfi (0.28 on the Dock, 0.04 on modules and controls, 0.03 / 0.02 on banners, none on the
  other thick glass). Its value is to be tuned against the kit's PNG exports of each material
  once they are exported; until then it is provisional.
- No lens, measured edge or outline on Regular and Thick; the blur radius never animates (a
  fading surface changes the glass's opacity only).
- Reduced transparency is the `opaque` glass quality (`ui.glass_quality`): the tint over
  `Surface/Window`, rims and shadow kept, no saturation.
