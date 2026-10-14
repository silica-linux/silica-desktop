//! Barra del titolo delle decorazioni lato server: 52 pt come la barra degli strumenti
//! misurata, con i tre pulsanti della finestra (`plan/reference/windows.md`, "Barra degli
//! strumenti" e "Pulsanti della finestra"). Aspetto chiaro.

use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesPixelProgram, Uniform, UniformType, UniformValue};
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::cache::ShaderCache;
use crate::corners::{CORNER_EXPONENT, CORNER_RADIUS};

/// Altezza della barra, in punti.
pub const HEIGHT: i32 = 52;
/// Centri dei pulsanti (chiudi, riduci, schermo intero) nella barra: x 26, 49, 72, y 26.
pub const BUTTON_CENTERS: [(f64, f64); 3] = [(26.0, 26.0), (49.0, 26.0), (72.0, 26.0)];
/// Raggio della zona attiva di ogni pulsante: la cella di accessibilità è 16 × 16 pt.
pub const BUTTON_HIT_RADIUS: f64 = 8.0;

/// Fondo della barra: come il contenuto della finestra attiva in chiaro (`#ffffff`).
const BACKGROUND: [f32; 3] = [1.0, 1.0, 1.0];
/// Finestra inattiva: riga di 1 pt `#e5e5e5` fra barra e contenuto.
const SEPARATOR_INACTIVE: [f32; 4] = [229.0 / 255.0, 229.0 / 255.0, 229.0 / 255.0, 1.0];

pub const SHADER: &str = include_str!("shaders/titlebar.frag");
pub const UNIFORMS: [(&str, UniformType); 13] = [
    ("frame_height", UniformType::_1f),
    ("radius", UniformType::_1f),
    ("exponent", UniformType::_1f),
    ("pixel", UniformType::_1f),
    ("background", UniformType::_3f),
    ("separator", UniformType::_4f),
    ("active", UniformType::_1f),
    ("symbols", UniformType::_1f),
    ("pressed", UniformType::_1f),
    ("alt", UniformType::_1f),
    ("disabled", UniformType::_1f),
    ("bar", UniformType::_1f),
    ("button_y", UniformType::_1f),
];

/// Stato della barra che cambia il disegno.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TitlebarState {
    pub active: bool,
    /// Opacità dei simboli (0–1): il puntatore sul gruppo li mostra, anche in una finestra
    /// inattiva, che intanto riprende i colori attivi.
    pub symbols: f32,
    /// Il pulsante premuto (0 chiusura, 1 riduzione, 2 schermo intero) con il puntatore ancora
    /// sopra.
    pub pressed: Option<u8>,
    /// ⌥ tenuto: il verde mostra "+" (ingrandisci) invece dello schermo intero.
    pub alt: bool,
    /// Riduzione e schermo intero non disponibili (finestra di dimensione fissa o dialogo).
    pub disabled: bool,
    pub scale: f64,
    /// Il fondo della barra: falso con il contenuto a tutta finestra (`silica-window-v1`), dove
    /// restano solo i pulsanti sopra l'app.
    pub bar: bool,
    /// Il centro verticale dei pulsanti (26 nella barra di Ossido; metà della barra degli
    /// strumenti dell'app con il contenuto a tutta finestra).
    pub button_y: f32,
}

/// Simboli dei pulsanti nel tempo (windows.md, "Altri stati"): all'ingresso del puntatore
/// sono già all'80 % e pieni in ~90 ms; all'uscita restano ~60 ms e spariscono in ~55 ms.
pub fn symbols_opacity(since_enter_ms: f64, since_leave_ms: Option<f64>) -> f32 {
    let shown = (0.8 + 0.2 * (since_enter_ms / 90.0)).clamp(0.0, 1.0);
    let fading = match since_leave_ms {
        None => 1.0,
        Some(ms) => (1.0 - (ms - 60.0) / 55.0).clamp(0.0, 1.0),
    };
    (shown * fading) as f32
}

/// La barra di una finestra, tenuta nei dati della finestra.
#[derive(Default)]
pub struct Titlebar {
    cache: ShaderCache<(Size<i32, Logical>, TitlebarState)>,
}

impl Titlebar {
    /// L'elemento della barra per una cornice (barra + contenuto, logica, nell'output).
    pub fn element(
        &self,
        program: &GlesPixelProgram,
        frame: Rectangle<i32, Logical>,
        state: TitlebarState,
    ) -> PixelShaderElement {
        // Con il contenuto a tutta finestra la fascia arriva sotto i pulsanti, centrati più in
        // basso se la barra dell'app è più alta.
        let height = HEIGHT.max((state.button_y * 2.0).ceil() as i32);
        let area = Rectangle::new(frame.loc, Size::from((frame.size.w, height)));
        self.cache.element(program, area, (frame.size, state), || {
            uniforms(frame.size, state)
        })
    }
}

