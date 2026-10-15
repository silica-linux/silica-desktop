//! Panorama (M5.2): all the windows of the space, side by side, and the spaces bar at the
//! top (plan/reference/missioncontrol.md).
//!
//! - Windows move (and shrink only if they do not fit) until they no longer overlap: with
//!   two windows and enough room the scale stays 1, they move apart along the line through
//!   their centers and there are 16 pt between them. The rule with many windows is not
//!   measured yet (RICHIESTE-panorama.md, PA-10): if they do not fit at full size, balanced
//!   scaled rows, centered group.
//! - The wallpaper does not darken but blurs as Panorama opens (`BACKDROP_BLUR_PT`); the
//!   menu bar and the desktop icons fade out, the spaces bar comes down in the menu bar's
//!   place, 40 pt tall: a material (blur of what is under it, tinted to `#9e9fa5` over the
//!   reference gray), border `#58595c`, ~62 pt shadow; the Dock stays.
//! - Opening and closing: progressive slowdown with a long tail, 50 % in ~40–54 ms, 90 % in
//!   ~130–140 ms, done in ~300 ms (tables measured frame by frame).
//! - A click on a window brings it to the front and closes Panorama; Esc or a click on the
//!   empty area close it; a click on a space of the bar switches to it; "+" adds a space.

use std::time::Instant;

use ossido_render::sprite::{Canvas, Sprite, Weight, text_width};
use silica_control::commands::{PanoramaRect, PanoramaState, PanoramaWindow};
use smithay::desktop::Window;
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::state::State;

/// Compact spaces bar.
pub const BAR_HEIGHT: f64 = 40.0;
pub const BAR_SHADOW: f64 = 62.0;
/// The bar is a material: a blur of what is under it with a tint over it. The measured
/// fills (over the solid `#7a7b80` wallpaper of the reference) are what the tint gives over
/// that gray: light `#9e9fa5` (≈ white at 30 %), dark `#55565b` (≈ black at 31 %).
const BAR_FILL: [u8; 4] = [0x9e, 0x9f, 0xa5, 0xff];
const BAR_FILL_DARK: [u8; 4] = [0x55, 0x56, 0x5b, 0xff];
/// Opacity of the light and dark tints (the lowest that reaches the measured colors).
const BAR_TINT_ALPHA: f64 = 0.30;
const BAR_TINT_ALPHA_BOTTOM: f64 = 0.35;
const BAR_TINT_ALPHA_DARK: f64 = 0.31;
/// The gray the bar colors were measured over.
const REFERENCE_WALLPAPER: [u8; 3] = [0x7a, 0x7b, 0x80];
/// σ of the blur under the bar, in points. Not measured (solid wallpaper in the reference).
pub const BAR_BLUR_PT: f64 = 20.0;
const BAR_BORDER: [u8; 4] = [0x58, 0x59, 0x5c, 0xff];
/// The shadow below the bar darkens the wallpaper by ~24 levels out of 122 at the edge.
const BAR_SHADOW_ALPHA: f32 = 0.2;
const CAPSULE_FILL: [u8; 4] = [0x87, 0x88, 0x8e, 0xff];
const CAPSULE_BORDER: [u8; 4] = [0x8d, 0x8e, 0x94, 0xff];
const CAPSULE_FILL_DARK: [u8; 4] = [0x78, 0x79, 0x7e, 0xff];
const CAPSULE_BORDER_DARK: [u8; 4] = [0x6e, 0x6f, 0x74, 0xff];
const LABEL_COLOR: [u8; 4] = [0, 0, 0, 0xff];
const LABEL_COLOR_DARK: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const PLUS_COLOR: [u8; 4] = [0x4c, 0x4c, 0x4c, 0xff];
const PLUS_COLOR_DARK: [u8; 4] = [0xe5, 0xe5, 0xe5, 0xff];
/// Labels: size ~13–14 pt; capsule of the current space 24 tall at 7 pt from the edge, with
/// ~11 pt of margin around the text; 2 pt between one entry and the next.
const LABEL_SIZE: f32 = 13.5;
const CAPSULE_TOP: f64 = 7.0;
const CAPSULE_HEIGHT: f64 = 24.0;
const CAPSULE_RADIUS: f32 = 6.5;
const LABEL_PADDING: f64 = 11.0;
const LABEL_GAP: f64 = 2.0;
/// The "+": 20 × 20 cross, 2 stroke, center at 26 pt from the right edge and 20 from the top.
const PLUS_SIZE: f64 = 20.0;
const PLUS_RIGHT: f64 = 26.0;

/// Gap between the arranged windows.
pub const WINDOW_GAP: f64 = 16.0;
/// Margins of the window area (provisional: PA-10).
const MARGIN_SIDE: f64 = 48.0;
const MARGIN_TOP: f64 = 48.0;
const MARGIN_BOTTOM: f64 = 24.0;

/// Expanded bar (pointer at the top): 170 pt tall with border `#5f6063` and ~40 pt of shadow;
/// the fill goes from `#9e9fa5` to `#a4a5ab` (y 114 and below).
pub const EXPANDED_HEIGHT: f64 = 170.0;
const EXPANDED_SHADOW: f64 = 40.0;
const EXPANDED_FILL_BOTTOM: [u8; 4] = [0xa4, 0xa5, 0xab, 0xff];
const EXPANDED_FILL_BOTTOM_DARK: [u8; 4] = [0x5b, 0x5c, 0x61, 0xff];
const EXPANDED_FILL_FROM: f64 = 114.0;
const EXPANDED_BORDER: [u8; 4] = [0x5f, 0x60, 0x63, 0xff];
/// Thumbnails: 160 pt wide (16:9 on the reference screen), 52 pt from the edge, 32 pt
/// between them; the group leaves an empty slot on the right (with two spaces the second
/// thumbnail is at the center of the screen).
const THUMB_WIDTH: f64 = 160.0;
const THUMB_TOP: f64 = 52.0;
const THUMB_GAP: f64 = 32.0;
/// Ring of the current space: 3 pt, 1 pt of bar between ring and thumbnail, sRGB `#3b99fd`.
const RING_WIDTH: f64 = 3.0;
const RING_GAP: f64 = 1.0;
const RING_COLOR: [u8; 4] = [0x3b, 0x99, 0xfd, 0xff];
/// Labels below the thumbnails: size ~12 pt, baseline 18 pt below the thumbnail.
const THUMB_LABEL_SIZE: f32 = 12.0;
const THUMB_LABEL_BASELINE: f64 = 18.0;
/// The "+" comes down with the bar: center at y 20 compact, 101.5 expanded.
const PLUS_Y: f64 = 20.0;
const PLUS_Y_EXPANDED: f64 = 101.5;
/// The thumbnails appear halfway through the expansion (in the video between 63 % and 80 %).
const THUMBS_FROM: f64 = 0.6;
const THUMBS_FULL: f64 = 0.8;
/// Beyond this distance from the press, a click on a window becomes a drag.
const DRAG_THRESHOLD: f64 = 4.0;
/// The window dragged over the bar shrinks to this scale.
const DRAG_OVER_BAR_SCALE: f64 = 0.25;

/// Bar expansion (ms, 0–1): height from 40 to 169 pt in the video, frame by frame. Closing is
/// not measured: same curve.
pub const EXPAND_CURVE: &[(f64, f64)] = &[
    (0.0, 0.0),
    (13.0, 0.29),
    (26.0, 0.37),
    (53.0, 0.52),
    (66.0, 0.59),
    (80.0, 0.64),
    (106.0, 0.74),
    (120.0, 0.78),
    (133.0, 0.81),
    (146.0, 0.87),
    (173.0, 0.91),
    (200.0, 0.92),
    (226.0, 0.95),
    (280.0, 0.98),
    (346.0, 0.99),
    (440.0, 1.0),
];

