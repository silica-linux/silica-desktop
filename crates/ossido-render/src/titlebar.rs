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
pub const UNIFORMS: [(&str, UniformType); 19] = [
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
    ("edge_fade", UniformType::_1f),
    ("edge_weak", UniformType::_1f),
    ("glass", UniformType::_3f),
    ("shadow", UniformType::_1f),
];

/// Radius of a button disc, in points.
const DISC_RADIUS: f32 = 7.0;

/// Glass of one disc (TL-GLASS, #94, measured at 1x on the dark and light discs).
///
/// Dark: a 1 px light arc only on the top and bottom edges (none on the sides), fading
/// over about ±55° from the vertical axis; inside, a darker band at 30–45 % of the height
/// and a round glow at 75–80 % of the height, ~10 px wide. Light: no light rim, a darker
/// outline strongest at the sides and weak at the top and bottom, a plain top-to-bottom
/// gradient and a 1 px shadow under the disc. Hover does not change the glass (only the
/// symbols appear).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glass {
    /// Angle from the vertical axis (radians) over which the outline goes from its
    /// top/bottom color to its side color: the arc fade in dark; in light the whole
    /// quarter turn (side outline at the sides, weaker towards top and bottom).
    pub edge_fade: f32,
    /// Light only: how much of the side outline stays at the top and bottom (0 = the
    /// fill color, 1 = as strong as the sides); dark spells its arc colors out.
    pub edge_weak: f32,
    /// The darker band as fractions of the disc height from the top (dark only).
    pub band: [f32; 2],
    /// The round glow: center as a fraction of the height from the top, and radius in
    /// points (dark only; radius 0 = none).
    pub glow_center: f32,
    pub glow_radius: f32,
    /// Alpha of the 1 px black shadow under the disc (light only).
    pub shadow: f32,
}

impl Glass {
    pub const fn new(dark: bool) -> Self {
        if dark {
            Self {
                edge_fade: 0.959_931, // 55°
                edge_weak: 1.0,
                band: [0.30, 0.45],
                glow_center: 0.775,
                glow_radius: 5.0,
                shadow: 0.0,
            }
        } else {
            Self {
                edge_fade: std::f32::consts::FRAC_PI_2,
                edge_weak: 0.35,
                band: [0.30, 0.45],
                glow_center: 0.775,
                glow_radius: 0.0,
                // lum 243 on 255 under the disc.
                shadow: 12.0 / 255.0,
            }
        }
    }

    /// Offset from the disc center (up is negative) of a fraction of the height.
    fn at(fraction: f32) -> f32 {
        (fraction - 0.5) * 2.0 * DISC_RADIUS
    }

    /// The darker band, as offsets from the disc center.
    pub fn band_range(&self) -> [f32; 2] {
        [Self::at(self.band[0]), Self::at(self.band[1])]
    }

    /// Glow center y, as an offset from the disc center.
    pub fn glow_y(&self) -> f32 {
        Self::at(self.glow_center)
    }

    /// How far the outline at `(dx, dy)` from the center is towards its top/bottom color:
    /// 1 on the vertical axis, 0 from `edge_fade` away (the same math as the shader).
    pub fn edge_weight(&self, dx: f32, dy: f32) -> f32 {
        let theta = dx.abs().atan2(dy.abs());
        let t = (theta / self.edge_fade).clamp(0.0, 1.0);
        1.0 - t * t * (3.0 - 2.0 * t)
    }

    /// Weight of the glow color at `(dx, dy)`: round, 0 outside its radius.
    pub fn glow(&self, dx: f32, dy: f32) -> f32 {
        if self.glow_radius <= 0.0 {
            return 0.0;
        }
        let t = (dx.hypot(dy - self.glow_y()) / self.glow_radius).clamp(0.0, 1.0);
        1.0 - t * t
    }

