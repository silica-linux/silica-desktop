//! La specifica del Dock (`docs/m1.5-dock.md`) in numeri: le stesse funzioni di
//! `apps/dock/lib/main.dart`, così le due versioni disegnano gli stessi pixel.

use std::f64::consts::PI;

pub const ICON_COUNT: usize = 12;
pub const ICON_SIZE: f64 = 48.0;
pub const SPACING: f64 = 8.0;
pub const STEP: f64 = ICON_SIZE + SPACING;
pub const PADDING: f64 = 8.0;
pub const BOTTOM_MARGIN: f64 = 8.0;
pub const INFLUENCE: f64 = 3.0;
pub const BACKGROUND_RADIUS: f64 = 18.0;
/// Larghezza della fila a riposo: 664.
pub const REST_ROW_WIDTH: f64 = ICON_COUNT as f64 * ICON_SIZE + (ICON_COUNT - 1) as f64 * SPACING;

/// Quanto cresce un'icona a distanza `d` (in passi) dal puntatore: 0..1.
pub fn bump(d: f64) -> f64 {
    if d < INFLUENCE {
        (1.0 + (PI * d / INFLUENCE).cos()) / 2.0
    } else {
        0.0
    }
}

/// Il centro a riposo dell'icona `i`, in una superficie larga `width`.
pub fn rest_center(width: f64, i: usize) -> f64 {
    (width - REST_ROW_WIDTH) / 2.0 + i as f64 * STEP + ICON_SIZE / 2.0
}

/// I lati delle icone con il puntatore a `pointer_x` e l'ingrandimento `m`.
pub fn icon_sizes(width: f64, pointer_x: Option<f64>, m: f64) -> [f64; ICON_COUNT] {
    let mut sizes = [ICON_SIZE; ICON_COUNT];
    if let Some(x) = pointer_x {
        for (i, side) in sizes.iter_mut().enumerate() {
            let d = (x - rest_center(width, i)).abs() / STEP;
            *side = ICON_SIZE * (1.0 + m * bump(d));
        }
    }
    sizes
}

/// L'icona più vicina al puntatore.
pub fn nearest_icon(width: f64, pointer_x: f64) -> usize {
    let index = ((pointer_x - rest_center(width, 0)) / STEP).round();
    if index <= 0.0 {
        0
    } else {
        (index as usize).min(ICON_COUNT - 1)
    }
}