/// Opening progress (ms, 0–1): movement of the Terminal in the video.
pub const OPEN_CURVE: &[(f64, f64)] = &[
    (0.0, 0.0),
    (13.0, 0.26),
    (27.0, 0.38),
    (67.0, 0.59),
    (93.0, 0.74),
    (120.0, 0.84),
    (133.0, 0.91),
    (173.0, 0.95),
    (227.0, 0.98),
    (320.0, 1.0),
];

/// Closing progress (ms, 0–1): edge of the file manager window in the burst after Esc.
pub const CLOSE_CURVE: &[(f64, f64)] = &[
    (0.0, 0.0),
    (13.0, 0.054),
    (31.0, 0.269),
    (52.0, 0.389),
    (67.0, 0.497),
    (79.0, 0.599),
    (93.0, 0.683),
    (106.0, 0.749),
    (118.0, 0.802),
    (132.0, 0.850),
    (145.0, 0.880),
    (157.0, 0.910),
    (171.0, 0.934),
    (184.0, 0.946),
    (199.0, 0.958),
    (211.0, 0.964),
    (224.0, 0.976),
    (236.0, 0.988),
    (291.0, 0.994),
    (319.0, 1.0),
];

/// App Exposé, opening (ms, 0–1): top edge of the window from y 300 to 388 in the video.
pub const APP_OPEN_CURVE: &[(f64, f64)] = &[
    (0.0, 0.0),
    (6.0, 0.05),
    (20.0, 0.27),
    (33.0, 0.39),
    (60.0, 0.50),
    (73.0, 0.68),
    (86.0, 0.75),
    (113.0, 0.81),
    (126.0, 0.85),
    (140.0, 0.91),
    (166.0, 0.93),
    (180.0, 0.95),
    (193.0, 0.98),
    (233.0, 0.99),
    (273.0, 1.0),
];

/// App Exposé, closing (ms, 0–1): from y 388 to 300 after Esc.
pub const APP_CLOSE_CURVE: &[(f64, f64)] = &[
    (0.0, 0.0),
    (13.0, 0.045),
    (26.0, 0.26),
    (40.0, 0.39),
    (53.0, 0.50),
    (80.0, 0.67),
    (93.0, 0.74),
    (120.0, 0.84),
    (133.0, 0.875),
    (160.0, 0.93),
    (173.0, 0.94),
    (200.0, 0.955),
    (213.0, 0.966),
    (253.0, 0.977),
    (266.0, 0.989),
    (320.0, 1.0),
];

/// App Exposé: the app windows in the middle, each with its title below. With one 900 × 600
/// window on the reference screen the scale is 0.947 and the center is at y 671.5: area from
/// 48 pt from the top to 61 pt above the Dock (room for the label). The rule with more windows
/// is not measured.
const APP_MARGIN_TOP: f64 = 48.0;
const APP_MARGIN_BOTTOM: f64 = 61.0;
const APP_MAX_SCALE: f64 = 0.947;
/// Label: size ~12 pt, white, capitals 14 pt below the window.
pub const APP_LABEL_SIZE: f32 = 12.0;
pub const APP_LABEL_GAP: f64 = 14.0;
/// At the top the wallpaper darkens: from `#6f7075` at y 0 to `#7a7b80` at y 102.
pub const APP_SHADE_HEIGHT: f64 = 102.0;
pub const APP_SHADE_ALPHA: f32 = 0.09;
/// σ of the blur of the wallpaper behind the windows, in points. Not measured: the reference
/// sessions used a solid wallpaper, where blur does not show (`missioncontrol.md`); no
/// dimming or tint, as measured.
pub const BACKDROP_BLUR_PT: f64 = 24.0;
/// Show Desktop (not measured, PA-31): the windows go out towards the nearest edge and a
/// strip of them stays visible.
const DESKTOP_PEEK: f64 = 24.0;

/// Value of a (time, progress) table at instant `ms`, interpolated.
pub fn curve_at(table: &[(f64, f64)], ms: f64) -> f64 {
    let mut previous = (0.0, 0.0);
    for &(t, p) in table {
        if ms <= t {
            let span = t - previous.0;
            if span <= 0.0 {
                return p;
            }
            return previous.1 + (p - previous.1) * (ms - previous.0) / span;
        }
        previous = (t, p);
    }
    1.0
}

/// Arranges the frames in `area` without overlapping them. If there is room at full size,
/// the windows move apart in pairs along the line through their centers ([`spread`]), as in
/// the reference; otherwise scaled rows ([`grid`]).
pub fn layout(
    frames: &[Rectangle<f64, Logical>],
    area: Rectangle<f64, Logical>,
) -> Vec<Rectangle<f64, Logical>> {
    if frames.is_empty() || area.size.w <= 0.0 || area.size.h <= 0.0 {
        return Vec::new();
    }
    if let Some(spread) = spread(frames, area) {
        return spread;
    }
    grid(frames, area)
}

fn center(r: &Rectangle<f64, Logical>) -> (f64, f64) {
    (r.loc.x + r.size.w / 2.0, r.loc.y + r.size.h / 2.0)
}

/// Overlapping windows (counting the 16 pt gap) move apart in equal parts along the line
/// through their centers, by as much as needed to separate them on one of the two axes. In
/// the reference: a file manager window (700,300 900 × 600) and a terminal (1283,308 597 × 422) move by
/// (−167, +32) and (+166, −32). Then the group comes back into the area, if it goes out. `None`
/// if it does not fit at full size.
fn spread(
    frames: &[Rectangle<f64, Logical>],
    area: Rectangle<f64, Logical>,
) -> Option<Vec<Rectangle<f64, Logical>>> {
    let mut out = frames.to_vec();
    let n = out.len();
    let mut settled = false;
    for _ in 0..64 {
        settled = true;
        for i in 0..n {
            for j in (i + 1)..n {
                let (Some(&a), Some(&b)) = (out.get(i), out.get(j)) else {
                    continue;
                };
                let (ca, cb) = (center(&a), center(&b));
                let (mut dx, mut dy) = (cb.0 - ca.0, cb.1 - ca.1);
                let need_x = (a.size.w + b.size.w) / 2.0 + WINDOW_GAP - dx.abs();
                let need_y = (a.size.h + b.size.h) / 2.0 + WINDOW_GAP - dy.abs();
                if need_x <= 1e-6 || need_y <= 1e-6 {
                    continue;
                }
                settled = false;
                // Coinciding centers: the second one to the right.
                if dx.abs() + dy.abs() < 1e-6 {
                    (dx, dy) = (1.0, 0.0);
                }
                let length = dx.hypot(dy);
                let (ux, uy) = (dx / length, dy / length);
                let along_x = if ux.abs() > 1e-6 {
                    need_x / ux.abs()
                } else {
                    f64::INFINITY
                };
                let along_y = if uy.abs() > 1e-6 {
                    need_y / uy.abs()
                } else {
                    f64::INFINITY
                };
                let half = along_x.min(along_y) / 2.0;
                if let Some(r) = out.get_mut(i) {
                    r.loc -= Point::from((ux * half, uy * half));
                }
                if let Some(r) = out.get_mut(j) {
                    r.loc += Point::from((ux * half, uy * half));
                }
            }
        }
        if settled {
            break;
        }
    }
    if !settled {
        return None;
    }
    let bounds = out.iter().copied().reduce(|a, b| a.merge(b))?;
    if bounds.size.w > area.size.w || bounds.size.h > area.size.h {
        return None;
    }
    let shift_x = (area.loc.x - bounds.loc.x).max(0.0)
        + (area.loc.x + area.size.w - (bounds.loc.x + bounds.size.w)).min(0.0);
    let shift_y = (area.loc.y - bounds.loc.y).max(0.0)
        + (area.loc.y + area.size.h - (bounds.loc.y + bounds.size.h)).min(0.0);
    for r in &mut out {
        r.loc += Point::from((shift_x, shift_y));
    }
    Some(out)
}

