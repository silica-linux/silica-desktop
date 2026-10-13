//! Panorama (M5.2): tutte le finestre dello spazio, una accanto all'altra, e la barra degli
//! spazi in alto (plan/reference/missioncontrol.md).
//!
//! - Le finestre si spostano (e rimpiccioliscono solo se non entrano) fino a non
//!   sovrapporsi: con due finestre e spazio a sufficienza la scala resta 1, si allontanano
//!   lungo la retta dei centri e fra loro ci sono 16 pt. La regola con molte finestre non è
//!   ancora misurata (RICHIESTE-panorama.md, PA-10): se non entrano a grandezza piena, righe
//!   bilanciate in scala, gruppo centrato.
//! - Lo sfondo non si oscura; la barra dei menu sparisce e al suo posto scende la barra degli
//!   spazi, alta 40 pt (`#9e9fa5`, bordo `#58595c`, ombra di ~62 pt); il Dock resta.
//! - Apertura e chiusura: rallentamento progressivo con coda lunga, 50 % in ~40–54 ms, 90 % in
//!   ~130–140 ms, fine in ~300 ms (tabelle misurate fotogramma per fotogramma).
//! - Un clic su una finestra la porta in primo piano e chiude Panorama; Esc o un clic sul vuoto
//!   chiudono; un clic su uno spazio della barra ci passa; "+" aggiunge uno spazio.

use std::time::Instant;

use ossido_render::sprite::{Canvas, Sprite, Weight, text_width};
use silica_control::commands::{PanoramaRect, PanoramaState, PanoramaWindow};
use smithay::desktop::Window;
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::state::State;

/// Barra degli spazi compatta.
pub const BAR_HEIGHT: f64 = 40.0;
pub const BAR_SHADOW: f64 = 62.0;
const BAR_FILL: [u8; 4] = [0x9e, 0x9f, 0xa5, 0xff];
const BAR_BORDER: [u8; 4] = [0x58, 0x59, 0x5c, 0xff];
/// L'ombra sotto la barra scurisce lo sfondo di ~24 livelli su 122 al bordo.
const BAR_SHADOW_ALPHA: f32 = 0.2;
const CAPSULE_FILL: [u8; 4] = [0x87, 0x88, 0x8e, 0xff];
const CAPSULE_BORDER: [u8; 4] = [0x8d, 0x8e, 0x94, 0xff];
const LABEL_COLOR: [u8; 4] = [0, 0, 0, 0xff];
const PLUS_COLOR: [u8; 4] = [0x4c, 0x4c, 0x4c, 0xff];
/// Etichette: corpo ~13–14 pt; capsula dello spazio corrente alta 24 a 7 pt dal bordo, con
/// ~11 pt di margine attorno al testo; fra una voce e l'altra 2 pt.
const LABEL_SIZE: f32 = 13.5;
const CAPSULE_TOP: f64 = 7.0;
const CAPSULE_HEIGHT: f64 = 24.0;
const CAPSULE_RADIUS: f32 = 6.5;
const LABEL_PADDING: f64 = 11.0;
const LABEL_GAP: f64 = 2.0;
/// Il "+": croce 20 × 20, tratto 2, centro a 26 pt dal bordo destro e 20 dall'alto.
const PLUS_SIZE: f64 = 20.0;
const PLUS_RIGHT: f64 = 26.0;

/// Spazio fra le finestre disposte.
pub const WINDOW_GAP: f64 = 16.0;
/// Margini dell'area delle finestre (provvisori: PA-10).
const MARGIN_SIDE: f64 = 48.0;
const MARGIN_TOP: f64 = 48.0;
const MARGIN_BOTTOM: f64 = 24.0;

