# Volume HUD

The panel shown when the volume keys are pressed. Measured with media keys sent
synthetically, on the grey wallpaper (HUD-00, CC-02).

## Shape and position

- **Not in the middle of the screen:** in macOS 27 the HUD is a panel at the **top right**,
  under the menu bar, styled like a Control Center module.
- **Size:** **292 × 64 pt**, the width of the Control Center's Display and Sound modules.
- **Position:** **10 pt** below the menu bar and **10 pt** from the screen's right edge.
- **Edges:**
  - A dark 1 pt line `#333438` just outside each side.
  - A bright row at the top (`#ced1d9`) and at the bottom (`#c7c8d1`), each fading inwards
    over about 6 pt.
- **Fill over grey:** `#6f7076`, slightly darker in the upper half (`#6d6e74`).
- **Halo:** a slight darkening of the wallpaper around the panel.
- **Light and dark:** identical, like the Control Center modules.

## Content

- **Title:** the output device's name, white, about 14 pt, 16 pt from the panel's left edge.
- **Slider:**
  - **Track:** 4 pt tall, filled white up to the level; the rest is `#84858b`.
  - **Speaker icon:** at the right end.
- **Ticks:** under the track, a row of **17 dots**, 2 × 2 pt, at a pitch of about 13.25 pt.
  There is one dot per volume step, with the first and last 19.5 pt inside the track's ends.
- **Menu bar:** a volume icon appears among the status items while the HUD shows. Its exact
  position has not been measured (*provisional*).

## Behaviour (CC-02)

- **Steps:** 16 presses take the volume from 0 to the maximum, in steps of 6.25 %.
  - With **⌥⇧** held, each step is a quarter as big.
  - Holding the key repeats it, about every 83 ms.
- **Mute:**
  - The track empties, with no white fill, and the dots stay.
  - The panel appears just as for a change in volume.
- **Pressing again while the HUD shows:** the panel does not reappear. The slider updates
  and the hide countdown restarts.
- **With the Control Center open,** the keys move its Sound slider instead, and no HUD
  shows.

| Phase | Time |
|---|---|
| First frame | about 88 ms after the key |
| Fully shown | about 470 ms (appears over about 380 ms) |
| Stays | **about 1.09 s after the last key** |
| Hides | about 550 ms: a quick drop in the first 80 ms, then a long slow tail |

## Animation

- **Appearing:**
  - The panel **scales up from about 0.91 to 1.0** around its centre while fading in.
  - The width grows almost linearly, about 4 pt every 27 ms.
  - The opacity speeds up towards the end.
  - There is no overshoot.
- **Hiding:** the same path in reverse.

Width during the appearance:

| t (ms from the first change) | 0 | 40 | 66 | 93 | 120 | 160 | 200 | 280 |
|---|---|---|---|---|---|---|---|---|
| Width (pt) | 264 | 268 | 272 | 276 | 280 | 284 | 286 | 290 |

## Not measured yet

- The brightness HUD. The reference displays had no brightness control.