/// Un colore sRGB a 8 bit per canale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// HSL → RGB come `HSLColor.toColor` di Flutter (stesso arrotondamento).
pub fn hsl(hue: f64, saturation: f64, lightness: f64) -> Rgb {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let secondary = chroma * (1.0 - (((hue / 60.0) % 2.0) - 1.0).abs());
    let matched = lightness - chroma / 2.0;
    let (r, g, b) = match hue {
        h if h < 60.0 => (chroma, secondary, 0.0),
        h if h < 120.0 => (secondary, chroma, 0.0),
        h if h < 180.0 => (0.0, chroma, secondary),
        h if h < 240.0 => (0.0, secondary, chroma),
        h if h < 300.0 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    let channel = |c: f64| ((c + matched) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(channel(r), channel(g), channel(b))
}

/// Il gradiente dell'icona `i`: tinta i·30°, dalla luminosità 60% alla 40%.
pub fn icon_colors(i: usize) -> (Rgb, Rgb) {
    let hue = i as f64 * 30.0;
    (hsl(hue, 0.7, 0.6), hsl(hue, 0.7, 0.4))
}

/// La molla di `SpringSimulation` di Flutter con massa 1, rigidità 400, smorzamento 30
/// (sottosmorzata), in forma chiusa. Tolleranze di `Tolerance.defaultTolerance`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    end: f64,
    /// Coefficienti della soluzione `e^(r·t)·(c1·cos(w·t) + c2·sin(w·t))`.
    c1: f64,
    c2: f64,
}

const MASS: f64 = 1.0;
const STIFFNESS: f64 = 400.0;
const DAMPING: f64 = 30.0;
const TOLERANCE: f64 = 1e-3;

fn r() -> f64 {
    -DAMPING / (2.0 * MASS)
}

fn w() -> f64 {
    (4.0 * MASS * STIFFNESS - DAMPING * DAMPING).sqrt() / (2.0 * MASS)
}

impl Spring {
    /// Da `start` con velocità `velocity` (unità al secondo) verso `end`.
    pub fn new(start: f64, end: f64, velocity: f64) -> Self {
        let c1 = start - end;
        let c2 = (velocity - r() * c1) / w();
        Self { end, c1, c2 }
    }

    /// Posizione dopo `t` secondi.
    pub fn x(&self, t: f64) -> f64 {
        let (r, w) = (r(), w());
        self.end + (r * t).exp() * (self.c1 * (w * t).cos() + self.c2 * (w * t).sin())
    }

    /// Velocità dopo `t` secondi.
    pub fn dx(&self, t: f64) -> f64 {
        let (r, w) = (r(), w());
        let power = (r * t).exp();
        let (cos, sin) = ((w * t).cos(), (w * t).sin());
        power * (self.c2 * w * cos - self.c1 * w * sin)
            + r * power * (self.c2 * sin + self.c1 * cos)
    }

    pub fn is_done(&self, t: f64) -> bool {
        (self.x(t) - self.end).abs() < TOLERANCE && self.dx(t).abs() < TOLERANCE
    }

    pub fn end(&self) -> f64 {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_and_layout_match_the_flutter_version() {
        assert_eq!(bump(0.0), 1.0);
        assert!((bump(1.5) - 0.5).abs() < 1e-12);
        assert_eq!(bump(3.0), 0.0);
        assert_eq!(REST_ROW_WIDTH, 664.0);
        assert_eq!(rest_center(1280.0, 0), (1280.0 - 664.0) / 2.0 + 24.0);
        let sizes = icon_sizes(1280.0, Some(rest_center(1280.0, 5)), 1.0);
        assert_eq!(sizes[5], 96.0);
        assert!((sizes[4] - 48.0 * 1.75).abs() < 1e-9);
        assert_eq!(sizes[0], 48.0);
        assert_eq!(icon_sizes(1280.0, None, 1.0), [48.0; ICON_COUNT]);
        assert_eq!(nearest_icon(1280.0, rest_center(1280.0, 5) + 20.0), 5);
        assert_eq!(nearest_icon(1280.0, -100.0), 0);
        assert_eq!(nearest_icon(1280.0, 5000.0), 11);
    }

    #[test]
    fn hsl_matches_flutter() {
        // HSLColor.fromAHSL(1, 0, 0.7, 0.6).toColor() == Color(0xFFE05252).
        assert_eq!(hsl(0.0, 0.7, 0.6), Rgb(0xE0, 0x52, 0x52));
        // HSLColor.fromAHSL(1, 210, 0.7, 0.4).toColor() == Color(0xFF1F66AD).
        assert_eq!(hsl(210.0, 0.7, 0.4), Rgb(0x1F, 0x66, 0xAD));
    }

    #[test]
    fn spring_starts_where_asked_overshoots_and_settles() {
        let spring = Spring::new(0.0, 1.0, 0.0);
        assert!(spring.x(0.0).abs() < 1e-12);
        assert!(spring.dx(0.0).abs() < 1e-12);
        // Sottosmorzata: supera 1 prima di fermarsi.
        let peak = (0..100)
            .map(|i| spring.x(f64::from(i) / 100.0))
            .fold(0.0, f64::max);
        assert!(peak > 1.0 && peak < 1.1, "{peak}");
        assert!(spring.is_done(1.0));
        assert!(!spring.is_done(0.1));
        // La velocità è la derivata della posizione.
        let t = 0.1;
        let numeric = (spring.x(t + 1e-6) - spring.x(t - 1e-6)) / 2e-6;
        assert!((numeric - spring.dx(t)).abs() < 1e-4);
    }
}
