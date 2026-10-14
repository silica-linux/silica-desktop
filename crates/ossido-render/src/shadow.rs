//! Window shadow, measured on macOS 27 (`plan/reference/windows.md`,
//! "Windows · shadow"): black, Gaussian-blur profile, shifted down, wider for the active
//! window, with a 1 pt contact ring; in dark about twice as strong.

use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesPixelProgram, Uniform, UniformType, UniformValue};
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::cache::ShaderCache;
use crate::corners::{CORNER_EXPONENT, CORNER_RADIUS};

/// Parameters of a shadow, in logical points; alpha from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowStyle {
    /// Amplitude A/255 of the fit alpha(d) = A·½·erfc((d − s)/(√2 σ)).
    pub amplitude: f32,
    pub shift: f32,
    pub sigma: f32,
    /// Vertical shift of the shadow (downwards).
    pub offset_y: f32,
    /// Alpha of the 1 pt contact ring: sides and top.
    pub ring_side: f32,
    pub ring_top: f32,
    /// Extent of the shadow beyond the window: sides, top, bottom (where it reaches zero).
    pub extent: (i32, i32, i32),
}

/// Active window, light appearance: A 100, s 0.5, σ 19.5, shift 18, ring 81 and 54,
/// margins 56 / 38 / 74.
pub const ACTIVE_LIGHT: ShadowStyle = ShadowStyle {
    amplitude: 100.0 / 255.0,
    shift: 0.5,
    sigma: 19.5,
    offset_y: 18.0,
    ring_side: 81.0 / 255.0,
    ring_top: 54.0 / 255.0,
    extent: (56, 38, 74),
};

/// Inactive window, light appearance: A 72, s −2, σ 8.5, shift 7, ring 59 and 44,
/// margins 23 / 16 / 30.
pub const INACTIVE_LIGHT: ShadowStyle = ShadowStyle {
    amplitude: 72.0 / 255.0,
    shift: -2.0,
    sigma: 8.5,
    offset_y: 7.0,
    ring_side: 59.0 / 255.0,
    ring_top: 44.0 / 255.0,
    extent: (23, 16, 30),
};

/// Active window, dark appearance: A 190, s −1, σ 20, same shift (18), ring 243 on the
/// sides and 239 on top (practically a 1 pt black border). Margins as in light: the profile
/// reaches 1 at d = 48 (windows.md, "Windows · shadow").
pub const ACTIVE_DARK: ShadowStyle = ShadowStyle {
    amplitude: 190.0 / 255.0,
    shift: -1.0,
    sigma: 20.0,
    offset_y: 18.0,
    ring_side: 243.0 / 255.0,
    ring_top: 239.0 / 255.0,
    extent: (56, 38, 74),
};

/// Inactive window, dark appearance: A 194, s −4, σ 9, shift 7, ring 223 and 215,
/// margins as in light (the profile is at 0 at d = 20).
pub const INACTIVE_DARK: ShadowStyle = ShadowStyle {
    amplitude: 194.0 / 255.0,
    shift: -4.0,
    sigma: 9.0,
    offset_y: 7.0,
    ring_side: 223.0 / 255.0,
    ring_top: 215.0 / 255.0,
    extent: (23, 16, 30),
};

/// The shadow of a window according to state and appearance.
pub fn style(active: bool, dark: bool) -> ShadowStyle {
    match (active, dark) {
        (true, false) => ACTIVE_LIGHT,
        (false, false) => INACTIVE_LIGHT,
        (true, true) => ACTIVE_DARK,
        (false, true) => INACTIVE_DARK,
    }
}

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

/// The shadow of a window, kept in the window's data.
#[derive(Default)]
pub struct WindowShadow {
    cache: ShaderCache<(Size<i32, Logical>, ShadowStyle)>,
}

impl WindowShadow {
    /// The shadow element for a window with this frame (logical, in the output).
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
        self.cache.element(program, area, (window.size, style), || {
            uniforms(window.size, (side, top), style)
        })
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