/// Rows balanced in vertical order, every row in horizontal order, a common scale at most 1
/// (the largest among the row splits), rows and group centered.
fn grid(
    frames: &[Rectangle<f64, Logical>],
    area: Rectangle<f64, Logical>,
) -> Vec<Rectangle<f64, Logical>> {
    grid_scaled(frames, area, 1.0)
}

/// Like [`grid`], with the scale at most `max_scale`.
fn grid_scaled(
    frames: &[Rectangle<f64, Logical>],
    area: Rectangle<f64, Logical>,
    max_scale: f64,
) -> Vec<Rectangle<f64, Logical>> {
    let n = frames.len();
    let at = |i: usize| frames.get(i).copied().unwrap_or_default();
    let mut by_y: Vec<usize> = (0..n).collect();
    by_y.sort_by(|&a, &b| {
        let ca = at(a).loc.y + at(a).size.h / 2.0;
        let cb = at(b).loc.y + at(b).size.h / 2.0;
        ca.total_cmp(&cb)
    });
    let mut best: Option<(f64, Vec<Vec<usize>>)> = None;
    for rows in 1..=n {
        let mut grouped: Vec<Vec<usize>> = Vec::with_capacity(rows);
        for r in 0..rows {
            let from = r * n / rows;
            let to = (r + 1) * n / rows;
            let mut row: Vec<usize> = by_y.get(from..to).unwrap_or_default().to_vec();
            row.sort_by(|&a, &b| at(a).loc.x.total_cmp(&at(b).loc.x));
            grouped.push(row);
        }
        // The gaps between windows are not scaled: they stay 16 pt.
        let width_scale = grouped
            .iter()
            .map(|row| {
                let gaps = WINDOW_GAP * (row.len().saturating_sub(1)) as f64;
                let widths = row.iter().map(|&i| at(i).size.w).sum::<f64>();
                (area.size.w - gaps) / widths.max(1.0)
            })
            .fold(f64::INFINITY, f64::min);
        let heights = grouped
            .iter()
            .map(|row| row.iter().map(|&i| at(i).size.h).fold(0.0, f64::max))
            .sum::<f64>();
        let height_scale = (area.size.h - WINDOW_GAP * (rows - 1) as f64) / heights.max(1.0);
        let scale = max_scale.min(width_scale).min(height_scale).max(0.05);
        if best.as_ref().is_none_or(|(s, _)| scale > *s + 1e-6) {
            best = Some((scale, grouped));
        }
    }
    let Some((scale, grouped)) = best else {
        return Vec::new();
    };
    let mut out = frames.to_vec();
    let heights: Vec<f64> = grouped
        .iter()
        .map(|row| {
            row.iter()
                .map(|&i| at(i).size.h * scale)
                .fold(0.0, f64::max)
        })
        .collect();
    let total = heights.iter().sum::<f64>() + WINDOW_GAP * (grouped.len() - 1) as f64;
    let mut y = area.loc.y + (area.size.h - total) / 2.0;
    for (row, height) in grouped.iter().zip(&heights) {
        let width = row.iter().map(|&i| at(i).size.w * scale).sum::<f64>()
            + WINDOW_GAP * (row.len().saturating_sub(1)) as f64;
        let mut x = area.loc.x + (area.size.w - width) / 2.0;
        for &i in row {
            let size = at(i).size.upscale(scale);
            if let Some(slot) = out.get_mut(i) {
                *slot = Rectangle::new((x, y + (height - size.h) / 2.0).into(), size);
            }
            x += size.w + WINDOW_GAP;
        }
        y += height + WINDOW_GAP;
    }
    out
}

/// The app of a window (xdg-toplevel `app_id`).
pub fn window_app(window: &Window) -> Option<String> {
    let toplevel = window.toplevel()?;
    smithay::wayland::compositor::with_states(toplevel.wl_surface(), |states| {
        states
            .data_map
            .get::<smithay::wayland::shell::xdg::XdgToplevelSurfaceData>()
            .and_then(|data| data.lock().ok())
            .and_then(|data| data.app_id.clone())
    })
}

/// Show Desktop: the frame pushed past the screen edge nearest to its center, with a strip of
/// `DESKTOP_PEEK` points still inside.
fn out_of_screen(
    frame: Rectangle<f64, Logical>,
    output: Rectangle<f64, Logical>,
) -> Rectangle<f64, Logical> {
    let (cx, cy) = center(&frame);
    let left = cx - output.loc.x;
    let right = output.loc.x + output.size.w - cx;
    let top = cy - output.loc.y;
    let bottom = output.loc.y + output.size.h - cy;
    let nearest = left.min(right).min(top).min(bottom);
    let mut out = frame;
    if nearest == left {
        out.loc.x = output.loc.x + DESKTOP_PEEK - frame.size.w;
    } else if nearest == right {
        out.loc.x = output.loc.x + output.size.w - DESKTOP_PEEK;
    } else if nearest == top {
        out.loc.y = output.loc.y + DESKTOP_PEEK - frame.size.h;
    } else {
        out.loc.y = output.loc.y + output.size.h - DESKTOP_PEEK;
    }
    out
}

/// A window in Panorama: the real frame and the arranged one.
#[derive(Clone)]
pub struct Placement {
    pub window: Window,
    pub from: Rectangle<f64, Logical>,
    pub to: Rectangle<f64, Logical>,
}

/// A window grabbed with the pointer: a click if it does not move, a drag otherwise.
#[derive(Clone)]
pub struct Drag {
    pub window: Window,
    /// Grab point inside the arranged frame, as a proportion (0–1).
    pub grab: (f64, f64),
    pub press: Point<f64, Logical>,
    pub pointer: Point<f64, Logical>,
    pub moved: bool,
}

/// The three views that move the windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// All the windows of the space, with the spaces bar.
    Panorama,
    /// The windows of an app (App Exposé).
    App,
    /// The windows off screen: the desktop shows.
    Desktop,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Panorama => "panorama",
            Mode::App => "app",
            Mode::Desktop => "desktop",
        }
    }
}

/// A dragged window dropped away from a thumbnail: it goes back to its place (BACKLOG-1 /
/// #74: ~500 ms, ease-out, no overshoot).
#[derive(Clone)]
pub struct Returning {
    pub window: Window,
    /// Where it was let go.
    pub from: Rectangle<f64, Logical>,
    pub start: Instant,
    /// Fraction of the way, eased (0–1); updated on every frame.
    pub fraction: f64,
}

/// The rectangle `fraction` of the way from `from` to `to`.
pub fn lerp_rect(
    from: Rectangle<f64, Logical>,
    to: Rectangle<f64, Logical>,
    fraction: f64,
) -> Rectangle<f64, Logical> {
    let lerp = |a: f64, b: f64| a + (b - a) * fraction;
    Rectangle::new(
        (lerp(from.loc.x, to.loc.x), lerp(from.loc.y, to.loc.y)).into(),
        (lerp(from.size.w, to.size.w), lerp(from.size.h, to.size.h)).into(),
    )
}

/// The eased fraction of the way back after `elapsed`, durations scaled by `speed`.
pub fn return_fraction(elapsed: std::time::Duration, speed: f64) -> f64 {
    ossido_render::animation::ease_out(ossido_render::animation::progress(
        elapsed,
        ossido_render::animation::PANORAMA_RETURN,
        speed,
    ))
}

pub struct Panorama {
    pub mode: Mode,
    pub opening: bool,
    pub start: Instant,
    /// Progress the phase restarts from (an Esc halfway through opening does not jump).
    pub from: f64,
    pub progress: f64,
    pub placements: Vec<Placement>,
    /// Spaces bar expanded (or expanding), with its animation.
    pub expanded: bool,
    pub expand_start: Instant,
    pub expand_from: f64,
    pub expansion: f64,
    pub drag: Option<Drag>,
    /// The window going back to its place after a drop that changed nothing.
    pub returning: Option<Returning>,
}

