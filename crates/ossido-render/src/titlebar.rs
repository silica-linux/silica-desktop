//! Title bar of the server-side decorations: 32 pt like the measured plain title bar, with
//! the three window buttons (`plan/reference/windows.md`, "Window buttons" and
//! "Windows · border and background"; `plan/reference/titlebar.md` §1), in light and dark.
//!
//! Button placement rule (TITLEBAR-GLASS, #86): 14 pt discs at a 23 pt pitch, always
//! centred vertically on the bar; the first centre is 16 pt from the left edge in bars up to
//! 32 pt (9 pt margin) and 26 pt in 52 pt toolbars (19 pt margin). The buttons never move or
//! shrink with the window width: apps declare a minimum width that keeps them clear.

use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesPixelProgram, Uniform, UniformType, UniformValue};
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::cache::ShaderCache;
use crate::corners::{CORNER_EXPONENT, CORNER_RADIUS};

/// Height of Ossido's own bar (a plain title bar), in points.
pub const HEIGHT: i32 = 32;
/// Height of the unified toolbars (drawn by the app with full-size content).
pub const TOOLBAR_HEIGHT: f64 = 52.0;
/// Distance between the centres of two buttons.
pub const BUTTON_PITCH: f64 = 23.0;
/// First button centre x in bars up to [`HEIGHT`] tall (9 pt margin from the edge).
pub const BUTTON_X_PLAIN: f64 = 16.0;
/// First button centre x in [`TOOLBAR_HEIGHT`] toolbars (19 pt margin).
pub const BUTTON_X_TOOLBAR: f64 = 26.0;
/// Radius of the active zone of each button: the accessibility cell is 16 × 16 pt.
pub const BUTTON_HIT_RADIUS: f64 = 8.0;

/// The first button centre x for a bar of the given height: 16 up to 32 pt, 26 from 52 pt,
/// in between (not measured) a linear blend rounded to a whole point.
pub fn button_x(bar: f64) -> f64 {
    let t = ((bar - f64::from(HEIGHT)) / (TOOLBAR_HEIGHT - f64::from(HEIGHT))).clamp(0.0, 1.0);
    (BUTTON_X_PLAIN + t * (BUTTON_X_TOOLBAR - BUTTON_X_PLAIN)).round()
}

/// Centres of the buttons (close, minimize, full screen) in a bar of the given height:
/// always on the vertical centre of the bar.
pub fn button_centers(bar: f64) -> [(f64, f64); 3] {
    let (x, y) = (button_x(bar), bar / 2.0);
    [(x, y), (x + BUTTON_PITCH, y), (x + 2.0 * BUTTON_PITCH, y)]
}

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
pub const UNIFORMS: [(&str, UniformType); 21] = [
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
    ("button_x", UniformType::_1f),
    ("button_y", UniformType::_1f),
    ("dark", UniformType::_1f),
    ("spec_c", UniformType::_4f),
    ("rim_c", UniformType::_4f),
    ("glow_c", UniformType::_4f),
    ("spec_g", UniformType::_4f),
    ("rim_g", UniformType::_4f),
    ("glow_g", UniformType::_4f),
];

/// Glass reflections of one disc (TL-GLASS, #165): white layers over the gradient and
/// outline. Every value is `provisional`: tuned by eye on the 1x crops
/// `finder-traffic-*` (the gradient and the bright dark-mode outline already carry part of
/// the effect), pending the TL-GLASS measurement at 2x.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reflection {
    /// Top specular highlight: peak whiteness, center y (up is negative), half width and
    /// half height of the ellipse, in points from the disc center.
    pub specular: [f32; 4],
    /// Subtle inner rim: peak whiteness, radius, half width.
    pub rim: [f32; 3],
    /// Soft glow at the bottom: peak whiteness, center y, radius.
    pub glow: [f32; 3],
}

impl Reflection {
    /// Colored discs (active window, hover with glyphs); provisional.
    pub const fn colored(dark: bool) -> Self {
        if dark {
            Self {
                specular: [0.16, -3.7, 4.0, 1.9],
                rim: [0.06, 5.9, 0.8],
                glow: [0.10, 6.0, 5.5],
            }
        } else {
            Self {
                specular: [0.22, -3.7, 4.0, 1.9],
                rim: [0.10, 5.9, 0.8],
                glow: [0.14, 6.0, 5.5],
            }
        }
    }

