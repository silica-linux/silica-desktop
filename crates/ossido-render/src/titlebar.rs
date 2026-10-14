//! Title bar of the server-side decorations: 52 pt like the measured toolbar, with the
//! three window buttons (`plan/reference/windows.md`, "Toolbar", "Window buttons" and
//! "Windows · border and background"), in light and dark.

use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesPixelProgram, Uniform, UniformType, UniformValue};
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::cache::ShaderCache;
use crate::corners::{CORNER_EXPONENT, CORNER_RADIUS};

/// Bar height, in points.
pub const HEIGHT: i32 = 52;
/// Centers of the buttons (close, minimize, full screen) in the bar: x 26, 49, 72, y 26.
pub const BUTTON_CENTERS: [(f64, f64); 3] = [(26.0, 26.0), (49.0, 26.0), (72.0, 26.0)];
/// Radius of the active zone of each button: the accessibility cell is 16 × 16 pt.
pub const BUTTON_HIT_RADIUS: f64 = 8.0;

/// Bar background: like the content of the active window in light (`#ffffff`).
const BACKGROUND: [f32; 3] = [1.0, 1.0, 1.0];
/// Dark: like the content when the window is active (`#272728`); when inactive the bar has
/// its own background, `#2f2f30`.
const BACKGROUND_DARK: [f32; 3] = [39.0 / 255.0, 39.0 / 255.0, 40.0 / 255.0];
const BACKGROUND_DARK_INACTIVE: [f32; 3] = [47.0 / 255.0, 47.0 / 255.0, 48.0 / 255.0];
/// Inactive window: 1 pt row `#e5e5e5` between bar and content; in dark `#363636`.
const SEPARATOR_INACTIVE: [f32; 4] = [229.0 / 255.0, 229.0 / 255.0, 229.0 / 255.0, 1.0];
const SEPARATOR_INACTIVE_DARK: [f32; 4] = [54.0 / 255.0, 54.0 / 255.0, 54.0 / 255.0, 1.0];

pub const SHADER: &str = include_str!("shaders/titlebar.frag");
pub const UNIFORMS: [(&str, UniformType); 14] = [
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
    ("dark", UniformType::_1f),
];

/// Bar state that changes the drawing.
/// Which window buttons are not available (grey discs without a symbol, no action).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Disabled {
    #[default]
    None,
    /// A fixed-size window (minimum equal to maximum): full screen only.
    Zoom,
    /// A dialog (it has a parent): minimize and full screen.
    MinimizeAndZoom,
}

impl Disabled {
    pub fn minimize(self) -> bool {
        self == Self::MinimizeAndZoom
    }

    pub fn zoom(self) -> bool {
        self != Self::None
    }

    fn uniform(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Zoom => 1.0,
            Self::MinimizeAndZoom => 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TitlebarState {
    pub active: bool,
    /// Opacity of the symbols (0–1): the pointer on the group shows them, even in an
    /// inactive window, which meanwhile takes back the active colors.
    pub symbols: f32,
    /// The pressed button (0 close, 1 minimize, 2 full screen) with the pointer still
    /// over it.
    pub pressed: Option<u8>,
    /// ⌥ held: the green one shows "+" (zoom) instead of full screen.
    pub alt: bool,
    /// Buttons not available: full screen for a fixed-size window, minimize and full screen
    /// for a dialog.
    pub disabled: Disabled,
    pub scale: f64,
    /// The bar background: false with full-size content (`silica-window-v1`), where only
    /// the buttons above the app remain.
    pub bar: bool,
    /// The vertical center of the buttons (26 in Ossido's bar; half of the app's toolbar
    /// with full-size content).
    pub button_y: f32,
    /// Dark appearance.
    pub dark: bool,
}

/// Button symbols over time (windows.md, "Other states"): when the pointer enters they are
/// already at 80 % and full in ~90 ms; when it leaves they stay ~60 ms and vanish in ~55 ms.
pub fn symbols_opacity(since_enter_ms: f64, since_leave_ms: Option<f64>) -> f32 {
    let shown = (0.8 + 0.2 * (since_enter_ms / 90.0)).clamp(0.0, 1.0);
    let fading = match since_leave_ms {
        None => 1.0,
        Some(ms) => (1.0 - (ms - 60.0) / 55.0).clamp(0.0, 1.0),
    };
    (shown * fading) as f32
}

/// The bar of a window, kept in the window's data.
#[derive(Default)]
pub struct Titlebar {
    cache: ShaderCache<(Size<i32, Logical>, TitlebarState)>,
}

impl Titlebar {
    /// The bar element for a frame (bar + content, logical, in the output).
    pub fn element(
        &self,
        program: &GlesPixelProgram,
        frame: Rectangle<i32, Logical>,
        state: TitlebarState,
    ) -> PixelShaderElement {
        // With full-size content the band reaches below the buttons, centered lower
        // if the app's bar is taller.
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
    let separator = match (state.active, state.dark) {
        (true, _) => [0.0; 4],
        (false, false) => SEPARATOR_INACTIVE,
        (false, true) => SEPARATOR_INACTIVE_DARK,
    };
    let background = match (state.active, state.dark) {
        (_, false) => BACKGROUND,
        (true, true) => BACKGROUND_DARK,
        (false, true) => BACKGROUND_DARK_INACTIVE,
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
            UniformValue::_3f(background[0], background[1], background[2]),
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
        Uniform::new("disabled", UniformValue::_1f(state.disabled.uniform())),
        Uniform::new("bar", UniformValue::_1f(flag(state.bar))),
        Uniform::new("button_y", UniformValue::_1f(state.button_y)),
        Uniform::new("dark", UniformValue::_1f(flag(state.dark))),
    ]
}

impl Part {
    /// Index of the button (0 close, 1 minimize, 2 full screen).
    pub fn button(self) -> Option<u8> {
        match self {
            Part::Close => Some(0),
            Part::Minimize => Some(1),
            Part::Zoom => Some(2),
            Part::Bar => None,
        }
    }
}

/// A part of the bar under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Close,
    Minimize,
    Zoom,
    /// The rest of the bar: dragging it moves the window.
    Bar,
}

/// The part of the bar at a point relative to the top left corner of the frame.
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

/// The pointer is on the button group: the symbols appear on all three.
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
        // On the first frame ~80 %, full within ~90 ms.
        assert!((symbols_opacity(0.0, None) - 0.8).abs() < 1e-6);
        assert_eq!(symbols_opacity(100.0, None), 1.0);
        // Exit: still for 60 ms, then gone in ~55 ms.
        assert_eq!(symbols_opacity(500.0, Some(50.0)), 1.0);
        assert!(symbols_opacity(500.0, Some(90.0)) < 0.5);
        assert_eq!(symbols_opacity(500.0, Some(120.0)), 0.0);
        assert_eq!(Part::Zoom.button(), Some(2));
        assert_eq!(Part::Bar.button(), None);
    }
}