/// A space thumbnail in the frame: where it is and the windows of the space (from bottom to
/// top, at their positions on the desktop).
#[derive(Clone)]
pub struct Thumbnail {
    pub rect: Rectangle<f64, Logical>,
    pub windows: Vec<(Window, Point<i32, Logical>)>,
}

/// What the frame needs to know about Panorama.
#[derive(Clone)]
pub struct PanoramaView {
    pub mode: Mode,
    pub progress: f64,
    pub placements: Vec<Placement>,
    pub expansion: f64,
    /// The thumbnails, only when visible.
    pub thumbnails: Vec<Thumbnail>,
    /// The dragged window and its frame at this moment.
    pub drag: Option<(Window, Rectangle<f64, Logical>)>,
    /// The label of each space.
    pub labels: Vec<String>,
}

/// An entry of the spaces bar, in points in the output.
struct BarItem {
    index: usize,
    label: String,
    rect: Rectangle<f64, Logical>,
}

/// The label of a space in the spaces bar, in the system language (English source, Italian
/// translation; `LANGUAGE`, then `LC_ALL`, `LC_MESSAGES`, `LANG`).
fn space_label(index: usize) -> String {
    space_label_in(index, &system_language(|key| std::env::var(key).ok()))
}

/// The label of the `index`-th desktop (counting desktops only).
pub fn desktop_label(index: usize) -> String {
    space_label(index)
}