    /// Gray discs (inactive and disabled); provisional.
    pub const fn grey(dark: bool) -> Self {
        if dark {
            Self {
                specular: [0.10, -3.7, 4.0, 1.9],
                rim: [0.05, 5.9, 0.8],
                glow: [0.08, 6.0, 5.5],
            }
        } else {
            Self {
                specular: [0.40, -3.7, 4.0, 1.9],
                rim: [0.30, 5.9, 0.8],
                glow: [0.20, 6.0, 5.5],
            }
        }
    }

    /// Whiteness added at a point `(dx, dy)` from the disc center (the same math as
    /// `reflections()` in the shader).
    pub fn whiteness(&self, dx: f32, dy: f32) -> f32 {
        let smooth = |x: f32| {
            let t = x.clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let [sa, sy, sw, sh] = self.specular;
        let spec = sa * (1.0 - smooth((dx / sw).hypot((dy - sy) / sh)));
        let [ra, rr, rw] = self.rim;
        let rim = ra * (1.0 - ((dx.hypot(dy) - rr).abs() / rw)).max(0.0);
        let [ga, gy, gr] = self.glow;
        let glow = ga * (1.0 - smooth(dx.hypot(dy - gy) / gr));
        (spec + rim + glow).clamp(0.0, 1.0)
    }
}

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
    /// The bar height the buttons are placed for (32 in Ossido's bar; the app's toolbar
    /// with full-size content): see [`button_centers`].
    pub bar_height: f32,
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
        let height = HEIGHT.max(state.bar_height.ceil() as i32);
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
    let centers = button_centers(f64::from(state.bar_height));
    let flag = |on: bool| if on { 1.0 } else { 0.0 };
    let (colored, grey) = (Reflection::colored(state.dark), Reflection::grey(state.dark));
    let four = |name: &'static str, a: f32, b: [f32; 3]| {
        Uniform::new(name, UniformValue::_4f(a, b[0], b[1], b[2]))
    };
    let spec = |name: &'static str, r: Reflection| {
        let [a, y, w, h] = r.specular;
        Uniform::new(name, UniformValue::_4f(a, y, w, h))
    };
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
        Uniform::new("button_x", UniformValue::_1f(centers[0].0 as f32)),
        Uniform::new("button_y", UniformValue::_1f(centers[0].1 as f32)),
        Uniform::new("dark", UniformValue::_1f(flag(state.dark))),
        spec("spec_c", colored),
        four("rim_c", colored.rim[0], [colored.rim[1], colored.rim[2], 0.0]),
        four("glow_c", colored.glow[0], [colored.glow[1], colored.glow[2], 0.0]),
        spec("spec_g", grey),
        four("rim_g", grey.rim[0], [grey.rim[1], grey.rim[2], 0.0]),
        four("glow_g", grey.glow[0], [grey.glow[1], grey.glow[2], 0.0]),
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

/// The part of the bar at a point relative to the top left corner of the frame, for a bar
/// `bar` pt tall.
pub fn part_at(point: Point<f64, Logical>, bar: f64) -> Option<Part> {
    if point.y < 0.0 || point.y >= bar || point.x < 0.0 {
        return None;
    }
    let parts = [Part::Close, Part::Minimize, Part::Zoom];
    for ((x, y), part) in button_centers(bar).iter().zip(parts) {
        let (dx, dy) = (point.x - x, point.y - y);
        if dx.abs() <= BUTTON_HIT_RADIUS && dy.abs() <= BUTTON_HIT_RADIUS {
            return Some(part);
        }
    }
    Some(Part::Bar)
}