/// Barra espansa (puntatore in alto): alta 170 pt con bordo `#5f6063` e ~40 pt d'ombra; il
/// riempimento scende da `#9e9fa5` a `#a4a5ab` (y 114 in giù).
pub const EXPANDED_HEIGHT: f64 = 170.0;
const EXPANDED_SHADOW: f64 = 40.0;
const EXPANDED_FILL_BOTTOM: [u8; 4] = [0xa4, 0xa5, 0xab, 0xff];
const EXPANDED_FILL_FROM: f64 = 114.0;
const EXPANDED_BORDER: [u8; 4] = [0x5f, 0x60, 0x63, 0xff];
/// Miniature: 160 pt di larghezza (16:9 sullo schermo di riferimento), a 52 pt dal bordo,
/// 32 pt fra l'una e l'altra; il gruppo lascia un posto vuoto a destra (con due spazi la
/// seconda miniatura è al centro dello schermo).
const THUMB_WIDTH: f64 = 160.0;
const THUMB_TOP: f64 = 52.0;
const THUMB_GAP: f64 = 32.0;
/// Anello dello spazio corrente: 3 pt, 1 pt di barra fra anello e miniatura, sRGB `#3b99fd`.
const RING_WIDTH: f64 = 3.0;
const RING_GAP: f64 = 1.0;
const RING_COLOR: [u8; 4] = [0x3b, 0x99, 0xfd, 0xff];
/// Etichette sotto le miniature: corpo ~12 pt, linea di base 18 pt sotto la miniatura.
const THUMB_LABEL_SIZE: f32 = 12.0;
const THUMB_LABEL_BASELINE: f64 = 18.0;
/// Il "+" scende con la barra: centro a y 20 compatta, 101,5 espansa.
const PLUS_Y: f64 = 20.0;
const PLUS_Y_EXPANDED: f64 = 101.5;
/// Le miniature compaiono a metà espansione (nel video fra il 63 % e l'80 %).
const THUMBS_FROM: f64 = 0.6;
const THUMBS_FULL: f64 = 0.8;
/// Oltre questa distanza dalla pressione il clic su una finestra diventa trascinamento.
const DRAG_THRESHOLD: f64 = 4.0;
/// La finestra trascinata sopra la barra si riduce a questa scala.
const DRAG_OVER_BAR_SCALE: f64 = 0.25;

/// Espansione della barra (ms, 0–1): altezza da 40 a 169 pt nel video, fotogramma per
/// fotogramma. La chiusura non è misurata: stessa curva.
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

/// Avanzamento dell'apertura (ms, 0–1): spostamento del Terminale nel video.
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

/// Avanzamento della chiusura (ms, 0–1): bordo del Finder nella raffica dopo Esc.
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

/// Valore di una tabella (tempo, avanzamento) all'istante `ms`, interpolato.
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

/// Dispone le cornici in `area` senza sovrapporle. Se a grandezza piena c'è posto, le
/// finestre si allontanano a coppie lungo la retta dei loro centri ([`spread`]), come nel
/// riferimento; altrimenti righe in scala ([`grid`]).
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

/// Le finestre che si sovrappongono (contando i 16 pt di distanza) si allontanano in parti
/// uguali lungo la retta dei centri, di quanto basta a separarle su uno dei due assi. Nel
/// riferimento: Finder (700,300 900 × 600) e Terminale (1283,308 597 × 422) si spostano di
/// (−167, +32) e (+166, −32). Poi il gruppo rientra nell'area, se ne esce. `None` se non
/// entra a grandezza piena.
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
                // Centri coincidenti: a destra la seconda.
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