    /// Alpha of the shadow at `(dx, dy)` from the disc center: the disc moved 1 pt down
    /// and hard-edged (the part under the disc itself is covered by it).
    pub fn shadow_at(&self, dx: f32, dy: f32) -> f32 {
        if dx.hypot(dy - 1.0) <= DISC_RADIUS {
            self.shadow
        } else {
            0.0
        }
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
    let glass = Glass::new(state.dark);
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
        Uniform::new("edge_fade", UniformValue::_1f(glass.edge_fade)),
        Uniform::new("edge_weak", UniformValue::_1f(glass.edge_weak)),
        Uniform::new(
            "glass",
            UniformValue::_3f(glass.band_range()[0], glass.glow_y(), glass.glow_radius),
        ),
        Uniform::new("shadow", UniformValue::_1f(glass.shadow)),
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
mod glass_tests {
    use super::*;

    #[test]
    fn measured_parameters() {
        let dark = Glass::new(true);
        // Arc fading over ~±55° from the vertical axis.
        assert!((dark.edge_fade.to_degrees() - 55.0).abs() < 0.01);
        // Darker band at 30–45 % of the height, glow centered at 75–80 %.
        assert_eq!(dark.band, [0.30, 0.45]);
        assert!((0.75..=0.80).contains(&dark.glow_center));
        // ~10 px wide glow.
        assert_eq!(dark.glow_radius * 2.0, 10.0);
        let [lo, hi] = dark.band_range();
        assert!((lo - -2.8).abs() < 1e-5 && (hi - -0.7).abs() < 1e-5);
        assert!(dark.glow_y() > 3.0 && dark.glow_y() < 4.5);
        assert_eq!(dark.shadow, 0.0);

        let light = Glass::new(false);
        // No glow, the outline goes the whole quarter turn, weak at top and bottom.
        assert_eq!(light.glow_radius, 0.0);
        assert!((light.edge_fade - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(light.edge_weak > 0.0 && light.edge_weak < 1.0);
        // 1 px shadow, lum 243 on 255.
        assert!((light.shadow * 255.0 - 12.0).abs() < 1e-4);
    }

    /// CPU model of the shader at 1x (1 pt = 1 px): the dark arc is on the top and bottom
    /// edges only, brightest at the center, gone on the sides.
    #[test]
    fn dark_arc_is_only_on_top_and_bottom() {
        let g = Glass::new(true);
        assert!((g.edge_weight(0.0, -6.5) - 1.0).abs() < 1e-6);
        assert!((g.edge_weight(0.0, 6.5) - 1.0).abs() < 1e-6);
        assert_eq!(g.edge_weight(6.5, 0.0), 0.0);
        assert_eq!(g.edge_weight(-6.5, 0.0), 0.0);
        // Fading along the top row: monotonic from the center, ~0 at ±55°.
        let row: Vec<f32> = (0..7)
            .map(|x| g.edge_weight(x as f32 + 0.5, -(49.0 - (x as f32 + 0.5).powi(2)).sqrt()))
            .collect();
        assert!(row.windows(2).all(|w| w[0] >= w[1]));
        assert!(row[0] > 0.9 && row[6] < 0.1);
    }

    #[test]
    fn light_outline_is_strongest_at_the_sides() {
        let g = Glass::new(false);
        assert_eq!(g.edge_weight(7.0, 0.0), 0.0); // side color
        assert!((g.edge_weight(0.0, -7.0) - 1.0).abs() < 1e-6); // weak color
    }

    /// Dark glow: round, in the lower half, centered, symmetric left-right at 1x.
    #[test]
    fn glow_is_round_in_the_lower_half_and_symmetric() {
        let g = Glass::new(true);
        let sample = |px: i32, py: i32| g.glow(px as f32 + 0.5 - 7.0, py as f32 + 0.5 - 7.0);
        for py in 0..14 {
            for px in 0..7 {
                assert!((sample(px, py) - sample(13 - px, py)).abs() < 1e-6);
            }
        }
        // Row at 75–80 % (y 10–11): centre brighter than the sides; none in the upper half.
        assert!(sample(6, 10) > sample(2, 10));
        assert!(sample(6, 10) > 0.8);
        assert_eq!(sample(6, 2), 0.0);
        assert_eq!(sample(6, 5), 0.0);
        // Round: as wide as tall around the center.
        let (cx, cy) = (0.0, g.glow_y());
        assert!((g.glow(cx + 3.0, cy) - g.glow(cx, cy + 3.0)).abs() < 1e-6);
        assert_eq!(Glass::new(false).glow(0.0, 4.0), 0.0);
    }

    #[test]
    fn light_shadow_sits_one_pixel_below_the_disc() {
        let g = Glass::new(false);
        // Under the bottom edge (y 7..8): a shadow; beside or above the disc: none.
        assert!(g.shadow_at(0.0, 7.5) > 0.0);
        assert_eq!(g.shadow_at(0.0, 8.5), 0.0);
        assert_eq!(g.shadow_at(0.0, -7.5), 0.0);
        assert_eq!(g.shadow_at(8.0, 0.0), 0.0);
        assert_eq!(Glass::new(true).shadow_at(0.0, 7.5), 0.0);
    }

    #[test]
    fn uniform_list_covers_the_glass_uniforms() {
        let names: Vec<_> = UNIFORMS.iter().map(|(n, _)| *n).collect();
        for n in ["edge_fade", "edge_weak", "glass", "shadow"] {
            assert!(names.contains(&n));
        }
    }
}