/// The label of a full-screen space: the window title, or its app id without the reverse
/// domain when there is no title.
pub fn window_label(window: &Window) -> String {
    use smithay::wayland::compositor::with_states;
    use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
    let (title, app_id) = window
        .toplevel()
        .map(|toplevel| {
            with_states(toplevel.wl_surface(), |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .and_then(|data| data.lock().ok())
                    .map(|data| (data.title.clone(), data.app_id.clone()))
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default();
    title
        .filter(|t| !t.is_empty())
        .or_else(|| app_id.and_then(|id| id.rsplit('.').next().map(str::to_owned)))
        .unwrap_or_default()
}

fn space_label_in(index: usize, language: &str) -> String {
    let n = index.saturating_add(1);
    match language {
        "it" => format!("Spazio {n}"),
        _ => format!("Space {n}"),
    }
}

/// The first language code of the environment (`it_IT.UTF-8` → `it`); `en` if unknown.
fn system_language(get: impl Fn(&str) -> Option<String>) -> String {
    let single = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| get(key).filter(|v| !v.is_empty()));
    let c_locale = single.as_deref().is_some_and(|v| v == "C" || v == "POSIX");
    let first = (!c_locale)
        .then(|| get("LANGUAGE"))
        .flatten()
        .and_then(|list| list.split(':').find(|s| !s.is_empty()).map(str::to_owned))
        .or(single);
    first
        .as_deref()
        .and_then(|name| name.split(['_', '-', '.', '@']).next())
        .filter(|code| !code.is_empty() && code.chars().all(|c| c.is_ascii_alphabetic()))
        .map(str::to_ascii_lowercase)
        .filter(|code| code != "c" && code != "posix")
        .unwrap_or_else(|| "en".to_owned())
}

fn bar_items(labels: &[String], width: f64) -> Vec<BarItem> {
    let mut items: Vec<BarItem> = (0..labels.len())
        .map(|index| {
            let label = labels.get(index).cloned().unwrap_or_default();
            let w =
                f64::from(text_width(&label, LABEL_SIZE, Weight::Regular)) + 2.0 * LABEL_PADDING;
            BarItem {
                index,
                label,
                rect: Rectangle::new((0.0, CAPSULE_TOP).into(), (w, CAPSULE_HEIGHT).into()),
            }
        })
        .collect();
    let total = items.iter().map(|i| i.rect.size.w).sum::<f64>()
        + LABEL_GAP * labels.len().saturating_sub(1) as f64;
    let mut x = (width - total) / 2.0;
    for item in &mut items {
        item.rect.loc.x = x;
        x += item.rect.size.w + LABEL_GAP;
    }
    items
}

/// The space thumbnails in the expanded bar: `count` thumbnails plus an empty slot,
/// centered; if they do not fit they all shrink together.
pub fn thumbnail_rects(count: usize, output: Size<f64, Logical>) -> Vec<Rectangle<f64, Logical>> {
    if output.w <= 0.0 || output.h <= 0.0 {
        return Vec::new();
    }
    let slots = count.saturating_add(1) as f64;
    let natural = slots * THUMB_WIDTH + (slots - 1.0) * THUMB_GAP;
    // Margin for the "+" on the right and symmetric on the left.
    let room = output.w - 4.0 * PLUS_RIGHT;
    let shrink = (room / natural).clamp(0.05, 1.0);
    let width = THUMB_WIDTH * shrink;
    let gap = THUMB_GAP * shrink;
    let height = width * output.h / output.w;
    let x0 = (output.w - natural * shrink) / 2.0;
    (0..count)
        .map(|i| {
            Rectangle::new(
                (x0 + i as f64 * (width + gap), THUMB_TOP).into(),
                (width, height).into(),
            )
        })
        .collect()
}

fn plus_rect(width: f64, expansion: f64) -> Rectangle<f64, Logical> {
    let y = PLUS_Y + (PLUS_Y_EXPANDED - PLUS_Y) * expansion;
    Rectangle::new(
        (width - PLUS_RIGHT - 16.0, y - 16.0).into(),
        (32.0, 32.0).into(),
    )
}

/// Bar height for an expansion 0–1.
pub fn bar_height(expansion: f64) -> f64 {
    BAR_HEIGHT + (EXPANDED_HEIGHT - BAR_HEIGHT) * expansion.clamp(0.0, 1.0)
}

/// The frame at this moment of the animation.
pub fn interpolate(p: &Placement, progress: f64) -> Rectangle<f64, Logical> {
    let lerp = |a: f64, b: f64| a + (b - a) * progress;
    Rectangle::new(
        (
            lerp(p.from.loc.x, p.to.loc.x),
            lerp(p.from.loc.y, p.to.loc.y),
        )
            .into(),
        (
            lerp(p.from.size.w, p.to.size.w),
            lerp(p.from.size.h, p.to.size.h),
        )
            .into(),
    )
}

/// The frame of the dragged window: it follows the pointer from the grab point; over the
/// bar it shrinks.
fn drag_rect(drag: &Drag, frame: Rectangle<f64, Logical>, bar: f64) -> Rectangle<f64, Logical> {
    let scale = if drag.pointer.y < bar {
        DRAG_OVER_BAR_SCALE
    } else {
        1.0
    };
    let size = frame.size.upscale(scale);
    Rectangle::new(
        (
            drag.pointer.x - drag.grab.0 * size.w,
            drag.pointer.y - drag.grab.1 * size.h,
        )
            .into(),
        size,
    )
}

impl State {
    pub fn panorama_open(&self) -> bool {
        self.panorama.as_ref().is_some_and(|p| p.opening)
    }

    fn output_size(&self) -> Size<f64, Logical> {
        self.space
            .output_geometry(&self.output)
            .map_or_else(Size::default, |geo| geo.size.to_f64())
    }

    /// The open view (not closing).
    pub fn panorama_mode(&self) -> Option<Mode> {
        self.panorama.as_ref().filter(|p| p.opening).map(|p| p.mode)
    }

    /// Opens Panorama, or closes it if it is open.
    pub fn toggle_panorama(&mut self, now: Instant) {
        self.toggle_view(Mode::Panorama, now);
    }

    /// Opens a view; if it is already open it closes it, if another one is open it switches to this.
    pub fn toggle_view(&mut self, mode: Mode, now: Instant) {
        if self.panorama_mode() == Some(mode) {
            self.close_panorama(now, None);
        } else {
            self.open_view(mode, now);
        }
    }

    /// The windows of the space (for App Exposé only those of the frontmost app) with the
    /// real frame and the arranged one.
    fn panorama_placements(&self, mode: Mode) -> Vec<Placement> {
        let mut windows: Vec<Window> = self.space.elements().cloned().collect();
        if mode == Mode::App {
            let Some(focused) = self.focused_window() else {
                return Vec::new();
            };
            let app = window_app(&focused);
            windows.retain(|w| *w == focused || (app.is_some() && window_app(w) == app));
        }
        let frames: Vec<Rectangle<f64, Logical>> = windows
            .iter()
            .filter_map(|w| {
                let geometry = self.space.element_geometry(w)?;
                Some(crate::decorations::frame_of(w, geometry).to_f64())
            })
            .collect();
        if frames.len() != windows.len() {
            return Vec::new();
        }
        let output = self
            .space
            .output_geometry(&self.output)
            .unwrap_or_default()
            .to_f64();
        let usable = self.usable_area().to_f64();
        let targets = match mode {
            Mode::Panorama => {
                let top = output.loc.y + BAR_HEIGHT + MARGIN_TOP;
                let bottom = usable.loc.y + usable.size.h - MARGIN_BOTTOM;
                let area = Rectangle::new(
                    (output.loc.x + MARGIN_SIDE, top).into(),
                    (output.size.w - 2.0 * MARGIN_SIDE, (bottom - top).max(1.0)).into(),
                );
                layout(&frames, area)
            }
            Mode::App => {
                let top = output.loc.y + APP_MARGIN_TOP;
                let bottom = usable.loc.y + usable.size.h - APP_MARGIN_BOTTOM;
                let area = Rectangle::new(
                    (output.loc.x + MARGIN_SIDE, top).into(),
                    (output.size.w - 2.0 * MARGIN_SIDE, (bottom - top).max(1.0)).into(),
                );
                if frames.is_empty() {
                    Vec::new()
                } else {
                    grid_scaled(&frames, area, APP_MAX_SCALE)
                }
            }
            Mode::Desktop => frames.iter().map(|f| out_of_screen(*f, output)).collect(),
        };
        windows
            .into_iter()
            .zip(frames)
            .zip(targets)
            .map(|((window, from), to)| Placement { window, from, to })
            .collect()
    }

    fn open_view(&mut self, mode: Mode, now: Instant) {
        self.settle_transition();
        // From another view it restarts from zero (the windows go back to their place at once).
        let from = self
            .panorama
            .as_ref()
            .filter(|p| p.mode == mode)
            .map_or(0.0, |p| p.progress);
        let placements = self.panorama_placements(mode);
        self.titlebar_hover = None;
        self.panorama = Some(Panorama {
            mode,
            opening: true,
            start: now,
            from,
            progress: from,
            placements,
            expanded: false,
            expand_start: now,
            expand_from: 0.0,
            expansion: 0.0,
            drag: None,
            returning: None,
        });
        if self.animation_speed <= 0.0 {
            self.advance_panorama(now);
        }
    }

    /// Closes Panorama; with `focus` it brings that window to the front.
    pub fn close_panorama(&mut self, now: Instant, focus: Option<Window>) {
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        if panorama.opening {
            panorama.opening = false;
            panorama.from = panorama.progress;
            panorama.start = now;
            panorama.drag = None;
            panorama.returning = None;
        }
        if let Some(window) = focus {
            self.focus_window(Some(window));
        }
        if self.animation_speed <= 0.0 {
            self.advance_panorama(now);
        }
    }

    /// Expands or collapses the spaces bar.
    pub fn expand_spaces_bar(&mut self, expanded: bool, now: Instant) {
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        if panorama.expanded == expanded || !panorama.opening || panorama.mode != Mode::Panorama {
            return;
        }
        panorama.expanded = expanded;
        panorama.expand_from = panorama.expansion;
        panorama.expand_start = now;
        if self.animation_speed <= 0.0 {
            self.advance_panorama(now);
        }
    }

    /// Advances the animations (on every frame); once fully closed, Panorama disappears.
    pub fn advance_panorama(&mut self, now: Instant) {
        let speed = self.animation_speed;
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        let elapsed = |start: Instant| {
            let ms = now.saturating_duration_since(start).as_secs_f64() * 1000.0;
            if speed <= 0.0 { f64::MAX } else { ms / speed }
        };
        let ms = elapsed(panorama.start);
        let (open, close) = match panorama.mode {
            Mode::App => (APP_OPEN_CURVE, APP_CLOSE_CURVE),
            Mode::Panorama | Mode::Desktop => (OPEN_CURVE, CLOSE_CURVE),
        };
        panorama.progress = if panorama.opening {
            panorama.from + (1.0 - panorama.from) * curve_at(open, ms)
        } else {
            panorama.from * (1.0 - curve_at(close, ms))
        };
        let step = curve_at(EXPAND_CURVE, elapsed(panorama.expand_start));
        panorama.expansion = if panorama.expanded {
            panorama.expand_from + (1.0 - panorama.expand_from) * step
        } else {
            panorama.expand_from * (1.0 - step)
        };
        if let Some(back) = panorama.returning.as_mut() {
            back.fraction = return_fraction(now.saturating_duration_since(back.start), speed);
            if back.fraction >= 1.0 {
                panorama.returning = None;
            }
        }
        if !panorama.opening && panorama.progress <= 0.0 {
            self.panorama = None;
        }
    }

    /// The dragged window is let go away from any thumbnail: it goes back to its place.
    fn start_return(&mut self, drag: &Drag, now: Instant) {
        if self.animation_speed <= 0.0 {
            return;
        }
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        let Some(placement) = panorama.placements.iter().find(|p| p.window == drag.window) else {
            return;
        };
        let from = drag_rect(drag, placement.to, bar_height(panorama.expansion));
        panorama.returning = Some(Returning {
            window: drag.window.clone(),
            from,
            start: now,
            fraction: 0.0,
        });
    }

    /// The windows of every space at their positions (those of the active space from the Space).
    fn space_contents(&self) -> Vec<Vec<(Window, Point<i32, Logical>)>> {
        (0..self.workspaces.count())
            .map(|index| {
                if index == self.workspaces.active {
                    self.space
                        .elements()
                        .filter_map(|w| Some((w.clone(), self.space.element_location(w)?)))
                        .collect()
                } else {
                    self.workspaces
                        .stored
                        .get(index)
                        .cloned()
                        .unwrap_or_default()
                }
            })
            .collect()
    }

    pub fn panorama_view(&self) -> Option<PanoramaView> {
        let panorama = self.panorama.as_ref()?;
        let thumbnails = if panorama.expansion >= THUMBS_FROM {
            thumbnail_rects(self.workspaces.count(), self.output_size())
                .into_iter()
                .zip(self.space_contents())
                .map(|(rect, windows)| Thumbnail { rect, windows })
                .collect()
        } else {
            Vec::new()
        };
        let dragging = panorama.drag.as_ref().filter(|d| d.moved).and_then(|drag| {
            let placement = panorama
                .placements
                .iter()
                .find(|p| p.window == drag.window)?;
            Some((
                drag.window.clone(),
                drag_rect(drag, placement.to, bar_height(panorama.expansion)),
            ))
        });
        let drag = dragging.or_else(|| {
            let back = panorama.returning.as_ref()?;
            let placement = panorama
                .placements
                .iter()
                .find(|p| p.window == back.window)?;
            Some((
                back.window.clone(),
                lerp_rect(back.from, placement.to, back.fraction),
            ))
        });
        Some(PanoramaView {
            mode: panorama.mode,
            progress: panorama.progress,
            placements: panorama.placements.clone(),
            expansion: panorama.expansion,
            thumbnails,
            drag,
            labels: self.space_labels(),
        })
    }

    /// The pointer moves with Panorama open: at the top the bar expands, below the expanded
    /// bar (outside the bar) it collapses; a grabbed window follows the pointer.
    pub fn panorama_pointer(&mut self, location: Point<f64, Logical>, now: Instant) {
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        if !panorama.opening {
            return;
        }
        if panorama.mode != Mode::Panorama {
            return;
        }
        if let Some(drag) = panorama.drag.as_mut() {
            drag.pointer = location;
            let (dx, dy) = (location.x - drag.press.x, location.y - drag.press.y);
            if dx.hypot(dy) > DRAG_THRESHOLD {
                drag.moved = true;
            }
        }
        let expanded = panorama.expanded;
        if !expanded && location.y < BAR_HEIGHT {
            self.expand_spaces_bar(true, now);
        } else if expanded && location.y > EXPANDED_HEIGHT {
            self.expand_spaces_bar(false, now);
        }
    }

    /// The space of the thumbnail under the point, with the bar expanded.
    fn thumbnail_at(&self, location: Point<f64, Logical>) -> Option<usize> {
        let panorama = self.panorama.as_ref()?;
        if panorama.expansion < THUMBS_FROM {
            return None;
        }
        thumbnail_rects(self.workspaces.count(), self.output_size())
            .iter()
            .position(|rect| rect.contains(location))
    }

    /// Press with Panorama open: an entry of the bar, the "+", a thumbnail, a window (which
    /// can be dragged) or the empty area.
    pub fn panorama_press_at(&mut self, location: Point<f64, Logical>, now: Instant) {
        let Some(panorama) = self.panorama.as_ref() else {
            return;
        };
        if !panorama.opening {
            return;
        }
        let expansion = panorama.expansion;
        let progress = panorama.progress;
        let width = self.output_size().w;
        // With the desktop in view a click puts the windows back in their place.
        if panorama.mode == Mode::Desktop {
            self.close_panorama(now, None);
            return;
        }
        if panorama.mode == Mode::Panorama && location.y < bar_height(expansion) {
            if plus_rect(width, expansion).contains(location) {
                // The bar redraws by itself: its key is the number of spaces.
                if let Err(err) = self.add_space() {
                    tracing::info!("panorama: {err}");
                }
                return;
            }
            let index = if expansion < THUMBS_FROM {
                bar_items(&self.space_labels(), width)
                    .iter()
                    .find(|item| item.rect.contains(location))
                    .map(|item| item.index)
            } else {
                self.thumbnail_at(location)
            };
            if let Some(index) = index {
                self.close_panorama(now, None);
                self.switch_space(index, now);
            }
            return;
        }
        let hit = panorama.placements.iter().rev().find_map(|placement| {
            let rect = interpolate(placement, progress);
            rect.contains(location).then(|| {
                let grab = (
                    (location.x - rect.loc.x) / rect.size.w.max(1.0),
                    (location.y - rect.loc.y) / rect.size.h.max(1.0),
                );
                (placement.window.clone(), grab)
            })
        });
        match hit {
            Some((window, grab)) => {
                if let Some(panorama) = self.panorama.as_mut() {
                    panorama.drag = Some(Drag {
                        window,
                        grab,
                        press: location,
                        pointer: location,
                        moved: false,
                    });
                }
            }
            None => self.close_panorama(now, None),
        }
    }

    /// Release: a window not moved goes to the front and Panorama closes; one dragged onto a
    /// thumbnail moves to that space, elsewhere it goes back to its place.
    pub fn panorama_release(&mut self, location: Point<f64, Logical>, now: Instant) {
        let Some(drag) = self.panorama.as_mut().and_then(|p| p.drag.take()) else {
            return;
        };
        if !drag.moved {
            self.close_panorama(now, Some(drag.window));
            return;
        }
        if self
            .panorama
            .as_ref()
            .is_none_or(|p| p.mode != Mode::Panorama)
        {
            return;
        }
        let Some(index) = self
            .thumbnail_at(location)
            .filter(|index| *index != self.workspaces.active)
        else {
            self.start_return(&drag, now);
            return;
        };
        if let Err(err) = self.move_window_to_space(&drag.window, index) {
            tracing::info!("panorama: {err}");
            self.start_return(&drag, now);
            return;
        }
        // The remaining windows are rearranged right away.
        let placements = self.panorama_placements(Mode::Panorama);
        if let Some(panorama) = self.panorama.as_mut() {
            panorama.placements = placements;
        }
    }

    /// For `panorama.state`.
    pub fn panorama_state(&self) -> PanoramaState {
        let Some(panorama) = self.panorama.as_ref() else {
            return PanoramaState {
                mode: None,
                open: false,
                progress: 0.0,
                windows: Vec::new(),
                expansion: 0.0,
                thumbnails: Vec::new(),
            };
        };
        let rect = |r: Rectangle<f64, Logical>| PanoramaRect {
            x: r.loc.x,
            y: r.loc.y,
            width: r.size.w,
            height: r.size.h,
        };
        PanoramaState {
            mode: Some(panorama.mode.name().to_owned()),
            open: panorama.opening,
            progress: panorama.progress,
            windows: panorama
                .placements
                .iter()
                .filter_map(|p| {
                    Some(PanoramaWindow {
                        id: State::window_id(&p.window)?,
                        x: p.to.loc.x,
                        y: p.to.loc.y,
                        width: p.to.size.w,
                        height: p.to.size.h,
                    })
                })
                .collect(),
            expansion: panorama.expansion,
            thumbnails: if panorama.expansion >= THUMBS_FROM {
                thumbnail_rects(self.workspaces.count(), self.output_size())
                    .into_iter()
                    .map(rect)
                    .collect()
            } else {
                Vec::new()
            },
        }
    }
}

fn mix(a: [u8; 4], b: [u8; 4], t: f64) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let mut out = [0; 4];
    for (o, (x, y)) in out.iter_mut().zip(a.iter().zip(b)) {
        *o = (f64::from(*x) + (f64::from(y) - f64::from(*x)) * t).round() as u8;
    }
    out
}

/// The tint (premultiplied, 0–255) that over the reference gray gives `target` with this
/// opacity.
fn tint(target: [u8; 4], alpha: f64) -> [f64; 4] {
    let mut out = [0.0, 0.0, 0.0, alpha * 255.0];
    for (o, (t, b)) in out.iter_mut().zip(target.iter().zip(REFERENCE_WALLPAPER)) {
        *o = (f64::from(*t) - (1.0 - alpha) * f64::from(b)).clamp(0.0, alpha * 255.0);
    }
    out
}

/// The tint of the bar fill (straight RGBA) at `toward_bottom` (0 at the top, 1 at the bottom
/// of the expanded bar).
fn bar_tint(dark: bool, toward_bottom: f64) -> [u8; 4] {
    let (top, bottom) = if dark {
        (
            tint(BAR_FILL_DARK, BAR_TINT_ALPHA_DARK),
            tint(EXPANDED_FILL_BOTTOM_DARK, BAR_TINT_ALPHA_DARK),
        )
    } else {
        (
            tint(BAR_FILL, BAR_TINT_ALPHA),
            tint(EXPANDED_FILL_BOTTOM, BAR_TINT_ALPHA_BOTTOM),
        )
    };
    let t = toward_bottom.clamp(0.0, 1.0);
    let mut premultiplied = [0.0; 4];
    for (p, (a, b)) in premultiplied.iter_mut().zip(top.iter().zip(bottom)) {
        *p = a + (b - a) * t;
    }
    let alpha = premultiplied[3].max(1e-6);
    let [r, g, b, a] = premultiplied;
    [
        (r * 255.0 / alpha).round().clamp(0.0, 255.0) as u8,
        (g * 255.0 / alpha).round().clamp(0.0, 255.0) as u8,
        (b * 255.0 / alpha).round().clamp(0.0, 255.0) as u8,
        a.round().clamp(0.0, 255.0) as u8,
    ]
}

fn faded(color: [u8; 4], alpha: f64) -> [u8; 4] {
    let [r, g, b, a] = color;
    [
        r,
        g,
        b,
        (f64::from(a) * alpha.clamp(0.0, 1.0)).round() as u8,
    ]
}

/// What is needed to draw the bar.
pub struct BarLook {
    pub width: f64,
    pub output: Size<f64, Logical>,
    pub count: usize,
    pub active: usize,
    pub expansion: f64,
    /// Color of the thumbnails behind the wallpaper (the desktop's).
    pub backdrop: [u8; 4],
    /// The label of each space (desktops and full-screen apps).
    pub labels: Vec<String>,
    /// Dark appearance.
    pub dark: bool,
}

impl BarLook {
    /// Height of the sprite: bar, border and shadow.
    fn sprite_height(&self) -> f64 {
        let shadow = BAR_SHADOW + (EXPANDED_SHADOW - BAR_SHADOW) * self.expansion;
        bar_height(self.expansion) + 1.0 + shadow
    }

    /// Cache key: it changes with what is visible (height at the whole point).
    fn key(&self) -> String {
        format!(
            "{}:{}:{}:{}:{:?}:{}",
            self.dark,
            self.count,
            self.active,
            (self.expansion * 1000.0).round(),
            self.backdrop,
            self.labels.join("\u{1f}")
        )
    }
}

/// Draws the spaces bar (compact, expanded or halfway) with the shadow below.
pub fn draw_bar(canvas: &mut Canvas, look: &BarLook) {
    let e = look.expansion.clamp(0.0, 1.0);
    let w = look.width as f32;
    let height = bar_height(e);
    // Fill: uniform when compact, with the gradient at the bottom when expanded.
    let rows = height.ceil() as usize;
    for row in 0..rows {
        let y = row as f64;
        let toward_bottom = (y / EXPANDED_FILL_FROM).min(1.0) * e;
        let color = bar_tint(look.dark, toward_bottom);
        let h = (height - y).min(1.0) as f32;
        canvas.fill_rect([0.0, row as f32, w, h], color);
    }
    let bar = height as f32;
    canvas.fill_rect([0.0, bar, w, 1.0], mix(BAR_BORDER, EXPANDED_BORDER, e));
    // Shadow: black fading to zero.
    let shadow = BAR_SHADOW + (EXPANDED_SHADOW - BAR_SHADOW) * e;
    let rows = shadow as usize;
    for row in 0..rows {
        let t = 1.0 - row as f32 / rows as f32;
        let alpha = (BAR_SHADOW_ALPHA * t * t * 255.0).round() as u8;
        canvas.fill_rect([0.0, bar + 1.0 + row as f32, w, 1.0], [0, 0, 0, alpha]);
    }
    let (capsule_fill, capsule_border, label_color, plus_color) = if look.dark {
        (
            CAPSULE_FILL_DARK,
            CAPSULE_BORDER_DARK,
            LABEL_COLOR_DARK,
            PLUS_COLOR_DARK,
        )
    } else {
        (CAPSULE_FILL, CAPSULE_BORDER, LABEL_COLOR, PLUS_COLOR)
    };
    // The compact entries vanish at the start of the expansion.
    let compact = 1.0 - e / 0.3;
    if compact > 0.0 {
        for item in bar_items(&look.labels, look.width) {
            let r = item.rect;
            if item.index == look.active {
                let rect = [
                    r.loc.x as f32,
                    r.loc.y as f32,
                    r.size.w as f32,
                    r.size.h as f32,
                ];
                canvas.fill_rounded(rect, CAPSULE_RADIUS, faded(capsule_fill, compact));
                canvas.stroke_rounded(rect, CAPSULE_RADIUS, 1.0, faded(capsule_border, compact));
            }
            // Baseline: ~10 pt capitals centered in the capsule.
            let baseline = (CAPSULE_TOP + CAPSULE_HEIGHT / 2.0 + 5.0) as f32;
            canvas.text(
                [(r.loc.x + LABEL_PADDING) as f32, baseline],
                &item.label,
                LABEL_SIZE,
                Weight::Regular,
                faded(label_color, compact),
            );
        }
    }
    // The thumbnails: the background (the desktop wallpaper is drawn over it), the ring and
    // the label.
    let shown = (e - THUMBS_FROM) / (THUMBS_FULL - THUMBS_FROM);
    if shown > 0.0 {
        for (index, r) in thumbnail_rects(look.count, look.output)
            .into_iter()
            .enumerate()
        {
            let rect = [
                r.loc.x as f32,
                r.loc.y as f32,
                r.size.w as f32,
                r.size.h as f32,
            ];
            canvas.fill_rect(rect, faded(look.backdrop, shown));
            if index == look.active {
                let inset = (RING_GAP + RING_WIDTH / 2.0) as f32;
                canvas.stroke_rounded(
                    [
                        rect[0] - inset,
                        rect[1] - inset,
                        rect[2] + 2.0 * inset,
                        rect[3] + 2.0 * inset,
                    ],
                    inset + 1.0,
                    RING_WIDTH as f32,
                    faded(RING_COLOR, shown),
                );
            }
            let label = look.labels.get(index).cloned().unwrap_or_default();
            let text = f64::from(text_width(&label, THUMB_LABEL_SIZE, Weight::Regular));
            canvas.text(
                [
                    (r.loc.x + (r.size.w - text) / 2.0) as f32,
                    (r.loc.y + r.size.h + THUMB_LABEL_BASELINE) as f32,
                ],
                &label,
                THUMB_LABEL_SIZE,
                Weight::Regular,
                faded(label_color, shown),
            );
        }
    }
    let plus = plus_rect(look.width, e);
    let cx = (plus.loc.x + plus.size.w / 2.0) as f32;
    let cy = (plus.loc.y + plus.size.h / 2.0) as f32;
    let half = (PLUS_SIZE / 2.0) as f32 - 1.0;
    canvas.line([cx - half, cy], [cx + half, cy], 2.0, plus_color);
    canvas.line([cx, cy - half], [cx, cy + half], 2.0, plus_color);
}

/// The spaces bar as an element, already moved up as much as the opening needs.
pub fn bar_element(
    renderer: &mut smithay::backend::renderer::gles::GlesRenderer,
    sprite: &Sprite,
    look: &BarLook,
    progress: f64,
    scale: f64,
) -> Option<
    smithay::backend::renderer::element::texture::TextureRenderElement<
        smithay::backend::renderer::gles::GlesTexture,
    >,
> {
    sprite.element(
        renderer,
        &look.key(),
        Size::from((look.width, look.sprite_height())),
        Point::from((0.0, bar_top(look.expansion, progress))),
        scale,
        bar_alpha(progress),
        |canvas| draw_bar(canvas, look),
    )
}

/// The top edge of the bar: its bottom edge comes down from 0 to 40 pt on the window curve.
fn bar_top(expansion: f64, progress: f64) -> f64 {
    -(1.0 - progress) * (bar_height(expansion) + 1.0)
}

/// The opacity of the bar while it comes down: the shadow, which at rest is below, appears
/// on the first frame.
pub fn bar_alpha(progress: f64) -> f32 {
    (progress * 4.0).clamp(0.0, 1.0) as f32
}

/// Where the bar material (blur of what is under the bar) shows: the bar fill, without
/// the border and the shadow.
pub fn bar_material_rect(width: f64, expansion: f64, progress: f64) -> Rectangle<f64, Logical> {
    Rectangle::new(
        (0.0, bar_top(expansion, progress)).into(),
        (width, bar_height(expansion)).into(),
    )
}

/// The area captured for the bar material: the whole expanded bar and as much below it as
/// the blur reaches, so it is not redone while the bar expands.
pub fn bar_capture_rect(width: f64) -> Rectangle<f64, Logical> {
    Rectangle::new(
        (0.0, 0.0).into(),
        (width, EXPANDED_HEIGHT + 3.0 * BAR_BLUR_PT).into(),
    )
}

#[cfg(test)]
mod tests {
    use super::{lerp_rect, return_fraction};
    use smithay::utils::{Logical, Rectangle};
    use std::time::Duration;

    #[test]
    fn a_dropped_window_goes_back_in_half_a_second_without_overshoot() {
        let ms = Duration::from_millis;
        assert_eq!(return_fraction(ms(0), 1.0), 0.0);
        assert_eq!(return_fraction(ms(500), 1.0), 1.0);
        assert_eq!(return_fraction(ms(900), 1.0), 1.0);
        // Ease-out: more than half of the way at half the time, never past the end.
        assert!(return_fraction(ms(250), 1.0) > 0.5);
        let mut last = 0.0;
        for t in 0..=500 {
            let f = return_fraction(ms(t), 1.0);
            assert!((0.0..=1.0).contains(&f) && f >= last, "{t} ms: {f}");
            last = f;
        }
        // Slowed down x2 it takes twice as long; with animations off it is immediate.
        assert!(return_fraction(ms(500), 2.0) < 1.0);
        assert_eq!(return_fraction(ms(0), 0.0), 1.0);
    }

    #[test]
    fn the_return_rectangle_interpolates_position_and_size() {
        let from = Rectangle::<f64, Logical>::new((10.0, 20.0).into(), (100.0, 50.0).into());
        let to = Rectangle::<f64, Logical>::new((110.0, 120.0).into(), (200.0, 150.0).into());
        assert_eq!(lerp_rect(from, to, 0.0), from);
        assert_eq!(lerp_rect(from, to, 1.0), to);
        let half = lerp_rect(from, to, 0.5);
        assert_eq!(half.loc, (60.0, 70.0).into());
        assert_eq!(half.size, (150.0, 100.0).into());
    }

    #[test]
    fn space_labels_follow_the_system_language() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |key: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        assert_eq!(
            super::system_language(env(&[("LANG", "it_IT.UTF-8")])),
            "it"
        );
        assert_eq!(
            super::system_language(env(&[("LANGUAGE", "en:it"), ("LANG", "it_IT.UTF-8")])),
            "en"
        );
        assert_eq!(
            super::system_language(env(&[("LANG", "C"), ("LANGUAGE", "it")])),
            "en"
        );
        assert_eq!(super::system_language(env(&[])), "en");
        assert_eq!(super::space_label_in(0, "it"), "Spazio 1");
        assert_eq!(super::space_label_in(1, "en"), "Space 2");
        assert_eq!(super::space_label_in(2, "de"), "Space 3");
    }

    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle<f64, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn two_windows_that_fit_keep_their_size() {
        // The reference: file manager 900 × 600 and terminal 597 × 422 go to (533,332) and
        // (1449,276), scale 1, 16 pt between them (measurements at ±1 pt).
        let area = rect(48.0, 88.0, 2464.0, 1220.0);
        let out = layout(
            &[
                rect(700.0, 300.0, 900.0, 600.0),
                rect(1283.0, 308.0, 597.0, 422.0),
            ],
            area,
        );
        assert_eq!(out[0].size, Size::from((900.0, 600.0)));
        assert_eq!(out[1].size, Size::from((597.0, 422.0)));
        assert!((out[1].loc.x - (out[0].loc.x + 900.0) - WINDOW_GAP).abs() < 1e-6);
        for (got, want) in [(out[0].loc, (533.0, 332.0)), (out[1].loc, (1449.0, 276.0))] {
            assert!(
                (got.x - want.0).abs() <= 1.5 && (got.y - want.1).abs() <= 1.5,
                "{got:?} instead of {want:?}"
            );
        }
        // Already separate: they stay where they are.
        let apart = [
            rect(100.0, 100.0, 300.0, 200.0),
            rect(900.0, 500.0, 300.0, 200.0),
        ];
        assert_eq!(layout(&apart, area), apart);
    }

    #[test]
    fn many_windows_shrink_into_rows_without_overlap() {
        let area = rect(48.0, 88.0, 2464.0, 1220.0);
        let frames: Vec<_> = (0..12)
            .map(|i| {
                rect(
                    100.0 + 40.0 * f64::from(i),
                    100.0 + 30.0 * f64::from(i),
                    700.0,
                    500.0,
                )
            })
            .collect();
        let out = layout(&frames, area);
        assert_eq!(out.len(), 12);
        for (i, a) in out.iter().enumerate() {
            assert!(a.size.w <= 700.0 && a.size.w > 100.0);
            assert!(area.contains_rect(*a), "{a:?} outside {area:?}");
            for b in out.iter().skip(i + 1) {
                assert!(!a.overlaps(*b), "{a:?} {b:?}");
            }
        }
        assert!(layout(&[], area).is_empty());
    }

    #[test]
    fn the_bar_tint_over_the_reference_gray_gives_the_measured_fills() {
        let over = |tint: [u8; 4]| -> [u8; 3] {
            let a = f64::from(tint[3]) / 255.0;
            let mut out = [0; 3];
            for (o, (c, b)) in out.iter_mut().zip(tint.iter().zip(REFERENCE_WALLPAPER)) {
                *o = (f64::from(*c) * a + f64::from(b) * (1.0 - a)).round() as u8;
            }
            out
        };
        for (dark, at, want) in [
            (false, 0.0, BAR_FILL),
            (false, 1.0, EXPANDED_FILL_BOTTOM),
            (true, 0.0, BAR_FILL_DARK),
            (true, 1.0, EXPANDED_FILL_BOTTOM_DARK),
        ] {
            let tint = bar_tint(dark, at);
            // Translucent: what is under it shows through.
            assert!(tint[3] < 128, "{tint:?}");
            let got = over(tint);
            for (g, w) in got.iter().zip(want) {
                assert!(
                    g.abs_diff(w) <= 1,
                    "dark {dark} at {at}: {got:?} instead of {want:?}"
                );
            }
        }
        // Halfway, between the two.
        let half = over(bar_tint(false, 0.5));
        assert!(half[0] > BAR_FILL[0] && half[0] < EXPANDED_FILL_BOTTOM[0]);
    }

    #[test]
    fn the_bar_material_follows_the_bar_down_and_stays_in_the_capture() {
        let capture = bar_capture_rect(1920.0);
        for expansion in [0.0, 0.5, 1.0] {
            for progress in [0.0, 0.3, 1.0] {
                let r = bar_material_rect(1920.0, expansion, progress);
                assert_eq!(r.size.h, bar_height(expansion));
                assert!(r.loc.y <= 0.0 && r.loc.y + r.size.h <= capture.size.h);
            }
        }
        assert_eq!(bar_material_rect(1920.0, 0.0, 1.0).loc.y, 0.0);
        assert_eq!(bar_alpha(0.0), 0.0);
        assert_eq!(bar_alpha(1.0), 1.0);
    }

    #[test]
    fn curves_follow_the_measurements() {
        assert_eq!(curve_at(OPEN_CURVE, 0.0), 0.0);
        // Halfway between 40 and 54 ms.
        assert!(curve_at(OPEN_CURVE, 40.0) < 0.5 && curve_at(OPEN_CURVE, 54.0) > 0.5);
        assert!((curve_at(CLOSE_CURVE, 67.0) - 0.497).abs() < 1e-9);
        assert_eq!(curve_at(CLOSE_CURVE, 1000.0), 1.0);
    }
}