/// The pointer is on the button group: the symbols appear on all three.
pub fn over_buttons(point: Point<f64, Logical>, bar: f64) -> bool {
    let centers = button_centers(bar);
    let (first, last) = (centers[0], centers[2]);
    point.x >= first.0 - BUTTON_HIT_RADIUS
        && point.x <= last.0 + BUTTON_HIT_RADIUS
        && (point.y - first.1).abs() <= BUTTON_HIT_RADIUS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_bar_buttons_sit_9_pt_from_the_edge_centred_on_16() {
        let c = button_centers(32.0);
        assert_eq!(c, [(16.0, 16.0), (39.0, 16.0), (62.0, 16.0)]);
        // The first disc edge (radius 7) is 9 pt from the left edge.
        assert_eq!(c[0].0 - 7.0, 9.0);
        // Shorter bars keep the same margin; the centre follows the bar.
        assert_eq!(button_centers(28.0)[0], (16.0, 14.0));
    }

    #[test]
    fn toolbar_buttons_sit_19_pt_from_the_edge_centred_on_26() {
        let c = button_centers(52.0);
        assert_eq!(c, [(26.0, 26.0), (49.0, 26.0), (72.0, 26.0)]);
        assert_eq!(c[0].0 - 7.0, 19.0);
        // Taller than a toolbar: the margin stays, the centre follows the bar.
        assert_eq!(button_centers(60.0)[0], (26.0, 30.0));
        // In between: blended, never outside the two measured margins.
        let x = button_x(42.0);
        assert!((16.0..=26.0).contains(&x));
    }

    #[test]
    fn parts_follow_the_measured_buttons() {
        // 32 pt bar.
        assert_eq!(part_at(Point::from((16.0, 16.0)), 32.0), Some(Part::Close));
        assert_eq!(
            part_at(Point::from((39.0, 10.0)), 32.0),
            Some(Part::Minimize)
        );
        assert_eq!(part_at(Point::from((62.0, 23.0)), 32.0), Some(Part::Zoom));
        assert_eq!(part_at(Point::from((200.0, 16.0)), 32.0), Some(Part::Bar));
        assert_eq!(part_at(Point::from((16.0, 32.0)), 32.0), None);
        assert_eq!(part_at(Point::from((16.0, -1.0)), 32.0), None);
        assert!(over_buttons(Point::from((50.0, 16.0)), 32.0));
        assert!(!over_buttons(Point::from((90.0, 16.0)), 32.0));
        // 52 pt toolbar.
        assert_eq!(part_at(Point::from((26.0, 26.0)), 52.0), Some(Part::Close));
        assert_eq!(
            part_at(Point::from((49.0, 20.0)), 52.0),
            Some(Part::Minimize)
        );
        assert_eq!(part_at(Point::from((72.0, 33.0)), 52.0), Some(Part::Zoom));
        assert_eq!(part_at(Point::from((26.0, 52.0)), 52.0), None);
        assert!(over_buttons(Point::from((60.0, 26.0)), 52.0));
        assert!(!over_buttons(Point::from((100.0, 26.0)), 52.0));
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

#[cfg(test)]
mod reflection_tests {
    use super::*;

    #[test]
    fn highlight_sits_on_top_and_glow_at_the_bottom() {
        for dark in [false, true] {
            for r in [Reflection::colored(dark), Reflection::grey(dark)] {
                assert!(r.specular[1] < 0.0 && r.glow[1] > 0.0);
                let (top, bottom, center) =
                    (r.whiteness(0.0, -3.7), r.whiteness(0.0, 5.0), r.whiteness(0.0, 0.0));
                // The specular peak is the brightest spot of the upper half; the center stays
                // mostly clean; everything stays a partial overlay.
                assert!(top > center, "dark={dark}");
                assert!(bottom > center, "dark={dark}");
                assert!(top < 0.8 && bottom < 0.8);
            }
        }
    }

    #[test]
    fn colored_light_discs_have_the_stronger_highlight_than_dark() {
        assert!(Reflection::colored(false).specular[0] > Reflection::colored(true).specular[0]);
    }

    /// Rendered at 2x (CPU model of the shader: the crate has no offscreen GL test path):
    /// the highlight is symmetric left-right and nothing is added outside the disc rim.
    #[test]
    fn rendered_button_pixels_are_symmetric_and_bounded() {
        let r = Reflection::colored(false);
        let n = 28; // 14 pt at 2x
        let sample = |px: i32, py: i32| {
            r.whiteness((px as f32 + 0.5) / 2.0 - 7.0, (py as f32 + 0.5) / 2.0 - 7.0)
        };
        for py in 0..n {
            for px in 0..n / 2 {
                assert!((sample(px, py) - sample(n - 1 - px, py)).abs() < 1e-6);
            }
        }
        let top_row: f32 = (0..n).map(|x| sample(x, 5)).sum();
        let mid_row: f32 = (0..n).map(|x| sample(x, 14)).sum();
        assert!(top_row > mid_row);
        assert!(r.whiteness(0.0, -20.0) < 1e-6 && r.whiteness(20.0, 0.0) < 1e-6);
    }

    #[test]
    fn uniform_list_covers_the_reflection_uniforms() {
        let names: Vec<_> = UNIFORMS.iter().map(|(n, _)| *n).collect();
        for n in ["spec_c", "rim_c", "glow_c", "spec_g", "rim_g", "glow_g"] {
            assert!(names.contains(&n));
        }
    }
}
