//! The inner border of windows in the dark appearance: 1 pt lighter than the background along
//! the whole outline, above the content (`plan/reference/windows.md`, "Windows · border and
//! background"). In light there is none.

use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesPixelProgram, Uniform, UniformType, UniformValue};
use smithay::utils::{Logical, Size};

use crate::cache::ShaderCache;
use crate::corners::{CORNER_EXPONENT, CORNER_RADIUS};

/// The border colors (sRGB 0–1): top row, sides, bottom row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderStyle {
    pub top: [f32; 3],
    pub side: [f32; 3],
    pub bottom: [f32; 3],
}

const fn gray(level: u8) -> [f32; 3] {
    let v = level as f32 / 255.0;
    [v, v, v]
}

/// Active, dark: `#3d3d3d` (61) on every side, against the `#272728` background.
pub const ACTIVE_DARK: BorderStyle = BorderStyle {
    top: gray(0x3d),
    side: gray(0x3d),
    bottom: gray(0x3d),
};

/// Inactive, dark: `#404041` on top (64), `#3a3a3b` on the left (58). The bottom row is not
/// measured: like the sides.
pub const INACTIVE_DARK: BorderStyle = BorderStyle {
    top: [
        0x40 as f32 / 255.0,
        0x40 as f32 / 255.0,
        0x41 as f32 / 255.0,
    ],
    side: [
        0x3a as f32 / 255.0,
        0x3a as f32 / 255.0,
        0x3b as f32 / 255.0,
    ],
    bottom: [
        0x3a as f32 / 255.0,
        0x3a as f32 / 255.0,
        0x3b as f32 / 255.0,
    ],
};

/// The border of a window according to state and appearance; none in light.
pub fn style(active: bool, dark: bool) -> Option<BorderStyle> {
    match (dark, active) {
        (false, _) => None,
        (true, true) => Some(ACTIVE_DARK),
        (true, false) => Some(INACTIVE_DARK),
    }
}

pub const SHADER: &str = include_str!("shaders/border.frag");
pub const UNIFORMS: [(&str, UniformType); 6] = [
    ("radius", UniformType::_1f),
    ("exponent", UniformType::_1f),
    ("pixel", UniformType::_1f),
    ("top", UniformType::_3f),
    ("side", UniformType::_3f),
    ("bottom", UniformType::_3f),
];

/// The border of a window, kept in its data.
#[derive(Default)]
pub struct WindowBorder {
    cache: ShaderCache<(Size<i32, Logical>, BorderStyle, u32)>,
}

impl WindowBorder {
    /// The border element for a frame (logical, in the output).
    pub fn element(
        &self,
        program: &GlesPixelProgram,
        frame: smithay::utils::Rectangle<i32, Logical>,
        style: BorderStyle,
        scale: f64,
    ) -> PixelShaderElement {
        let pixel = (1.0 / scale.max(1.0)) as f32;
        self.cache
            .element(program, frame, (frame.size, style, pixel.to_bits()), || {
                let radius = (CORNER_RADIUS as f32)
                    .min(frame.size.w as f32 / 2.0)
                    .min(frame.size.h as f32 / 2.0)
                    .max(0.0);
                let color =
                    |name, [r, g, b]: [f32; 3]| Uniform::new(name, UniformValue::_3f(r, g, b));
                vec![
                    Uniform::new("radius", UniformValue::_1f(radius)),
                    Uniform::new("exponent", UniformValue::_1f(CORNER_EXPONENT)),
                    Uniform::new("pixel", UniformValue::_1f(pixel)),
                    color("top", style.top),
                    color("side", style.side),
                    color("bottom", style.bottom),
                ]
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_only_in_dark() {
        assert_eq!(style(true, false), None);
        assert_eq!(style(false, false), None);
        assert_eq!(style(true, true), Some(ACTIVE_DARK));
        assert_eq!(style(false, true), Some(INACTIVE_DARK));
        assert_eq!(ACTIVE_DARK.top, gray(61));
    }
}