fn uniforms(frame: Size<i32, Logical>, state: TitlebarState) -> Vec<Uniform<'static>> {
    let radius = (CORNER_RADIUS as f32)
        .min(frame.w as f32 / 2.0)
        .min(frame.h as f32 / 2.0)
        .max(0.0);
    let separator = if state.active {
        [0.0; 4]
    } else {
        SEPARATOR_INACTIVE
    };
    let flag = |on: bool| if on { 1.0 } else { 0.0 };
    vec![
        Uniform::new("frame_height", UniformValue::_1f(frame.h as f32)),
        Uniform::new("radius", UniformValue::_1f(radius)),
        Uniform::new("exponent", UniformValue::_1f(CORNER_EXPONENT)),
        Uniform::new(
            "pixel",
            UniformValue::_1f((1.0 / state.scale.max(1.0)) as f32),
        ),
        Uniform::new(
            "background",
            UniformValue::_3f(BACKGROUND[0], BACKGROUND[1], BACKGROUND[2]),
        ),
        Uniform::new(
            "separator",
            UniformValue::_4f(separator[0], separator[1], separator[2], separator[3]),
        ),
        Uniform::new("active", UniformValue::_1f(flag(state.active))),
        Uniform::new("symbols", UniformValue::_1f(state.symbols.clamp(0.0, 1.0))),
        Uniform::new(
            "pressed",
            UniformValue::_1f(state.pressed.map_or(-1.0, f32::from)),
        ),
        Uniform::new("alt", UniformValue::_1f(flag(state.alt))),
        Uniform::new("disabled", UniformValue::_1f(flag(state.disabled))),
        Uniform::new("bar", UniformValue::_1f(flag(state.bar))),
        Uniform::new("button_y", UniformValue::_1f(state.button_y)),
    ]
}

impl Part {
    /// Indice del pulsante (0 chiusura, 1 riduzione, 2 schermo intero).
    pub fn button(self) -> Option<u8> {
        match self {
            Part::Close => Some(0),
            Part::Minimize => Some(1),
            Part::Zoom => Some(2),
            Part::Bar => None,
        }
    }
}

/// Una parte della barra sotto il puntatore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Close,
    Minimize,
    Zoom,
    /// Il resto della barra: si trascina per spostare la finestra.
    Bar,
}

/// La parte della barra in un punto relativo all'angolo in alto a sinistra della cornice.
pub fn part_at(point: Point<f64, Logical>) -> Option<Part> {
    if point.y < 0.0 || point.y >= f64::from(HEIGHT) || point.x < 0.0 {
        return None;
    }
    let parts = [Part::Close, Part::Minimize, Part::Zoom];
    for ((x, y), part) in BUTTON_CENTERS.iter().zip(parts) {
        let (dx, dy) = (point.x - x, point.y - y);
        if dx.abs() <= BUTTON_HIT_RADIUS && dy.abs() <= BUTTON_HIT_RADIUS {
            return Some(part);
        }
    }
    Some(Part::Bar)
}

/// Il puntatore è sul gruppo dei pulsanti: i simboli compaiono su tutti e tre.
pub fn over_buttons(point: Point<f64, Logical>) -> bool {
    let (first, last) = (BUTTON_CENTERS[0], BUTTON_CENTERS[2]);
    point.x >= first.0 - BUTTON_HIT_RADIUS
        && point.x <= last.0 + BUTTON_HIT_RADIUS
        && (point.y - first.1).abs() <= BUTTON_HIT_RADIUS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_follow_the_measured_buttons() {
        assert_eq!(part_at(Point::from((26.0, 26.0))), Some(Part::Close));
        assert_eq!(part_at(Point::from((49.0, 20.0))), Some(Part::Minimize));
        assert_eq!(part_at(Point::from((72.0, 33.0))), Some(Part::Zoom));
        assert_eq!(part_at(Point::from((200.0, 26.0))), Some(Part::Bar));
        assert_eq!(part_at(Point::from((26.0, 52.0))), None);
        assert_eq!(part_at(Point::from((26.0, -1.0))), None);
        assert!(over_buttons(Point::from((60.0, 26.0))));
        assert!(!over_buttons(Point::from((100.0, 26.0))));
    }
}

#[cfg(test)]
mod symbol_tests {
    use super::*;

    #[test]
    fn symbols_appear_at_once_and_leave_after_a_pause() {
        // Al primo fotogramma ~80 %, pieni entro ~90 ms.
        assert!((symbols_opacity(0.0, None) - 0.8).abs() < 1e-6);
        assert_eq!(symbols_opacity(100.0, None), 1.0);
        // Uscita: fermi per 60 ms, poi spariti in ~55 ms.
        assert_eq!(symbols_opacity(500.0, Some(50.0)), 1.0);
        assert!(symbols_opacity(500.0, Some(90.0)) < 0.5);
        assert_eq!(symbols_opacity(500.0, Some(120.0)), 0.0);
        assert_eq!(Part::Zoom.button(), Some(2));
        assert_eq!(Part::Bar.button(), None);
    }
}
