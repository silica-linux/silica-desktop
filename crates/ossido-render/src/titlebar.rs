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
pub const UNIFORMS: [(&str, UniformType); 8] = [
    ("frame_height", UniformType::_1f),
    ("radius", UniformType::_1f),
    ("exponent", UniformType::_1f),
    ("pixel", UniformType::_1f),
    ("background", UniformType::_3f),
    ("separator", UniformType::_4f),
    ("active", UniformType::_1f),
    ("hovered", UniformType::_1f),
];

/// Stato della barra che cambia il disegno.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TitlebarState {
    pub active: bool,
    pub hovered: bool,
    pub scale: f64,
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
        let area = Rectangle::new(frame.loc, Size::from((frame.size.w, HEIGHT)));
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
        Uniform::new("hovered", UniformValue::_1f(flag(state.hovered))),
    ]
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
