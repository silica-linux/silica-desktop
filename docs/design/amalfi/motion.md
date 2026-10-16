# Amalfi motion

Behaviour and timing follow the reference desktop as measured in `docs/spec/*.md` (what moves, when,
for how long, in which order). Amalfi changes the **geometry** (sizes, radii, distances), so every
animation has to be re-derived for the new geometry: keep the measured **timings and curve shapes**,
re-scale the **distances and amplitudes** proportionally to the Amalfi sizes, and express every motion
with the tokens below so the whole system feels the same.

## Tokens

Springs are given as response (s) and damping ratio ζ, and as Flutter `SpringDescription`
(mass 1, stiffness k = (2π / response)², damping c = 2 ζ √k).

| Token | Response | ζ | k | c | Overshoot | Use |
|---|---|---|---|---|---|---|
| `spring.snappy` | 0.25 | 0.90 | 631.7 | 45.2 | ~0 % | hover/press, lens grow, switch thumb, checkbox |
| `spring.panel` | 0.35 | 0.82 | 322.3 | 29.4 | ~1–2 % (+2 pt on a 270 pt growth) | Faro results growth, menus and popovers height, sheets |
| `spring.window` | 0.40 | 0.88 | 246.7 | 27.6 | <1 % | window open/close scale, minimise, Mission Control layouts |
| `spring.morph` | 0.50 | 0.70 | 157.9 | 17.6 | ~4–5 % | shape morphs: Faro buttons out/in, Launcher ↔ Faro, Control Center detail expand |
| `spring.bouncy` | 0.60 | 0.50 | 109.7 | 10.5 | ~16 % | Dock launch bounce, attention bounce |

| Token | Duration | Curve | Use |
|---|---|---|---|
| `fade.instant` | 100 ms | ease-out (0.2, 0, 0, 1) | closing transient surfaces (Faro, menus, tooltips) |
| `fade.quick` | 150 ms | ease-out | content swaps inside a panel, hover overlays |
| `fade.standard` | 250 ms | ease-in-out (0.4, 0, 0.2, 1) | notifications in/out, appearance changes |
| `fade.slow` | 400 ms | ease-in-out | wallpaper, display changes |
| `delay.hoverIntent` | 60 ms | — | pointer-enter morphs (Faro buttons) |
| `delay.tooltip` | 700 ms first, 0 ms while moving between items | — | tooltips |

Reduced motion (accessibility): springs become `fade.quick` cross-fades, no overshoot, no scale; bounces off.

## Rules

1. Opacity reaches 1 fast (≤ 60–100 ms) while geometry settles with a spring: content is readable at once.
2. Geometry animates from the **anchor edge** the reference uses (Faro: top edge fixed; menus: the edge
   attached to the menu title; popovers: the arrow).
3. Overshoot amplitudes are given in points in the specs; scale them by Amalfi size / reference size.
4. Glass never animates its blur radius; it animates its shape, opacity and the content under it.
5. A lens (`Glass/Clear`) grows in with `spring.snappy`, tracks the pointer 1:1, fades with `fade.quick`.
6. No animation is longer than the reference one; when the reference is unmeasured, use the token in
   the table below and mark it as an assumption in the code comment.

## Surfaces

| Surface | Event | Motion | Reference |
|---|---|---|---|
| Faro | open | opacity 0.22 → 1 in ~60 ms; height from 0.97 with `spring.panel` (+2 pt), ~190 ms | `docs/spec/faro.md` |
| Faro | pointer enters (buttons out) | after `delay.hoverIntent`: bar 640 → 384, buttons slide out from behind it, `spring.morph`; bar −20 pt undershoot then +2; buttons +12 pt overshoot; settled ~690 / ~800 ms | reference notes FARO-02 |
| Faro | typing (buttons in) | reverse of buttons out with `spring.morph` (unmeasured: assumption) | — |
| Faro | results appear | ~500 ms after the last key, bottom edge grows with `spring.panel`, ~400 ms; content swaps in one frame | FARO-02 |
| Faro | enter/leave scope | height with `spring.panel`, +4 / −4 pt, ~450 ms; width fixed 640 | FARO-02 |
| Faro | close | `fade.instant` with a slight brighten and scale to 0.97 toward the centre, gone ~100 ms | FARO-02 |
| Launcher | open / Esc to bar | width is always 640 now: only height animates, `spring.panel`; content fades `fade.quick` | `docs/spec/launcher.md` |
| Menus | open | fade 0 → 1 `fade.instant`, no scale; close `fade.instant` | `docs/spec/menubar.md` |
| Popovers | open/close | fade + scale 0.96 → 1 from the arrow, `spring.panel` | `docs/spec/misc.md` |
| Tooltips | show/hide | `fade.quick` after `delay.tooltip` | `docs/spec/tip.md` |
| Notifications | banner in/out | slide from the trailing edge + fade, `spring.panel`; out `fade.standard` | `docs/spec/notifications.md` |
| Control Center | panel open | `fade.instant` + height `spring.panel`; module → detail `spring.morph` | `docs/spec/controlcenter.md` |
| Dock | magnification, launch bounce, minimise | magnification follows the pointer 1:1 (no spring); bounce `spring.bouncy`; minimise `spring.window` | `docs/spec/dock.md` |
| Windows | open / close / zoom | `spring.window` scale 0.96 → 1 + `fade.quick` | `docs/spec/windows.md` |
| Mission Control / Panorama | enter / exit | `spring.window` | `docs/spec/panorama.md`, `spaces.md` |
| App switcher | show | `fade.instant` | `docs/spec/app-switcher.md` |
| Screenshot bar | show/hide | `fade.quick` + 8 pt rise, `spring.panel` | `docs/spec/screenshot-ui.md` |
| Segmented control | select | lens grows (`spring.snappy`), slides to the new segment (`spring.morph`), fades | — |
| Switch | toggle | thumb `spring.snappy`; while dragged the thumb is a Clear lens | — |
| Push / icon buttons | hover / press | overlay `fade.quick` in, 60 ms out | — |

When a measured spec gives a curve that a token cannot match (for example the Launcher Esc shrink),
keep the measured curve for that one surface and say so in the code.
