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
use silica_control::commands::{PanoramaState, PanoramaWindow};
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

pub struct Panorama {
    pub opening: bool,
    pub start: Instant,
    /// Avanzamento da cui riparte la fase (un Esc a metà apertura non salta).
    pub from: f64,
    pub progress: f64,
    pub placements: Vec<Placement>,
}

/// Quello che il frame deve sapere di Panorama.
#[derive(Clone)]
pub struct PanoramaView {
    pub progress: f64,
    pub placements: Vec<Placement>,
}

/// Una voce della barra degli spazi, in punti nell'output.
struct BarItem {
    index: usize,
    label: String,
    rect: Rectangle<f64, Logical>,
}

fn bar_items(count: usize, width: f64) -> Vec<BarItem> {
    let mut items: Vec<BarItem> = (0..count)
        .map(|index| {
            let label = format!("Spazio {}", index + 1);
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

fn plus_rect(width: f64) -> Rectangle<f64, Logical> {
    Rectangle::new(
        (width - PLUS_RIGHT - 16.0, 20.0 - 16.0).into(),
        (32.0, 32.0).into(),
    )
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

impl State {
    pub fn panorama_open(&self) -> bool {
        self.panorama.as_ref().is_some_and(|p| p.opening)
    }

    /// Apre Panorama, o lo chiude se è aperto.
    pub fn toggle_panorama(&mut self, now: Instant) {
        if self.panorama_open() {
            self.close_panorama(now, None);
        } else {
            self.open_panorama(now);
        }
    }

    fn open_panorama(&mut self, now: Instant) {
        self.settle_transition();
        let from = self.panorama.as_ref().map_or(0.0, |p| p.progress);
        let windows: Vec<Window> = self.space.elements().cloned().collect();
        let frames: Vec<Rectangle<f64, Logical>> = windows
            .iter()
            .filter_map(|w| {
                let geometry = self.space.element_geometry(w)?;
                Some(crate::decorations::frame_of(w, geometry).to_f64())
            })
            .collect();
        if frames.len() != windows.len() {
            return;
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
        let placements = windows
            .into_iter()
            .zip(frames)
            .zip(targets)
            .map(|((window, from), to)| Placement { window, from, to })
            .collect();
        self.titlebar_hover = None;
        self.panorama = Some(Panorama {
            opening: true,
            start: now,
            from,
            progress: from,
            placements,
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
        }
        if let Some(window) = focus {
            self.focus_window(Some(window));
        }
        if self.animation_speed <= 0.0 {
            self.advance_panorama(now);
        }
    }

    /// Fa avanzare l'animazione (a ogni frame); chiusa del tutto, Panorama sparisce.
    pub fn advance_panorama(&mut self, now: Instant) {
        let speed = self.animation_speed;
        let Some(panorama) = self.panorama.as_mut() else {
            return;
        };
        let ms = now.saturating_duration_since(panorama.start).as_secs_f64() * 1000.0;
        let ms = if speed <= 0.0 { f64::MAX } else { ms / speed };
        panorama.progress = if panorama.opening {
            panorama.from + (1.0 - panorama.from) * curve_at(OPEN_CURVE, ms)
        } else {
            panorama.from * (1.0 - curve_at(CLOSE_CURVE, ms))
        };
        if !panorama.opening && panorama.progress <= 0.0 {
            self.panorama = None;
        }
    }

    pub fn panorama_view(&self) -> Option<PanoramaView> {
        let panorama = self.panorama.as_ref()?;
        Some(PanoramaView {
            progress: panorama.progress,
            placements: panorama.placements.clone(),
        })
    }

    /// Un clic con Panorama aperto: una voce della barra, il "+", una finestra o il vuoto.
    pub fn panorama_click(&mut self, location: Point<f64, Logical>, now: Instant) {
        if !self.panorama_open() {
            return;
        }
        let width = self
            .space
            .output_geometry(&self.output)
            .map_or(0.0, |geo| f64::from(geo.size.w));
        if location.y < BAR_HEIGHT {
            if plus_rect(width).contains(location) {
                // La barra si ridisegna da sola: la sua chiave è il numero di spazi.
                if let Err(err) = self.add_space() {
                    tracing::info!("panorama: {err}");
                }
                return;
            }
            let items = bar_items(self.workspaces.count(), width);
            if let Some(item) = items.iter().find(|item| item.rect.contains(location)) {
                let index = item.index;
                self.close_panorama(now, None);
                self.switch_space(index, now);
            }
            return;
        }
        let progress = self.panorama.as_ref().map_or(1.0, |p| p.progress);
        let hit = self.panorama.as_ref().and_then(|p| {
            p.placements
                .iter()
                .rev()
                .find(|placement| interpolate(placement, progress).contains(location))
                .map(|placement| placement.window.clone())
        });
        self.close_panorama(now, hit);
    }

    /// Per `panorama.state`.
    pub fn panorama_state(&self) -> PanoramaState {
        let Some(panorama) = self.panorama.as_ref() else {
            return PanoramaState {
                open: false,
                progress: 0.0,
                windows: Vec::new(),
            };
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
        }
    }
}

/// Disegna la barra degli spazi compatta con l'ombra sotto, larga `width` punti.
pub fn draw_bar(canvas: &mut Canvas, width: f64, count: usize, active: usize) {
    let w = width as f32;
    let bar = BAR_HEIGHT as f32;
    canvas.fill_rounded([0.0, 0.0, w, bar], 0.0, BAR_FILL);
    canvas.fill_rounded([0.0, bar, w, 1.0], 0.0, BAR_BORDER);
    // Ombra: nero che sfuma a zero in BAR_SHADOW punti.
    let rows = BAR_SHADOW as usize;
    for row in 0..rows {
        let t = 1.0 - row as f32 / rows as f32;
        let alpha = (BAR_SHADOW_ALPHA * t * t * 255.0).round() as u8;
        canvas.fill_rounded([0.0, bar + 1.0 + row as f32, w, 1.0], 0.0, [0, 0, 0, alpha]);
    }
    for item in bar_items(count, width) {
        let r = item.rect;
        if item.index == active {
            let rect = [
                r.loc.x as f32,
                r.loc.y as f32,
                r.size.w as f32,
                r.size.h as f32,
            ];
            canvas.fill_rounded(rect, CAPSULE_RADIUS, CAPSULE_FILL);
            canvas.stroke_rounded(rect, CAPSULE_RADIUS, 1.0, CAPSULE_BORDER);
        }
        // Linea di base: maiuscole di ~10 pt centrate nella capsula.
        let baseline = (CAPSULE_TOP + CAPSULE_HEIGHT / 2.0 + 5.0) as f32;
        canvas.text(
            [(r.loc.x + LABEL_PADDING) as f32, baseline],
            &item.label,
            LABEL_SIZE,
            Weight::Regular,
            LABEL_COLOR,
        );
    }
    let cx = (width - PLUS_RIGHT) as f32;
    let cy = 20.0;
    let half = (PLUS_SIZE / 2.0) as f32 - 1.0;
    canvas.line([cx - half, cy], [cx + half, cy], 2.0, PLUS_COLOR);
    canvas.line([cx, cy - half], [cx, cy + half], 2.0, PLUS_COLOR);
}

/// La barra degli spazi come elemento, già spostata in alto quanto serve all'animazione.
pub fn bar_element(
    renderer: &mut smithay::backend::renderer::gles::GlesRenderer,
    sprite: &Sprite,
    width: f64,
    count: usize,
    active: usize,
    progress: f64,
    scale: f64,
) -> Option<
    smithay::backend::renderer::element::texture::TextureRenderElement<
        smithay::backend::renderer::gles::GlesTexture,
    >,
> {
    // Il bordo inferiore della barra scende da 0 a 40 pt sulla curva delle finestre;
    // l'ombra, che a riposo sta sotto, compare nel primo fotogramma.
    let height = BAR_HEIGHT + 1.0 + BAR_SHADOW;
    let y = -(1.0 - progress) * (BAR_HEIGHT + 1.0);
    sprite.element(
        renderer,
        &format!("{count}:{active}"),
        Size::from((width, height)),
        Point::from((0.0, y)),
        scale,
        (progress * 4.0).clamp(0.0, 1.0) as f32,
        |canvas| draw_bar(canvas, width, count, active),
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
