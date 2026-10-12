//! Ombra delle finestre, misurata su macOS 27 (`plan/reference/windows.md`,
//! "Finestre · ombra"): nera, profilo da sfocatura gaussiana, spostata in basso, più
//! ampia per la finestra attiva, con un anello di 1 pt a contatto.

use std::cell::RefCell;

use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesPixelProgram, Uniform, UniformType, UniformValue};
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::corners::{CORNER_EXPONENT, CORNER_RADIUS};

/// Parametri di un'ombra, in punti logici; alfa da 0 a 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowStyle {
    /// Ampiezza A/255 dell'adattamento alfa(d) = A·½·erfc((d − s)/(√2 σ)).
    pub amplitude: f32,
    pub shift: f32,
    pub sigma: f32,
    /// Spostamento verticale dell'ombra (verso il basso).
    pub offset_y: f32,
    /// Alfa dell'anello di 1 pt a contatto: lati e sopra.
    pub ring_side: f32,
    pub ring_top: f32,
    /// Estensione dell'ombra oltre la finestra: lati, sopra, sotto (dove arriva a zero).
    pub extent: (i32, i32, i32),
}

/// Finestra attiva, aspetto chiaro: A 100, s 0,5, σ 19,5, spostamento 18, anello 81 e 54,
/// margini 56 / 38 / 74.
pub const ACTIVE_LIGHT: ShadowStyle = ShadowStyle {
    amplitude: 100.0 / 255.0,
    shift: 0.5,
    sigma: 19.5,
    offset_y: 18.0,
    ring_side: 81.0 / 255.0,
    ring_top: 54.0 / 255.0,
    extent: (56, 38, 74),
};

/// Finestra inattiva, aspetto chiaro: A 72, s −2, σ 8,5, spostamento 7, anello 59 e 44,
/// margini 23 / 16 / 30.
pub const INACTIVE_LIGHT: ShadowStyle = ShadowStyle {
    amplitude: 72.0 / 255.0,
    shift: -2.0,
    sigma: 8.5,
    offset_y: 7.0,
    ring_side: 59.0 / 255.0,
    ring_top: 44.0 / 255.0,
    extent: (23, 16, 30),
};

pub const SHADER: &str = include_str!("shaders/shadow.frag");
pub const UNIFORMS: [(&str, UniformType); 9] = [
    ("window_rect", UniformType::_4f),
    ("radius", UniformType::_1f),
    ("exponent", UniformType::_1f),
    ("offset_y", UniformType::_1f),
    ("amplitude", UniformType::_1f),
    ("shift", UniformType::_1f),
    ("sigma", UniformType::_1f),
    ("ring_side", UniformType::_1f),
    ("ring_top", UniformType::_1f),
];

/// L'ombra di una finestra, tenuta fra un frame e l'altro (nei dati della finestra) perché
/// il damage tracker la ridisegni solo quando cambia geometria o stile.
#[derive(Default)]
pub struct WindowShadow {
    state: RefCell<Option<Cached>>,
}

/// L'elemento con la geometria e lo stile per cui è stato preparato.
type Cached = (PixelShaderElement, Rectangle<i32, Logical>, ShadowStyle);

impl WindowShadow {
    /// L'elemento dell'ombra per una finestra con questa geometria (logica, nell'output).
    pub fn element(
        &self,
        program: &GlesPixelProgram,
        window: Rectangle<i32, Logical>,
        style: ShadowStyle,
    ) -> PixelShaderElement {
        let (side, top, bottom) = style.extent;
        let area = Rectangle::new(
            window.loc - Point::from((side, top)),
            window.size + Size::from((side.saturating_mul(2), top.saturating_add(bottom))),
        );
        let uniforms = uniforms(window.size, (side, top), style);
        let mut state = self.state.borrow_mut();
        match state.as_mut() {
            Some((element, last_window, last_style))
                if *last_window == window && *last_style == style =>
            {
                element.clone()
            }
            Some((element, last_window, last_style)) => {
                element.resize(area, None);
                element.update_uniforms(uniforms);
                *last_window = window;
                *last_style = style;
                element.clone()
            }
            None => {
                let element = PixelShaderElement::new(
                    program.clone(),
                    area,
                    None,
                    1.0,
                    uniforms,
                    Kind::Unspecified,
                );
                *state = Some((element.clone(), window, style));
                element
            }
        }
    }
}

fn uniforms(
    window: Size<i32, Logical>,
    (side, top): (i32, i32),
    style: ShadowStyle,
) -> Vec<Uniform<'static>> {
    let radius = (CORNER_RADIUS as f32)
        .min(window.w as f32 / 2.0)
        .min(window.h as f32 / 2.0)
        .max(0.0);
    vec![
        Uniform::new(
            "window_rect",
            UniformValue::_4f(side as f32, top as f32, window.w as f32, window.h as f32),
        ),
        Uniform::new("radius", UniformValue::_1f(radius)),
        Uniform::new("exponent", UniformValue::_1f(CORNER_EXPONENT)),
        Uniform::new("offset_y", UniformValue::_1f(style.offset_y)),
        Uniform::new("amplitude", UniformValue::_1f(style.amplitude)),
        Uniform::new("shift", UniformValue::_1f(style.shift)),
        Uniform::new("sigma", UniformValue::_1f(style.sigma)),
        Uniform::new("ring_side", UniformValue::_1f(style.ring_side)),
        Uniform::new("ring_top", UniformValue::_1f(style.ring_top)),
    ]
}