/// Righe bilanciate nell'ordine verticale, ogni riga in ordine orizzontale, una scala
/// comune al più 1 (la più grande fra le divisioni in righe), righe e gruppo centrati.
fn grid(
    frames: &[Rectangle<f64, Logical>],
    area: Rectangle<f64, Logical>,
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
        // Gli spazi fra le finestre non si scalano: restano 16 pt.
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
        let scale = 1.0_f64.min(width_scale).min(height_scale).max(0.05);
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

/// Una finestra in Panorama: la cornice vera e quella disposta.
#[derive(Clone)]
pub struct Placement {
    pub window: Window,
    pub from: Rectangle<f64, Logical>,
    pub to: Rectangle<f64, Logical>,
}

/// Una finestra presa con il puntatore: clic se non si muove, trascinamento altrimenti.
#[derive(Clone)]
pub struct Drag {
    pub window: Window,
    /// Punto di presa dentro la cornice disposta, in proporzione (0–1).
    pub grab: (f64, f64),
    pub press: Point<f64, Logical>,
    pub pointer: Point<f64, Logical>,
    pub moved: bool,
}

pub struct Panorama {
    pub opening: bool,
    pub start: Instant,
    /// Avanzamento da cui riparte la fase (un Esc a metà apertura non salta).
    pub from: f64,
    pub progress: f64,
    pub placements: Vec<Placement>,
    /// Barra degli spazi espansa (o in espansione), con la sua animazione.
    pub expanded: bool,
    pub expand_start: Instant,
    pub expand_from: f64,
    pub expansion: f64,
    pub drag: Option<Drag>,
}

/// Una miniatura di spazio nel frame: dove sta e le finestre dello spazio (dal basso
/// verso l'alto, alle loro posizioni sulla scrivania).
#[derive(Clone)]
pub struct Thumbnail {
    pub rect: Rectangle<f64, Logical>,
    pub windows: Vec<(Window, Point<i32, Logical>)>,
}

/// Quello che il frame deve sapere di Panorama.
#[derive(Clone)]
pub struct PanoramaView {
    pub progress: f64,
    pub placements: Vec<Placement>,
    pub expansion: f64,
    /// Le miniature, solo quando si vedono.
    pub thumbnails: Vec<Thumbnail>,
    /// La finestra trascinata e la sua cornice in questo momento.
    pub drag: Option<(Window, Rectangle<f64, Logical>)>,
}

/// Una voce della barra degli spazi, in punti nell'output.
struct BarItem {
    index: usize,
    label: String,
    rect: Rectangle<f64, Logical>,
}

fn space_label(index: usize) -> String {
    format!("Spazio {}", index.saturating_add(1))
}

fn bar_items(count: usize, width: f64) -> Vec<BarItem> {
    let mut items: Vec<BarItem> = (0..count)
        .map(|index| {
            let label = space_label(index);
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
        + LABEL_GAP * count.saturating_sub(1) as f64;
    let mut x = (width - total) / 2.0;
    for item in &mut items {
        item.rect.loc.x = x;
        x += item.rect.size.w + LABEL_GAP;
    }
    items
}

/// Le miniature degli spazi nella barra espansa: `count` miniature più un posto vuoto,
/// centrate; se non entrano si stringono tutte insieme.
pub fn thumbnail_rects(count: usize, output: Size<f64, Logical>) -> Vec<Rectangle<f64, Logical>> {
    if output.w <= 0.0 || output.h <= 0.0 {
        return Vec::new();
    }
    let slots = count.saturating_add(1) as f64;
    let natural = slots * THUMB_WIDTH + (slots - 1.0) * THUMB_GAP;
    // Margine per il "+" a destra e simmetrico a sinistra.
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

/// Altezza della barra per un'espansione 0–1.
pub fn bar_height(expansion: f64) -> f64 {
    BAR_HEIGHT + (EXPANDED_HEIGHT - BAR_HEIGHT) * expansion.clamp(0.0, 1.0)
}

/// La cornice in questo momento dell'animazione.
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

/// La cornice della finestra trascinata: segue il puntatore dal punto di presa; sopra la
/// barra si riduce.
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

    /// Apre Panorama, o lo chiude se è aperto.
    pub fn toggle_panorama(&mut self, now: Instant) {
        if self.panorama_open() {
            self.close_panorama(now, None);
        } else {
            self.open_panorama(now);
        }
    }

    /// Le finestre dello spazio con la cornice vera e quella disposta.
    fn panorama_placements(&self) -> Vec<Placement> {
        let windows: Vec<Window> = self.space.elements().cloned().collect();
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
        let top = output.loc.y + BAR_HEIGHT + MARGIN_TOP;
        let bottom = usable.loc.y + usable.size.h - MARGIN_BOTTOM;
        let area = Rectangle::new(
            (output.loc.x + MARGIN_SIDE, top).into(),
            (output.size.w - 2.0 * MARGIN_SIDE, (bottom - top).max(1.0)).into(),
        );
        let targets = layout(&frames, area);
        windows
            .into_iter()
            .zip(frames)
            .zip(targets)
            .map(|((window, from), to)| Placement { window, from, to })
            .collect()
    }

    fn open_panorama(&mut self, now: Instant) {
        self.settle_transition();
        let from = self.panorama.as_ref().map_or(0.0, |p| p.progress);
        let placements = self.panorama_placements();
        self.titlebar_hover = None;
        self.panorama = Some(Panorama {
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
        });
        if self.animation_speed <= 0.0 {
            self.advance_panorama(now);
        }
    }

    /// Chiude Panorama; con `focus` porta quella finestra in primo piano.
    pub fn close_panorama(&mut self, now: Instant, focus: Option<Window>) {
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        if panorama.opening {
            panorama.opening = false;
            panorama.from = panorama.progress;
            panorama.start = now;
            panorama.drag = None;
        }
        if let Some(window) = focus {
            self.focus_window(Some(window));
        }
        if self.animation_speed <= 0.0 {
            self.advance_panorama(now);
        }
    }

    /// Espande o richiude la barra degli spazi.
    pub fn expand_spaces_bar(&mut self, expanded: bool, now: Instant) {
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        if panorama.expanded == expanded || !panorama.opening {
            return;
        }
        panorama.expanded = expanded;
        panorama.expand_from = panorama.expansion;
        panorama.expand_start = now;
        if self.animation_speed <= 0.0 {
            self.advance_panorama(now);
        }
    }

    /// Fa avanzare le animazioni (a ogni frame); chiuso del tutto, Panorama sparisce.
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
        panorama.progress = if panorama.opening {
            panorama.from + (1.0 - panorama.from) * curve_at(OPEN_CURVE, ms)
        } else {
            panorama.from * (1.0 - curve_at(CLOSE_CURVE, ms))
        };
        let step = curve_at(EXPAND_CURVE, elapsed(panorama.expand_start));
        panorama.expansion = if panorama.expanded {
            panorama.expand_from + (1.0 - panorama.expand_from) * step
        } else {
            panorama.expand_from * (1.0 - step)
        };
        if !panorama.opening && panorama.progress <= 0.0 {
            self.panorama = None;
        }
    }

    /// Le finestre di ogni spazio alle loro posizioni (quelle dello spazio attivo dallo Space).
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
        let drag = panorama.drag.as_ref().filter(|d| d.moved).and_then(|drag| {
            let placement = panorama
                .placements
                .iter()
                .find(|p| p.window == drag.window)?;
            Some((
                drag.window.clone(),
                drag_rect(drag, placement.to, bar_height(panorama.expansion)),
            ))
        });
        Some(PanoramaView {
            progress: panorama.progress,
            placements: panorama.placements.clone(),
            expansion: panorama.expansion,
            thumbnails,
            drag,
        })
    }

    /// Il puntatore si muove con Panorama aperto: in alto la barra si espande, sotto la barra
    /// espansa (fuori dalla barra) si richiude; una finestra presa segue il puntatore.
    pub fn panorama_pointer(&mut self, location: Point<f64, Logical>, now: Instant) {
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        if !panorama.opening {
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

    /// Lo spazio della miniatura sotto il punto, con la barra espansa.
    fn thumbnail_at(&self, location: Point<f64, Logical>) -> Option<usize> {
        let panorama = self.panorama.as_ref()?;
        if panorama.expansion < THUMBS_FROM {
            return None;
        }
        thumbnail_rects(self.workspaces.count(), self.output_size())
            .iter()
            .position(|rect| rect.contains(location))
    }

    /// Pressione con Panorama aperto: una voce della barra, il "+", una miniatura, una
    /// finestra (che si può trascinare) o il vuoto.
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
        if location.y < bar_height(expansion) {
            if plus_rect(width, expansion).contains(location) {
                // La barra si ridisegna da sola: la sua chiave è il numero di spazi.
                if let Err(err) = self.add_space() {
                    tracing::info!("panorama: {err}");
                }
                return;
            }
            let index = if expansion < THUMBS_FROM {
                bar_items(self.workspaces.count(), width)
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

    /// Rilascio: una finestra non mossa va in primo piano e Panorama si chiude; una
    /// trascinata su una miniatura passa a quello spazio, altrove torna al suo posto.
    pub fn panorama_release(&mut self, location: Point<f64, Logical>, now: Instant) {
        let Some(drag) = self.panorama.as_mut().and_then(|p| p.drag.take()) else {
            return;
        };
        if !drag.moved {
            self.close_panorama(now, Some(drag.window));
            return;
        }
        let Some(index) = self.thumbnail_at(location) else {
            return;
        };
        if index == self.workspaces.active {
            return;
        }
        if let Err(err) = self.move_window_to_space(&drag.window, index) {
            tracing::info!("panorama: {err}");
            return;
        }
        // Le finestre rimaste si ridispongono subito.
        let placements = self.panorama_placements();
        if let Some(panorama) = self.panorama.as_mut() {
            panorama.placements = placements;
        }
    }

    /// Per `panorama.state`.
    pub fn panorama_state(&self) -> PanoramaState {
        let Some(panorama) = self.panorama.as_ref() else {
            return PanoramaState {
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

fn faded(color: [u8; 4], alpha: f64) -> [u8; 4] {
    let [r, g, b, a] = color;
    [
        r,
        g,
        b,
        (f64::from(a) * alpha.clamp(0.0, 1.0)).round() as u8,
    ]
}

/// Quello che serve per disegnare la barra.
pub struct BarLook {
    pub width: f64,
    pub output: Size<f64, Logical>,
    pub count: usize,
    pub active: usize,
    pub expansion: f64,
    /// Colore delle miniature dietro lo sfondo (quello del desktop).
    pub backdrop: [u8; 4],
}

impl BarLook {
    /// Altezza dello sprite: barra, bordo e ombra.
    fn sprite_height(&self) -> f64 {
        let shadow = BAR_SHADOW + (EXPANDED_SHADOW - BAR_SHADOW) * self.expansion;
        bar_height(self.expansion) + 1.0 + shadow
    }

    /// Chiave della cache: cambia con quello che si vede (altezza al punto intero).
    fn key(&self) -> String {
        format!(
            "{}:{}:{}:{:?}",
            self.count,
            self.active,
            (self.expansion * 1000.0).round(),
            self.backdrop
        )
    }
}

/// Disegna la barra degli spazi (compatta, espansa o a metà) con l'ombra sotto.
pub fn draw_bar(canvas: &mut Canvas, look: &BarLook) {
    let e = look.expansion.clamp(0.0, 1.0);
    let w = look.width as f32;
    let height = bar_height(e);
    // Riempimento: uniforme da compatta, con la sfumatura in basso da espansa.
    let rows = height.ceil() as usize;
    for row in 0..rows {
        let y = row as f64;
        let toward_bottom = (y / EXPANDED_FILL_FROM).min(1.0) * e;
        let color = mix(BAR_FILL, EXPANDED_FILL_BOTTOM, toward_bottom);
        let h = (height - y).min(1.0) as f32;
        canvas.fill_rect([0.0, row as f32, w, h], color);
    }
    let bar = height as f32;
    canvas.fill_rect([0.0, bar, w, 1.0], mix(BAR_BORDER, EXPANDED_BORDER, e));
    // Ombra: nero che sfuma a zero.
    let shadow = BAR_SHADOW + (EXPANDED_SHADOW - BAR_SHADOW) * e;
    let rows = shadow as usize;
    for row in 0..rows {
        let t = 1.0 - row as f32 / rows as f32;
        let alpha = (BAR_SHADOW_ALPHA * t * t * 255.0).round() as u8;
        canvas.fill_rect([0.0, bar + 1.0 + row as f32, w, 1.0], [0, 0, 0, alpha]);
    }
    // Le voci compatte svaniscono all'inizio dell'espansione.
    let compact = 1.0 - e / 0.3;
    if compact > 0.0 {
        for item in bar_items(look.count, look.width) {
            let r = item.rect;
            if item.index == look.active {
                let rect = [
                    r.loc.x as f32,
                    r.loc.y as f32,
                    r.size.w as f32,
                    r.size.h as f32,
                ];
                canvas.fill_rounded(rect, CAPSULE_RADIUS, faded(CAPSULE_FILL, compact));
                canvas.stroke_rounded(rect, CAPSULE_RADIUS, 1.0, faded(CAPSULE_BORDER, compact));
            }
            // Linea di base: maiuscole di ~10 pt centrate nella capsula.
            let baseline = (CAPSULE_TOP + CAPSULE_HEIGHT / 2.0 + 5.0) as f32;
            canvas.text(
                [(r.loc.x + LABEL_PADDING) as f32, baseline],
                &item.label,
                LABEL_SIZE,
                Weight::Regular,
                faded(LABEL_COLOR, compact),
            );
        }
    }
    // Le miniature: il fondo (lo sfondo del desktop ci si disegna sopra), l'anello e
    // l'etichetta.
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
            let label = space_label(index);
            let text = f64::from(text_width(&label, THUMB_LABEL_SIZE, Weight::Regular));
            canvas.text(
                [
                    (r.loc.x + (r.size.w - text) / 2.0) as f32,
                    (r.loc.y + r.size.h + THUMB_LABEL_BASELINE) as f32,
                ],
                &label,
                THUMB_LABEL_SIZE,
                Weight::Regular,
                faded(LABEL_COLOR, shown),
            );
        }
    }
    let plus = plus_rect(look.width, e);
    let cx = (plus.loc.x + plus.size.w / 2.0) as f32;
    let cy = (plus.loc.y + plus.size.h / 2.0) as f32;
    let half = (PLUS_SIZE / 2.0) as f32 - 1.0;
    canvas.line([cx - half, cy], [cx + half, cy], 2.0, PLUS_COLOR);
    canvas.line([cx, cy - half], [cx, cy + half], 2.0, PLUS_COLOR);
}

/// La barra degli spazi come elemento, già spostata in alto quanto serve all'apertura.
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
    // Il bordo inferiore della barra scende da 0 a 40 pt sulla curva delle finestre;
    // l'ombra, che a riposo sta sotto, compare nel primo fotogramma.
    let y = -(1.0 - progress) * (bar_height(look.expansion) + 1.0);
    sprite.element(
        renderer,
        &look.key(),
        Size::from((look.width, look.sprite_height())),
        Point::from((0.0, y)),
        scale,
        (progress * 4.0).clamp(0.0, 1.0) as f32,
        |canvas| draw_bar(canvas, look),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle<f64, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn two_windows_that_fit_keep_their_size() {
        // Il riferimento: Finder 900 × 600 e Terminale 597 × 422 vanno a (533,332) e
        // (1449,276), scala 1, 16 pt fra loro (misure a ±1 pt).
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
                "{got:?} invece di {want:?}"
            );
        }
        // Già separate: restano dove sono.
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
            assert!(area.contains_rect(*a), "{a:?} fuori da {area:?}");
            for b in out.iter().skip(i + 1) {
                assert!(!a.overlaps(*b), "{a:?} {b:?}");
            }
        }
        assert!(layout(&[], area).is_empty());
    }

    #[test]
    fn curves_follow_the_measurements() {
        assert_eq!(curve_at(OPEN_CURVE, 0.0), 0.0);
        // Metà strada fra 40 e 54 ms.
        assert!(curve_at(OPEN_CURVE, 40.0) < 0.5 && curve_at(OPEN_CURVE, 54.0) > 0.5);
        assert!((curve_at(CLOSE_CURVE, 67.0) - 0.497).abs() < 1e-9);
        assert_eq!(curve_at(CLOSE_CURVE, 1000.0), 1.0);
    }
}
