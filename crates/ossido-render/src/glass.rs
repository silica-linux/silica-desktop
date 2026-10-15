//! Glass: the background behind a surface, blurred and transformed into the material
//! color, drawn in the regions declared with `silica-glass-v1`.
//!
//! For every surface with glass: the elements behind it are redrawn into an offscreen
//! texture (regions plus the blur margin), blurred with a separable Gaussian, mixed with the
//! measured sharp share, and a `GlassElement` per region draws it with the color matrix and
//! superellipse corners. The textures are reused while the background does not change.
//! Parameters from `plan/reference/glass.md` and `menubar.md` (light appearance).

use std::cell::RefCell;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformType,
    UniformValue,
};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Bind, Frame, Offscreen, Renderer};
use smithay::utils::{Buffer, Physical, Point, Rectangle, Scale, Size, Transform};

use crate::glass_profile::{GlassProfile, lens_width};

/// Parameters of a material: blur, sharp share and color (`out = M · in + b + q₂ · l² + q₃ · l³`
/// on the encoded sRGB values, 0–1, l the mean of the input channels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialParams {
    /// σ of the Gaussian in points.
    pub sigma: f64,
    /// Share of the sharp image mixed with the blurred one.
    pub sharp: f32,
    pub matrix: [[f32; 3]; 3],
    pub offset: [f32; 3],
    /// Quadratic and cubic terms on the input luminance (`q₂`, `q₃`): some transfers bend.
    pub curve: [[f32; 3]; 2],
    /// The edge lens.
    pub lens: Lens,
    /// The edge row and the inner glow.
    pub edge: Edge,
    /// Gap below which two regions of this material merge (0: never).
    pub merge_pt: f32,
}

/// The glass edge, measured across the edges of the reference captures on gray
/// (`xtask/tools/glass_parity.py`): for each orientation (top, sides, bottom) a profile of
/// offsets inside the shape and the darkening just outside it. The shader picks the
/// orientation from the shape's own normal (SDF), so corners, tips and the bridges between
/// merged shapes blend continuously.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub top: EdgeProfile,
    pub side: EdgeProfile,
    pub bottom: EdgeProfile,
}

/// One orientation of the edge.
/// - `inner[i]`: offset in levels (0–255) of the i-th point inside the edge, relative to the
///   fill, as measured on the reference gray. Positive offsets are lights: they add the same
///   amount over darker backgrounds and less over lighter ones (over white the Dock's +61
///   row is +8, over black +60: glass.md › Luminous edge), scaled by
///   min(1, (1 − c) / (1 − c_gray)). Negative offsets are shadows: they multiply.
/// - `outer[i]`: alpha of black on the i-th point outside the edge (a dark outline drawn over
///   whatever is behind).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeProfile {
    pub inner: [f32; 12],
    pub outer: [f32; 2],
}

const fn profile(inner: [f32; 12], outer: [f32; 2]) -> EdgeProfile {
    EdgeProfile { inner, outer }
}

const NONE: [f32; 2] = [0.0, 0.0];

/// Dock, light: 1 pt light row on top and at the bottom (+61 over the `#818289` fill, the
/// same on both sides in the gray capture) with an inner glow, 1 dark pt on the sides
/// (61 over the fill's 132), nothing outside. Light Dock over grey (silica-reference, glass.md), columns x 1095–1408 and
/// rows 1380–1410 (medians).
const DOCK_EDGE: Edge = Edge {
    top: profile(
        [61.0, 13.3, 8.0, 5.0, 3.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        NONE,
    ),
    side: profile(
        [-71.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        NONE,
    ),
    bottom: profile(
        [61.0, 13.3, 8.0, 5.0, 3.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        NONE,
    ),
};

/// Dock, dark: +38 rows on top and at the bottom over the `#6a6a6f` fill, glow, sides 49 over
/// the fill's 106 (dark Dock over grey, glass.md).
const DOCK_DARK_EDGE: Edge = Edge {
    top: profile(
        [38.0, 8.3, 5.0, 3.0, 2.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        NONE,
    ),
    side: profile(
        [-58.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        NONE,
    ),
    bottom: profile(
        [38.0, 8.3, 5.0, 3.0, 2.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        NONE,
    ),
};

/// Control Center modules (`regular`): a strong light row on top and at the bottom (+94 /
/// +90 over the `#6a6b70` fill) with a glow over ~6 pt; on the sides the dark line is
/// **outside** the module (black ~0.63, the "1 pt border sticking out" of
/// controlcenter.md), nothing inside. Dark modules over grey (glass.md), the middle rows and columns of the
/// eleven modules (identical in light).
const REGULAR_EDGE: Edge = Edge {
    top: profile(
        [
            94.0, 22.8, 13.8, 8.4, 5.8, 3.8, 1.1, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
        NONE,
    ),
    side: profile(
        [-2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.63, 0.0],
    ),
    bottom: profile(
        [
            90.0, 19.0, 11.0, 7.7, 5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
        NONE,
    ),
};

/// Menus, light: a dark outline all around, outside the body (black ~0.2: menubar.md
/// "outer line ~24 %", 0.16–0.30 over the shadowed gray of the captures), and inside a light
/// row, stronger at the bottom (+30) than on top (+14) and faint on the sides (+4, then a
/// slightly darker band of ~3 levels). Light menu over grey (top) and
/// light Dock context menu (bottom and sides), glass.md and misc.md.
const MENU_EDGE: Edge = Edge {
    top: profile(
        [14.2, 5.7, 4.8, 3.7, 2.8, 1.8, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.2, 0.015],
    ),
    side: profile(
        [
            3.7, -3.0, -3.0, -3.0, -3.0, -2.0, -2.0, -1.3, -1.0, -0.3, 0.0, 0.0,
        ],
        [0.2, 0.015],
    ),
    bottom: profile(
        [30.4, 5.8, 3.1, 2.1, 1.4, 1.4, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.2, 0.015],
    ),
};

/// Menus, dark: an almost black outline outside (black ~0.85, menubar.md `#111112` over the
/// gray), inside +51 on top, +39 at the bottom, +10 on the sides followed by a darker band
/// (−12 … −4 over 4 pt). Dark menu over grey and dark Dock context menu (glass.md, misc.md).
const MENU_DARK_EDGE: Edge = Edge {
    top: profile(
        [50.9, 6.2, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.85, 0.025],
    ),
    side: profile(
        [
            9.5, -12.2, -10.8, -9.2, -4.0, -0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
        [0.85, 0.025],
    ),
    bottom: profile(
        [38.7, 9.2, 3.3, 2.0, 1.7, 0.7, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.85, 0.025],
    ),
};

/// Notifications: the 1 pt row is dark all around and is **outside** the frame (the client
/// widens the region by 1 pt per side): `#5e6064` on top and `#636367` at the bottom over a
/// ~`#c0` fill (−98), `#393a3d` on the sides (black ~0.70). Inside, the first row on top and at
/// the bottom is ~22 levels lighter, absorbed in 2 pt (`d6 c3 c1 bf`). `notifications.md`,
/// "Banner · material over gray". Not yet in the parity harness.
const NOTIFICATION_EDGE: Edge = Edge {
    top: profile(
        [
            -98.0, 22.0, 3.0, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
        NONE,
    ),
    side: profile(
        [
            -135.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
        NONE,
    ),
    bottom: profile(
        [
            -97.0, 22.0, 3.0, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
        NONE,
    ),
};

impl Edge {
    /// The three profiles as `mat4`s: inner 0–11 (0–1 levels), outer 0–1 in 12–13.
    fn uniform(&self) -> UniformValue {
        let pack = |p: &EdgeProfile| {
            let mut m = [0.0_f32; 16];
            for (slot, value) in m.iter_mut().zip(p.inner) {
                *slot = value / 255.0;
            }
            m[12] = p.outer[0];
            m[13] = p.outer[1];
            m
        };
        UniformValue::Matrix4x4 {
            matrices: vec![pack(&self.top), pack(&self.side), pack(&self.bottom)],
            transpose: false,
        }
    }
}

/// The edge lens: for every side, from which distance from the edge (s) the glass samples the
/// background for a pixel at distance t, in points, with (t, s) points at increasing t (at most
/// 8). s > t magnifies (samples further inside), s < 0 samples outside the edge. Before the
/// first point the first one applies, past the last one s = t.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lens {
    pub side: &'static [(f32, f32)],
    pub top: &'static [(f32, f32)],
    pub bottom: &'static [(f32, f32)],
}

impl Lens {
    /// How far outside the edge it can sample, in points.
    pub fn outward_reach(&self) -> f32 {
        [self.side, self.top, self.bottom]
            .iter()
            .flat_map(|curve| curve.iter().map(|(_, s)| -s))
            .fold(0.0, f32::max)
    }

    /// The largest displacement of the glass, in points (how far inside the edge the sampled
    /// background is from the pixel's own position), and the distance from the edge where it is.
    pub fn max_displacement(&self) -> (f32, f32) {
        [self.side, self.top, self.bottom]
            .iter()
            .flat_map(|curve| curve.iter().map(|(t, s)| (s - t, *t)))
            .fold(
                (0.0, 0.0),
                |best, now| if now.0 > best.0 { now } else { best },
            )
    }

    /// How far inside the edge it can sample, in points (the deepest source of the curves).
    pub fn inward_reach(&self) -> f32 {
        [self.side, self.top, self.bottom]
            .iter()
            .flat_map(|curve| curve.iter().map(|(t, s)| t.max(*s)))
            .fold(0.0, f32::max)
    }

    /// The three curves as arrays of `mat4` (columns: t0, s0, t1, s1), scaled to pixels.
    fn uniform(&self, px: f32) -> UniformValue {
        let pack = |curve: &[(f32, f32)]| {
            // Empty slots have a huge t: the shader stops there.
            let mut m = [1.0e6_f32; 16];
            for ([slot_t, slot_s], (t, s)) in m.as_chunks_mut::<2>().0.iter_mut().zip(curve) {
                *slot_t = t * px;
                *slot_s = s * px;
            }
            m
        };
        UniformValue::Matrix4x4 {
            matrices: vec![pack(self.side), pack(self.top), pack(self.bottom)],
            transpose: false,
        }
    }
}

/// Dock background, light: σ 2.5–3.25 pt (2.75 is used), w 0.28, free matrix with
/// b = (37.3; 39.1; 36.9), fitted on the gray and ramp captures converted to sRGB with their
/// display profile (`xtask/tools/glass_parity.py`; rmse 1.88, 2.03 with glass.md's matrix).
pub const CLEAR_LIGHT: MaterialParams = MaterialParams {
    sigma: 2.75,
    sharp: 0.28,
    matrix: [
        [1.107, -0.331, -0.022],
        [-0.148, 0.822, 0.068],
        [-0.152, -0.345, 1.257],
    ],
    offset: [37.3 / 255.0, 39.1 / 255.0, 36.9 / 255.0],
    curve: [[0.0; 3]; 2],
    lens: DOCK_LENS,
    edge: DOCK_EDGE,
    merge_pt: MERGE_PT,
};

/// Dock background, dark: σ 2.5–3.25 pt (2.875 is used, halfway), w 0.28. The transfer bends
/// (darker over light backgrounds than a matrix gives): free matrix with b = (11.4; 12.7; 11.9)
/// and a quadratic term on the luminance, fitted on the sRGB-converted gray and ramp captures
/// (`xtask/tools/glass_parity.py`; rmse 1.08, 4.7 with glass.md's matrix alone).
pub const CLEAR_DARK: MaterialParams = MaterialParams {
    sigma: 2.875,
    sharp: 0.28,
    matrix: [
        [1.157, -0.259, -0.022],
        [-0.154, 0.858, 0.147],
        [-0.082, -0.273, 1.215],
    ],
    offset: [11.4 / 255.0, 12.7 / 255.0, 11.9 / 255.0],
    curve: [[-0.232, -0.212, -0.218], [0.0; 3]],
    lens: DOCK_LENS,
    edge: DOCK_DARK_EDGE,
    merge_pt: MERGE_PT,
};

/// Dock lens (GLASS-PROFILES, silica-reference #97): the glass shows the background 2-3 pt
/// further inside, at most ~2.5 pt around 12 pt from the edge, and nothing past ~28 pt (the
/// same on every side: the measurement does not tell the sides apart). Each point is
/// (distance from the edge, distance the glass samples at).
const DOCK_LENS: Lens = Lens {
    side: DOCK_LENS_CURVE,
    top: DOCK_LENS_CURVE,
    bottom: DOCK_LENS_CURVE,
};

const DOCK_LENS_CURVE: &[(f32, f32)] = &[
    (0.0, 0.0),
    (4.0, 5.5),
    (8.0, 10.3),
    (12.0, 14.5),
    (16.0, 18.2),
    (20.0, 21.5),
    (24.0, 24.7),
    (28.0, 28.0),
];

/// Control Center modules, light: σ 4.5 pt, w 0.04, free matrix with b = (34.2; 31.6; 32.0),
/// fitted on the sRGB-converted gray and ramp captures (`xtask/tools/glass_parity.py`;
/// rmse 1.41, 2.5 with glass.md's matrix); identical in dark (glass.md, "Control Center").
pub const REGULAR_LIGHT: MaterialParams = MaterialParams {
    sigma: 4.5,
    sharp: 0.04,
    matrix: [
        [0.907, -0.248, -0.064],
        [-0.140, 0.637, 0.110],
        [-0.166, -0.278, 1.049],
    ],
    offset: [34.2 / 255.0, 31.6 / 255.0, 32.0 / 255.0],
    curve: [[0.0; 3]; 2],
    lens: LIQUID_LENS,
    edge: REGULAR_EDGE,
    merge_pt: MERGE_PT,
};

/// Control Center modules, dark: the same fit gives the same matrix within 0.004 (rmse 1.41);
/// edge as in light ("same identical look in light and dark over gray", controlcenter.md).
pub const REGULAR_DARK: MaterialParams = REGULAR_LIGHT;

/// The lens of the regular glass (GLASS-PROFILES, silica-reference #97): the Control Center
/// modules show the background up to ~3.5 pt further inside, at the most around 15 pt from the
/// edge, and nothing past ~30 pt. The same on every side. (The first measurement, which found
/// a fold of 17-25 pt in the outer 6 pt, read the edge light as displacement.)
const LIQUID_LENS: Lens = Lens {
    side: LIQUID_LENS_CURVE,
    top: LIQUID_LENS_CURVE,
    bottom: LIQUID_LENS_CURVE,
};

const LIQUID_LENS_CURVE: &[(f32, f32)] = &[
    (0.0, 0.0),
    (5.0, 7.0),
    (10.0, 13.2),
    (15.0, 18.5),
    (20.0, 22.6),
    (25.0, 26.2),
    (30.0, 30.0),
];

/// The color matrix of a tone: saturation 1.75 around the Rec. 709 luminance (the glass
/// measurements: s 1.65-1.8) with `gain` on gray. Each tone below adds a uniform offset (the
/// output for black) and a quadratic term so the gray axis also hits the mid-grey and white
/// outputs of GLASS-PROFILES (silica-reference #97).
const fn tone_matrix(gain: f32) -> [[f32; 3]; 3] {
    // gain * ((1 - s) * L + s * I), s = 1.75: the luminance weights L = (0.2126, 0.7152, 0.0722)
    // times 1 - s = -0.75, plus s on the diagonal.
    const D: f32 = 1.75;
    const R: f32 = -0.75 * 0.2126;
    const G: f32 = -0.75 * 0.7152;
    const B: f32 = -0.75 * 0.0722;
    [
        [gain * (R + D), gain * G, gain * B],
        [gain * R, gain * (G + D), gain * B],
        [gain * R, gain * G, gain * (B + D)],
    ]
}

const fn flat(level: f32) -> [f32; 3] {
    [level / 255.0; 3]
}

const NO_LENS: Lens = Lens {
    side: &[],
    top: &[],
    bottom: &[],
};

/// Menus, light: black 136, `#7a7b80` 201, white 254 (GLASS-PROFILES). The chroma is the
/// glass family's (saturation 1.75); gain 0.5719 on gray and a −0.1092 quadratic term give the
/// three points (the earlier parity fit of the captures, 158/203/237, missed black and white
/// by more than 3 levels). The blur, 13 pt in light, is the profile's. No lens: thick glass
/// shows none.
pub const THICK_LIGHT: MaterialParams = MaterialParams {
    sigma: 13.0,
    sharp: 0.0,
    matrix: tone_matrix(0.5719),
    offset: flat(136.0),
    curve: [[-0.1092; 3], [0.0; 3]],
    lens: NO_LENS,
    edge: MENU_EDGE,
    merge_pt: MERGE_PT,
};

/// Menus, dark: black 32, `#7a7b80` 114, white 121 (GLASS-PROFILES); the transfer saturates
/// early (gain 0.9418, quadratic term −0.5928). Blur 8 pt, the profile's.
pub const THICK_DARK: MaterialParams = MaterialParams {
    sigma: 8.0,
    matrix: tone_matrix(0.9418),
    offset: flat(32.0),
    curve: [[-0.5928; 3], [0.0; 3]],
    edge: MENU_DARK_EDGE,
    ..THICK_LIGHT
};

/// Toolbar glass controls, light: black 133, `#7a7b80` 193, white 252 (GLASS-PROFILES); thin
/// glass, so the lens of the regular glass. Blur ≈ 6 pt.
pub const CONTROL_LIGHT: MaterialParams = MaterialParams {
    sigma: 6.0,
    sharp: 0.04,
    matrix: tone_matrix(0.4909),
    offset: flat(133.0),
    curve: [[-0.0242; 3], [0.0; 3]],
    lens: LIQUID_LENS,
    edge: REGULAR_EDGE,
    merge_pt: MERGE_PT,
};

/// Toolbar glass controls, dark: black 34, `#7a7b80` 130, white 185.
pub const CONTROL_DARK: MaterialParams = MaterialParams {
    matrix: tone_matrix(0.9304),
    offset: flat(34.0),
    curve: [[-0.3382; 3], [0.0; 3]],
    ..CONTROL_LIGHT
};

/// Popovers, light: the menus' tone (GLASS-PROFILES: "same as menu"), blur 13 pt.
pub const POPOVER_LIGHT: MaterialParams = THICK_LIGHT;

/// Popovers, dark: black 25, `#7a7b80` 112, white 205; blur 18 pt (wider than the menus').
pub const POPOVER_DARK: MaterialParams = MaterialParams {
    sigma: 18.0,
    matrix: tone_matrix(0.6843),
    offset: flat(25.0),
    curve: [[0.0215; 3], [0.0; 3]],
    ..THICK_DARK
};

/// Tooltips, light: black 204, `#7a7b80` 223, white 229 (GLASS-PROFILES): almost opaque, the
/// background only shows through as a few levels. The blur is not measured (hidden by the
/// opacity): 8 pt.
pub const TOOLTIP_LIGHT: MaterialParams = MaterialParams {
    sigma: 8.0,
    matrix: tone_matrix(0.2021),
    offset: flat(204.0),
    curve: [[-0.1041; 3], [0.0; 3]],
    ..THICK_LIGHT
};

/// Tooltips, dark: black 40, `#7a7b80` 60, white 85.
pub const TOOLTIP_DARK: MaterialParams = MaterialParams {
    matrix: tone_matrix(0.1440),
    offset: flat(40.0),
    curve: [[0.0325; 3], [0.0; 3]],
    edge: MENU_DARK_EDGE,
    ..TOOLTIP_LIGHT
};

/// Sheets, light: opaque (GLASS-PROFILES): no background shows through. The fill is the
/// menus' over the reference gray, 201 (the level itself is not measured).
pub const SHEET_LIGHT: MaterialParams = MaterialParams {
    matrix: [[0.0; 3]; 3],
    offset: flat(201.0),
    curve: [[0.0; 3]; 2],
    ..THICK_LIGHT
};

/// Sheets, dark: opaque, 114 (the menus' over the reference gray).
pub const SHEET_DARK: MaterialParams = MaterialParams {
    offset: flat(114.0),
    edge: MENU_DARK_EDGE,
    ..SHEET_LIGHT
};

pub const BLUR_SHADER: &str = include_str!("shaders/blur.frag");
pub const BLUR_UNIFORMS: [(&str, UniformType); 2] =
    [("step", UniformType::_2f), ("sigma", UniformType::_1f)];
pub const MATERIAL_SHADER: &str = include_str!("shaders/material.frag");
pub const MATERIAL_UNIFORMS: [(&str, UniformType); 26] = [
    ("tex_size", UniformType::_2f),
    ("shapes", UniformType::Matrix4x4),
    ("radii", UniformType::Matrix4x4),
    ("tips", UniformType::Matrix4x4),
    ("shape_count", UniformType::_1f),
    ("merge", UniformType::_1f),
    ("exponent", UniformType::_1f),
    ("row_r", UniformType::_3f),
    ("row_g", UniformType::_3f),
    ("row_b", UniformType::_3f),
    ("offset", UniformType::_3f),
    ("curve2", UniformType::_3f),
    ("curve3", UniformType::_3f),
    ("scale_px", UniformType::_1f),
    ("lens", UniformType::Matrix4x4),
    ("lens_reach", UniformType::_1f),
    ("edges", UniformType::Matrix4x4),
    ("fill_gray", UniformType::_1f),
    ("edge_power", UniformType::_1f),
    ("lens_widths", UniformType::Matrix4x4),
    ("refraction", UniformType::_1f),
    ("rim", UniformType::_1f),
    ("line", UniformType::_1f),
    ("saturation", UniformType::_1f),
    ("tint_color", UniformType::_4f),
    ("shadow", UniformType::_3f),
];

/// How fast each orientation's edge (top, sides, bottom) fades around a curve: its weight is
/// the matching component of the outward normal to this power. Fitted on the ends of the
/// Control Center modules over gray (glass.md, `gray-dark` capture): the dark side line is
/// ~0.7 of its strength 15° from the side, ~0.5 at 30° and gone by 45°; the light top row ~0.4
/// at 30° from the top. With the square of the component (the earlier weighting) the side
/// line covered the whole end of a capsule: the black crescents at the left and right ends
/// of modules, pills and menus (#168).
pub const EDGE_POWER: f32 = 5.0;

/// The weights of the top, side and bottom edges, and the shared part, for an outward normal
/// (the shader's `w_top`, `w_side`, `w_bottom`, `w_shared`). Where no orientation owns the
/// curve (a corner's diagonal) the shared part takes what the three agree on, so an outline
/// measured the same on every side (the menus') runs without a gap.
pub fn edge_weights(normal: (f32, f32)) -> [f32; 4] {
    let (x, y) = normal;
    let length = (x * x + y * y).sqrt();
    let (x, y) = if length > 0.0 {
        (x / length, y / length)
    } else {
        (0.0, -1.0)
    };
    let top = (-y).max(0.0).powf(EDGE_POWER);
    let bottom = y.max(0.0).powf(EDGE_POWER);
    let side = x.abs().powf(EDGE_POWER);
    [top, side, bottom, (1.0 - top - side - bottom).max(0.0)]
}

impl MaterialParams {
    /// The fill over the reference gray (`#7a7b80`), 0–1 luminance: the level the edge
    /// offsets were measured against.
    pub fn fill_on_gray(&self) -> f32 {
        self.fill_on_gray_with(&GlassProfile::NEUTRAL)
    }

    /// The same with a profile's saturation and tint over the material.
    pub fn fill_on_gray_with(&self, profile: &GlassProfile) -> f32 {
        let [r, g, b] = profile_color(profile, self.transfer(REFERENCE_GRAY));
        ((r + g + b) / 3.0).clamp(0.01, 0.99)
    }

    /// The material's color transfer on an encoded sRGB color (0-1): what a blurred flat
    /// background of that color shows inside the glass, before the edge, the profile's
    /// saturation and tint (the shader's `out = M * in + b + q2 * l^2 + q3 * l^3`, clamped).
    pub fn transfer(&self, input: [f32; 3]) -> [f32; 3] {
        let [x, y, z] = input;
        let l = (x + y + z) / 3.0;
        let [q2, q3] = self.curve;
        let mut out = [0.0; 3];
        for ((((value, row), offset), q2), q3) in out
            .iter_mut()
            .zip(self.matrix)
            .zip(self.offset)
            .zip(q2)
            .zip(q3)
        {
            let [r0, r1, r2] = row;
            let v = r0 * x + r1 * y + r2 * z + offset + (q2 + q3 * l) * l * l;
            *value = v.clamp(0.0, 1.0);
        }
        out
    }
}

/// The reference gray (`#7a7b80`) the edge offsets and the tone checks are measured on.
pub const REFERENCE_GRAY: [f32; 3] = [122.0 / 255.0, 123.0 / 255.0, 128.0 / 255.0];

/// A profile's saturation and tint over a material color (the shader's, on the fill).
pub fn profile_color(profile: &GlassProfile, [r, g, b]: [f32; 3]) -> [f32; 3] {
    let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let saturate = |c: f32| l + (c - l) * profile.saturation;
    let [tr, tg, tb, amount] = profile.tint;
    let tint = |c: f32, t: f32| c + (t - c) * amount;
    [
        tint(saturate(r), tr),
        tint(saturate(g), tg),
        tint(saturate(b), tb),
    ]
}

/// How far the lens reaches inside the glass (the last point of the curves), in points: the
/// normal is computed on corners at least this wide, so that it does not jump on the diagonal
/// of a right angle.
fn lens_extent(lens: &Lens) -> f32 {
    [lens.side, lens.top, lens.bottom]
        .iter()
        .filter_map(|curve| curve.last().map(|(t, _)| *t))
        .fold(0.0, f32::max)
}

/// Glass quality levels: full (blurs everything behind), static frosted (only the desktop
/// wallpaper: recomputed rarely), opaque (the material over the desktop color, without blur).
/// For weak GPUs (plan, "Risks").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassQuality {
    Full,
    Frosted,
    Opaque,
}

impl GlassQuality {
    pub fn name(self) -> &'static str {
        match self {
            GlassQuality::Full => "full",
            GlassQuality::Frosted => "frosted",
            GlassQuality::Opaque => "opaque",
        }
    }
}

/// Notification glass, light appearance: banners and Notification Center. Blur σ 10–11 pt
/// without a sharp component; matrix of the Center's second notification, the most reliable
/// (rmse 3.5; the card on top of a stack has more offset and less gain). The offset measured
/// there (141, 139, 139) includes the Center's scrim: here it is recomputed so that over the
/// reference gray (`#7a7b80`) the fill is that of the isolated banner, `#c0c1c6`. Near the
/// top and bottom edges a slight horizontal magnification (~4 % at 5–6 pt), invisible under
/// this blur: no lens (notifications.md, "Banner").
pub const NOTIFICATION_LIGHT: MaterialParams = MaterialParams {
    sigma: 10.5,
    sharp: 0.03,
    matrix: [
        [0.632, -0.150, -0.096],
        [-0.052, 0.469, -0.023],
        [-0.132, -0.202, 0.731],
    ],
    offset: [145.6 / 255.0, 144.6 / 255.0, 145.3 / 255.0],
    curve: [[0.0; 3]; 2],
    lens: Lens {
        side: &[],
        top: &[],
        bottom: &[],
    },
    edge: NOTIFICATION_EDGE,
    // Cards separate even a few points apart (Notification Center).
    merge_pt: 0.0,
};

/// Notification glass, dark: σ 10.75–11 pt (10.9 is used), w 0.02, matrix of the second
/// notification (rmse 7.43). As in light the measured offset includes the Center's scrim: here
/// it is recomputed so that over gray the fill is that of the isolated banner, `#75767b`
/// (notifications.md, "Color matrix" and "Center"). The dark edge is not measured: as in
/// light.
pub const NOTIFICATION_DARK: MaterialParams = MaterialParams {
    sigma: 10.9,
    sharp: 0.02,
    matrix: [
        [0.757, -0.256, -0.091],
        [-0.111, 0.512, 0.012],
        [-0.148, -0.297, 0.857],
    ],
    offset: [67.8 / 255.0, 67.0 / 255.0, 67.9 / 255.0],
    ..NOTIFICATION_LIGHT
};

/// Gap between two regions of the same material below which they join into a single shape,
/// like drops (smooth union of the distances, k = 2 · MERGE_PT: two edges at distance g touch
/// when g < k / 2). Measured limits: the search panel's round buttons 10 pt from the panel stay
/// separate (spotlight.md) and the Control Center modules, whose glasses are 10 pt apart
/// horizontally (the side edge sticks out by 1 pt), have no bridge or bulge on the edge
/// (controlcenter.md): k ≤ 10, that is at most 5. The exact value is not measured
/// (plan/reference/RICHIESTE.md, G-02).
pub const MERGE_PT: f32 = 5.0;
/// Regions the shader merges together at most: beyond that, separate groups.
const MAX_SHAPES: usize = 8;

/// Beyond this σ (pixels) blurring happens at half or a quarter of the resolution: the shader
/// kernel reaches 48 pixels.
const MAX_SIGMA_PX: f64 = 12.0;

/// Where a tip points: its apex at the middle of that side of its rectangle, its base on the
/// opposite one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tip {
    Down,
    Up,
    Left,
    Right,
}

/// The outline of a tip, measured in TIP-01 (points). Not a triangle: two straight flanks at
/// `angle` from the body edge, whose sharp corners would sit `half_base` from the axis; a round
/// apex of `apex_radius` tangent to both flanks; concave fillets of `fillet_radius` tangent to
/// the body edge and to each flank. It is symmetric and fixed in points (a larger popover has
/// the same tip). The fit is within ~0.2 px of the reference outline at 1×.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TipProfile {
    pub half_base: f64,
    /// Flank angle from the body edge, in radians.
    pub angle: f64,
    pub apex_radius: f64,
    pub fillet_radius: f64,
}

/// A measured tip (TIP-01, points and degrees).
struct TipPreset {
    depth: f64,
    angle_deg: f64,
    apex_radius: f64,
    fillet_radius: f64,
}

/// The Dock icon menu's tip: 10.19 deep, flanks at 50.8°, apex radius 5.30, fillets 9.41
/// (15.29 from the axis to where the outline leaves the body edge).
const DOCK_MENU_TIP: TipPreset = TipPreset {
    depth: 10.19,
    angle_deg: 50.8,
    apex_radius: 5.30,
    fillet_radius: 9.41,
};
/// A popover's tip on its top or bottom edge (mean of the two): 10.8 deep, flanks at 46.2°,
/// apex radius 5.58, fillets 8.0.
const POPOVER_TIP: TipPreset = TipPreset {
    depth: 10.80,
    angle_deg: 46.2,
    apex_radius: 5.58,
    fillet_radius: 8.0,
};
/// A popover's tip on a side edge: one point deeper with longer fillets, the same flanks and
/// apex.
const POPOVER_SIDE_TIP: TipPreset = TipPreset {
    depth: 11.74,
    angle_deg: 46.1,
    apex_radius: 5.73,
    fillet_radius: 13.6,
};

impl TipPreset {
    fn profile(&self) -> TipProfile {
        let angle = self.angle_deg.to_radians();
        TipProfile {
            half_base: TipProfile::half_base_for(angle, self.apex_radius, self.depth),
            angle,
            apex_radius: self.apex_radius,
            fillet_radius: self.fillet_radius,
        }
    }
}

impl TipProfile {
    pub fn dock_menu() -> Self {
        DOCK_MENU_TIP.profile()
    }

    pub fn popover() -> Self {
        POPOVER_TIP.profile()
    }

    pub fn popover_side() -> Self {
        POPOVER_SIDE_TIP.profile()
    }

    /// The half base that gives `depth`: the sharp apex sits half_base · tan(angle) from the
    /// edge, and the round apex takes back apex_radius · (1 / cos(angle) − 1).
    fn half_base_for(angle: f64, apex_radius: f64, depth: f64) -> f64 {
        (depth + apex_radius * (1.0 / angle.cos() - 1.0)) / angle.tan()
    }

    /// Depth from the body edge to the apex.
    pub fn depth(&self) -> f64 {
        self.half_base * self.angle.tan() - self.apex_radius * (1.0 / self.angle.cos() - 1.0)
    }

    /// From the axis to where the outline leaves the body edge.
    pub fn half_width(&self) -> f64 {
        self.half_base + self.fillet_radius * (self.angle / 2.0).tan()
    }

    /// The tip drawn in a rectangle of `width` along the body edge and `depth` away from it
    /// (points): the measured preset closest to that size keeps its flank angle and apex,
    /// the half base gives the depth and the fillets give the width. The measured sizes give
    /// the measured outlines.
    pub fn fit(width: f64, depth: f64) -> Self {
        let half = width / 2.0;
        // The side does not choose: a size measured on one edge gives the same outline on
        // every edge (rotated), so the closest measured size wins.
        let off = |p: &Self| (p.half_width() - half).abs() + (p.depth() - depth).abs();
        let preset = [Self::dock_menu(), Self::popover(), Self::popover_side()]
            .into_iter()
            .fold(None::<Self>, |best, p| match best {
                Some(b) if off(&b) <= off(&p) => Some(b),
                _ => Some(p),
            })
            .unwrap_or_else(Self::dock_menu);
        // Protocol rectangles are whole points: a size that rounds the measured one draws the
        // measured outline itself (it may pass the rectangle by a fraction of a point).
        if (preset.depth() - depth).abs() <= 0.5 && (preset.half_width() - half).abs() <= 0.5 {
            return preset;
        }
        let depth = depth.max(1.0);
        let half = half.max(0.5);
        // The apex cannot be rounder than the tip is deep or wide.
        let apex_radius = preset.apex_radius.min(depth * 0.8).min(half * 0.8);
        let mut angle = preset.angle;
        let mut half_base = Self::half_base_for(angle, apex_radius, depth);
        if half_base > half {
            // Narrower than the measured flanks allow: steeper flanks from the rectangle's
            // corners, still reaching `depth`.
            half_base = half;
            angle = Self::angle_for(half_base, apex_radius, depth);
        }
        let fillet_radius = ((half - half_base) / (angle / 2.0).tan()).max(0.0);
        Self {
            half_base,
            angle,
            apex_radius,
            fillet_radius,
        }
    }

    /// The flank angle that gives `depth` from a given half base (the depth grows with the
    /// angle while the apex radius stays under the half base): bisection.
    fn angle_for(half_base: f64, apex_radius: f64, depth: f64) -> f64 {
        let reach = |a: f64| half_base * a.tan() - apex_radius * (1.0 / a.cos() - 1.0);
        let (mut low, mut high) = (0.01_f64, 89.0_f64.to_radians());
        for _ in 0..60 {
            let mid = 0.5 * (low + high);
            if reach(mid) < depth {
                low = mid;
            } else {
                high = mid;
            }
        }
        0.5 * (low + high)
    }

    /// For the shader, in physical pixels: half base, tan(angle), apex and fillet radii.
    fn uniform(&self, scale: f64) -> [f32; 4] {
        [
            (self.half_base * scale) as f32,
            self.angle.tan() as f32,
            (self.apex_radius * scale) as f32,
            (self.fillet_radius * scale) as f32,
        ]
    }
}

/// A glass region in the output, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalRegion {
    pub rect: Rectangle<i32, Physical>,
    pub radius: f64,
    pub material: MaterialParams,
    /// Compositing opacity (0–1): it does not change the blurred background, only the drawing.
    pub opacity: f32,
    /// Physical pixels per point.
    pub scale: f64,
    /// A tip instead of a rectangle (`add_tip`, `add_upward_tip`, `add_side_tip`).
    pub tip: Option<Tip>,
    /// Regions merge only within the same group.
    pub group: u32,
    /// The kind of surface: blur, lens, edge, tint and shadow over the material.
    pub profile: GlassProfile,
}

impl PhysicalRegion {
    /// σ of the blur in points: the profile's, or the material's.
    pub fn sigma(&self) -> f64 {
        self.profile.sigma.unwrap_or(self.material.sigma)
    }

    /// What the blurred texture depends on.
    fn blur_key(&self) -> BlurKey {
        BlurKey {
            sigma: self.sigma(),
            sharp: self.material.sharp,
        }
    }

    /// How far the region draws beyond its shape, in pixels: the merging bulge and the shadow.
    fn bleed(&self) -> i32 {
        let bulge = f64::from(self.material.merge_pt) * 0.5;
        let shadow = f64::from(self.profile.shadow.reach());
        ((bulge + shadow) * self.scale).ceil() as i32 + 1
    }

    /// Same shape and same material: the blurred background can be reused even if the
    /// opacity changed (a fade does not redo the blur on every frame).
    fn same_shape(&self, other: &Self) -> bool {
        self.rect == other.rect
            && self.radius == other.radius
            && self.material == other.material
            && self.scale == other.scale
            && self.tip == other.tip
            && self.group == other.group
            && self.profile == other.profile
    }
}

/// The blur of a texture: σ in points and the sharp share.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BlurKey {
    sigma: f64,
    sharp: f32,
}

/// The state of the elements behind the glass: if it does not change, the texture is reused.
type Signature = Vec<(Id, CommitCounter, Rectangle<i32, Physical>)>;

/// The glass of a surface between frames (in the surface data).
#[derive(Default)]
pub struct GlassCache {
    state: RefCell<Option<Cached>>,
}

struct Cached {
    quality: GlassQuality,
    signature: Signature,
    regions: Vec<PhysicalRegion>,
    bbox: Rectangle<i32, Physical>,
    /// One blurred texture per blur present, in the order of the regions.
    textures: Vec<(BlurKey, GlesTexture)>,
    ids: Vec<Id>,
    commit: CommitCounter,
}

/// The glass programs.
#[derive(Clone)]
pub struct GlassPrograms {
    pub blur: GlesTexProgram,
    pub material: GlesTexProgram,
}

impl GlassCache {
    /// The glass elements for these regions, given what is behind (`behind`, from the front
    /// to the back) and its fixed part (`behind_static`: the desktop wallpaper, for the
    /// frosted level). Error if the renderer cannot draw offscreen: the caller skips the glass
    /// for this frame.
    #[allow(clippy::too_many_arguments)]
    pub fn elements<E: RenderElement<GlesRenderer>>(
        &self,
        renderer: &mut GlesRenderer,
        programs: &GlassPrograms,
        regions: &[PhysicalRegion],
        behind: &[E],
        behind_static: &[E],
        quality: GlassQuality,
        output: Rectangle<i32, Physical>,
        scale: f64,
        clear_color: [f32; 4],
    ) -> Result<Vec<GlassElement>, GlesError> {
        let behind = match quality {
            GlassQuality::Full => behind,
            GlassQuality::Frosted => behind_static,
            GlassQuality::Opaque => behind.get(..0).unwrap_or_default(),
        };
        let margin = regions
            .iter()
            .map(|region| {
                let reach = f64::from(
                    region.material.lens.outward_reach()
                        + region.material.merge_pt
                        + region.profile.shadow.reach(),
                );
                ((region.sigma() * 3.0 + reach) * scale).ceil() as i32 + 2
            })
            .max()
            .unwrap_or(0);
        let Some(bbox) = regions
            .iter()
            .map(|region| region.rect)
            .reduce(|a, b| a.merge(b))
            .map(|rect| {
                Rectangle::new(
                    rect.loc - Point::from((margin, margin)),
                    rect.size + Size::from((margin.saturating_mul(2), margin.saturating_mul(2))),
                )
            })
            .and_then(|rect| rect.intersection(output))
        else {
            return Ok(Vec::new());
        };
        let signature: Signature = behind
            .iter()
            .filter(|element| element.geometry(Scale::from(scale)).overlaps(bbox))
            .map(|element| {
                (
                    element.id().clone(),
                    element.current_commit(),
                    element.geometry(Scale::from(scale)),
                )
            })
            .collect();

        let mut state = self.state.borrow_mut();
        let reuse = state.as_ref().is_some_and(|cached| {
            cached.quality == quality
                && cached.signature == signature
                && cached.regions.len() == regions.len()
                && cached
                    .regions
                    .iter()
                    .zip(regions)
                    .all(|(a, b)| a.same_shape(b))
                && cached.bbox == bbox
        });
        // Only the opacity changed: same textures, new elements to redraw.
        if reuse
            && let Some(cached) = state.as_mut()
            && cached.regions != regions
        {
            cached.regions = regions.to_vec();
            cached.commit.increment();
        }
        if !reuse {
            let sharp = render_to_texture(renderer, behind, bbox, scale, clear_color)?;
            let mut textures: Vec<(BlurKey, GlesTexture)> = Vec::new();
            for region in regions {
                let key = region.blur_key();
                if textures.iter().any(|(k, _)| *k == key) {
                    continue;
                }
                let blurred = blur(renderer, programs, &sharp, bbox.size, key, scale)?;
                textures.push((key, blurred));
            }
            let count = groups(regions).len();
            let (ids, mut commit) = match state.take() {
                Some(cached) if cached.ids.len() == count => (cached.ids, cached.commit),
                Some(cached) => ((0..count).map(|_| Id::new()).collect(), cached.commit),
                None => (
                    (0..count).map(|_| Id::new()).collect(),
                    CommitCounter::default(),
                ),
            };
            commit.increment();
            *state = Some(Cached {
                quality,
                signature,
                regions: regions.to_vec(),
                bbox,
                textures,
                ids,
                commit,
            });
        }
        let Some(cached) = state.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(groups(&cached.regions)
            .into_iter()
            .zip(&cached.ids)
            .filter_map(|(shapes, id)| {
                let key = shapes.first()?.blur_key();
                let texture = cached.textures.iter().find(|(k, _)| *k == key)?.1.clone();
                // The smooth union goes out of the regions by at most k / 4 = merge_pt / 2,
                // the shadow by its reach.
                let bulge = shapes.first()?.bleed();
                let area = shapes
                    .iter()
                    .map(|shape| shape.rect)
                    .reduce(|a, b| a.merge(b))?;
                let area = Rectangle::new(
                    area.loc - Point::from((bulge, bulge)),
                    area.size + Size::from((bulge.saturating_mul(2), bulge.saturating_mul(2))),
                )
                .intersection(cached.bbox)?;
                Some(GlassElement {
                    id: id.clone(),
                    commit: cached.commit,
                    texture,
                    bbox: cached.bbox,
                    area,
                    shapes,
                    program: programs.material.clone(),
                })
            })
            .collect())
    }
}

/// The regions grouped by material, profile and client group (in the order they appear), at most
/// `MAX_SHAPES` per group: the regions of a group merge with each other.
fn groups(regions: &[PhysicalRegion]) -> Vec<Vec<PhysicalRegion>> {
    let mut groups: Vec<Vec<PhysicalRegion>> = Vec::new();
    for region in regions {
        match groups.iter_mut().find(|group| {
            group.len() < MAX_SHAPES
                && group.first().is_some_and(|first| {
                    first.material == region.material
                        && first.profile == region.profile
                        && first.opacity == region.opacity
                        && first.group == region.group
                })
        }) {
            Some(group) => group.push(*region),
            None => groups.push(vec![*region]),
        }
    }
    groups
}

fn buffer_size(size: Size<i32, Physical>) -> Size<i32, Buffer> {
    size.to_logical(1).to_buffer(1, Transform::Normal)
}

/// Draws `elements` (from the front to the back) over `clear_color` into a new texture
/// covering `bbox` of the output: the glass background, the snapshot of a closing
/// window.
pub fn render_to_texture<E: RenderElement<GlesRenderer>>(
    renderer: &mut GlesRenderer,
    elements: &[E],
    bbox: Rectangle<i32, Physical>,
    scale: f64,
    clear_color: [f32; 4],
) -> Result<GlesTexture, GlesError> {
    let mut texture = renderer.create_buffer(Fourcc::Abgr8888, buffer_size(bbox.size))?;
    {
        let mut target = renderer.bind(&mut texture)?;
        let mut frame = renderer.render(&mut target, bbox.size, Transform::Normal)?;
        frame.clear(clear_color.into(), &[Rectangle::from_size(bbox.size)])?;
        for element in elements.iter().rev() {
            let geometry = element.geometry(Scale::from(scale));
            let Some(visible) = geometry.intersection(bbox) else {
                continue;
            };
            let dst = Rectangle::new(geometry.loc - bbox.loc, geometry.size);
            let damage = [Rectangle::new(visible.loc - geometry.loc, visible.size)];
            element.draw(&mut frame, element.src(), dst, &damage, &[])?;
        }
        // Same GL context for whoever uses the texture: the order of the commands is enough.
        let _sync = frame.finish()?;
    }
    Ok(texture)
}

/// One pass: `source` (`from` large) drawn into a new texture `to` large, over `under` if
/// present.
#[allow(clippy::too_many_arguments)]
fn pass(
    renderer: &mut GlesRenderer,
    source: &GlesTexture,
    from: Size<i32, Physical>,
    to: Size<i32, Physical>,
    program: Option<&GlesTexProgram>,
    uniforms: &[Uniform<'_>],
    alpha: f32,
    under: Option<&GlesTexture>,
) -> Result<GlesTexture, GlesError> {
    let mut texture = renderer.create_buffer(Fourcc::Abgr8888, buffer_size(to))?;
    {
        let mut target = renderer.bind(&mut texture)?;
        let mut frame = renderer.render(&mut target, to, Transform::Normal)?;
        let full = [Rectangle::from_size(to)];
        frame.clear([0.0, 0.0, 0.0, 0.0].into(), &full)?;
        let src = Rectangle::from_size(buffer_size(from).to_f64());
        if let Some(under) = under {
            frame.render_texture_from_to(
                under,
                src,
                Rectangle::from_size(to),
                &full,
                &[],
                Transform::Normal,
                1.0,
                None,
                &[],
            )?;
        }
        frame.render_texture_from_to(
            source,
            src,
            Rectangle::from_size(to),
            &full,
            &[],
            Transform::Normal,
            alpha,
            program,
            uniforms,
        )?;
        // Same GL context for whoever uses the texture: the order of the commands is enough.
        let _sync = frame.finish()?;
    }
    Ok(texture)
}

/// Blurs the capture with the σ and mixes in the sharp share.
fn blur(
    renderer: &mut GlesRenderer,
    programs: &GlassPrograms,
    sharp: &GlesTexture,
    size: Size<i32, Physical>,
    key: BlurKey,
    scale: f64,
) -> Result<GlesTexture, GlesError> {
    let sigma = key.sigma * scale;
    // Reduction: with a large σ blurring happens at half or a quarter of the resolution.
    let factor = if sigma > MAX_SIGMA_PX * 2.0 {
        4
    } else if sigma > MAX_SIGMA_PX {
        2
    } else {
        1
    };
    let small = Size::from(((size.w / factor).max(1), (size.h / factor).max(1)));
    let mut current = if factor > 1 {
        pass(renderer, sharp, size, small, None, &[], 1.0, None)?
    } else {
        sharp.clone()
    };
    let sigma_small = (sigma / f64::from(factor)) as f32;
    for step in [(1.0 / small.w as f32, 0.0), (0.0, 1.0 / small.h as f32)] {
        let uniforms = [
            Uniform::new("step", UniformValue::_2f(step.0, step.1)),
            Uniform::new("sigma", UniformValue::_1f(sigma_small)),
        ];
        current = pass(
            renderer,
            &current,
            small,
            small,
            Some(&programs.blur),
            &uniforms,
            1.0,
            None,
        )?;
    }
    // Back to full resolution, with the sharp share below: w · sharp + (1 − w) · blurred.
    pass(
        renderer,
        &current,
        small,
        size,
        None,
        &[],
        1.0 - key.sharp,
        Some(sharp),
    )
}

/// A glass region ready to draw.
#[derive(Debug, Clone)]
pub struct GlassElement {
    id: Id,
    commit: CommitCounter,
    /// The blurred background, as large as `bbox`.
    texture: GlesTexture,
    bbox: Rectangle<i32, Physical>,
    /// Where it is drawn: the regions of the group plus the merging room.
    area: Rectangle<i32, Physical>,
    /// The regions of the group (same material, at most `MAX_SHAPES`).
    shapes: Vec<PhysicalRegion>,
    program: GlesTexProgram,
}

impl Element for GlassElement {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        let local = Rectangle::new(self.area.loc - self.bbox.loc, self.area.size);
        Rectangle::new(
            Point::from((f64::from(local.loc.x), f64::from(local.loc.y))),
            buffer_size(local.size).to_f64(),
        )
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.area
    }

    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for GlassElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        let Some(first) = self.shapes.first() else {
            return Ok(());
        };
        let m = first.material.matrix;
        let b = first.material.offset;
        let [q2, q3] = first.material.curve;
        let lens = first.material.lens;
        let edge = first.material.edge;
        let profile = first.profile;
        let px = first.scale as f32;
        // The deepest the lens samples, for its width on each shape (`lens_width`).
        let lens_reach_px = lens.inward_reach() * px;
        let mut lens_widths = [profile.falloff; 16];
        // The regions relative to the texture: up to 8 rectangles in two mat4 (one column per
        // rectangle: x, y, width, height) and the radii in a mat4.
        let mut rects = [[0.0_f32; 16]; 2];
        let mut radii = [0.0_f32; 16];
        // The tips' outlines (TIP-01), one column per region: half base, tan(angle), apex and
        // fillet radii in pixels.
        let mut tips = [[0.0_f32; 16]; 2];
        for (i, shape) in self.shapes.iter().take(MAX_SHAPES).enumerate() {
            let local = shape.rect.loc - self.bbox.loc;
            let values = [
                local.x as f32,
                local.y as f32,
                shape.rect.size.w as f32,
                shape.rect.size.h as f32,
            ];
            if let Some(slot) = rects
                .get_mut(i / 4)
                .and_then(|matrix| matrix.get_mut((i % 4) * 4..(i % 4) * 4 + 4))
            {
                slot.copy_from_slice(&values);
            }
            // Negative radius: a tip (the shader recognizes it by the sign): −1 points down,
            // −2 up, −3 left, −4 right.
            if let Some(tip) = shape.tip {
                let (along, away) = match tip {
                    Tip::Down | Tip::Up => (shape.rect.size.w, shape.rect.size.h),
                    Tip::Left | Tip::Right => (shape.rect.size.h, shape.rect.size.w),
                };
                let scale = shape.scale.max(0.01);
                let profile = TipProfile::fit(f64::from(along) / scale, f64::from(away) / scale);
                if let Some(slot) = tips
                    .get_mut(i / 4)
                    .and_then(|matrix| matrix.get_mut((i % 4) * 4..(i % 4) * 4 + 4))
                {
                    slot.copy_from_slice(&profile.uniform(scale));
                }
            }
            let thickness = shape.rect.size.w.min(shape.rect.size.h) as f32;
            if let Some(slot) = lens_widths.get_mut(i) {
                *slot = lens_width(thickness, lens_reach_px, profile.falloff);
            }
            if let Some(slot) = radii.get_mut(i) {
                *slot = match shape.tip {
                    None => shape.radius as f32,
                    Some(Tip::Down) => -1.0,
                    Some(Tip::Up) => -2.0,
                    Some(Tip::Left) => -3.0,
                    Some(Tip::Right) => -4.0,
                };
            }
        }
        let uniforms = [
            Uniform::new(
                "tex_size",
                UniformValue::_2f(self.bbox.size.w as f32, self.bbox.size.h as f32),
            ),
            Uniform::new(
                "shapes",
                UniformValue::Matrix4x4 {
                    matrices: rects.to_vec(),
                    transpose: false,
                },
            ),
            Uniform::new(
                "radii",
                UniformValue::Matrix4x4 {
                    matrices: vec![radii],
                    transpose: false,
                },
            ),
            Uniform::new(
                "tips",
                UniformValue::Matrix4x4 {
                    matrices: tips.to_vec(),
                    transpose: false,
                },
            ),
            Uniform::new(
                "shape_count",
                UniformValue::_1f(self.shapes.len().min(MAX_SHAPES) as f32),
            ),
            Uniform::new(
                "merge",
                UniformValue::_1f(2.0 * first.material.merge_pt * px),
            ),
            Uniform::new("exponent", UniformValue::_1f(2.0)),
            Uniform::new("row_r", UniformValue::_3f(m[0][0], m[0][1], m[0][2])),
            Uniform::new("row_g", UniformValue::_3f(m[1][0], m[1][1], m[1][2])),
            Uniform::new("row_b", UniformValue::_3f(m[2][0], m[2][1], m[2][2])),
            Uniform::new("offset", UniformValue::_3f(b[0], b[1], b[2])),
            Uniform::new("curve2", UniformValue::_3f(q2[0], q2[1], q2[2])),
            Uniform::new("curve3", UniformValue::_3f(q3[0], q3[1], q3[2])),
            Uniform::new("scale_px", UniformValue::_1f(px)),
            Uniform::new("lens", lens.uniform(px)),
            Uniform::new("lens_reach", UniformValue::_1f(lens_extent(&lens) * px)),
            Uniform::new("edges", edge.uniform()),
            Uniform::new(
                "fill_gray",
                UniformValue::_1f(first.material.fill_on_gray_with(&profile)),
            ),
            Uniform::new("edge_power", UniformValue::_1f(EDGE_POWER)),
            Uniform::new(
                "lens_widths",
                UniformValue::Matrix4x4 {
                    matrices: vec![lens_widths],
                    transpose: false,
                },
            ),
            Uniform::new("refraction", UniformValue::_1f(profile.refraction)),
            Uniform::new("rim", UniformValue::_1f(profile.rim)),
            Uniform::new("line", UniformValue::_1f(profile.line)),
            Uniform::new("saturation", UniformValue::_1f(profile.saturation)),
            Uniform::new(
                "tint_color",
                UniformValue::_4f(
                    profile.tint[0],
                    profile.tint[1],
                    profile.tint[2],
                    profile.tint[3],
                ),
            ),
            Uniform::new(
                "shadow",
                UniformValue::_3f(
                    profile.shadow.opacity,
                    profile.shadow.sigma * px,
                    profile.shadow.offset_y * px,
                ),
            ),
        ];
        frame.render_texture_from_to(
            &self.texture,
            src,
            dst,
            damage,
            opaque_regions,
            Transform::Normal,
            first.opacity.clamp(0.0, 1.0),
            Some(&self.program),
            &uniforms,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn the_side_edge_fades_around_a_capsule_end_like_the_modules() {
        // Angle from the side (0°) towards the top (90°), outward normal (−cos, −sin) on a
        // left end: the measured dark line ~0.7 at 15°, ~0.5 at 30°, ≤ 0.2 at 45°.
        let side_at = |deg: f32| {
            let a = deg.to_radians();
            edge_weights((-a.cos(), -a.sin()))[1]
        };
        assert!((side_at(0.0) - 1.0).abs() < 1e-6);
        assert!((0.75..0.9).contains(&side_at(15.0)), "{}", side_at(15.0));
        assert!((0.4..0.6).contains(&side_at(30.0)), "{}", side_at(30.0));
        assert!(side_at(45.0) < 0.2, "{}", side_at(45.0));
        assert!(side_at(60.0) < 0.05, "{}", side_at(60.0));
        // The top row has the same falloff towards the side.
        let top_at = |deg: f32| {
            let a = deg.to_radians();
            edge_weights((-a.cos(), -a.sin()))[0]
        };
        assert!((top_at(60.0) - side_at(30.0)).abs() < 1e-6);
        // The weights and the shared part make one, on every direction.
        for deg in (0..360).step_by(5) {
            let a = (deg as f32).to_radians();
            let w = edge_weights((a.cos(), a.sin()));
            let total: f32 = w.iter().sum();
            assert!((total - 1.0).abs() < 1e-5, "{deg}°: {w:?}");
            assert!(w.iter().all(|v| (0.0..=1.0).contains(v)), "{deg}°: {w:?}");
        }
        // A degenerate normal (the middle of a shape) counts as the top.
        assert_eq!(edge_weights((0.0, 0.0))[0], 1.0);
    }

    #[test]
    fn the_presets_give_the_measured_tips() {
        // TIP-01: depth and half width of the three outlines.
        for (profile, depth, half_width) in [
            (TipProfile::dock_menu(), 10.19, 15.29),
            (TipProfile::popover(), 10.80, 16.15),
            (TipProfile::popover_side(), 11.74, 19.45),
        ] {
            assert!(close(profile.depth(), depth, 0.01), "{profile:?}");
            assert!(close(profile.half_width(), half_width, 0.2), "{profile:?}");
        }
    }

    #[test]
    fn a_measured_size_gives_its_preset_back() {
        let dock = TipProfile::dock_menu();
        let fitted = TipProfile::fit(2.0 * dock.half_width(), dock.depth());
        assert!(close(fitted.half_base, dock.half_base, 1e-6));
        assert!(close(fitted.fillet_radius, dock.fillet_radius, 1e-6));
        let side = TipProfile::popover_side();
        let fitted = TipProfile::fit(2.0 * side.half_width(), side.depth());
        assert!(close(fitted.angle, side.angle, 1e-9));
        // A popover-sized tip on a top edge takes the popover's flanks, a Dock menu's the
        // Dock's.
        assert!(close(
            TipProfile::fit(32.0, 11.0).angle,
            TipProfile::popover().angle,
            1e-9
        ));
        assert!(close(TipProfile::fit(31.0, 10.0).angle, dock.angle, 1e-9));
        // Whole-point sizes that round a measured tip draw it exactly.
        assert_eq!(TipProfile::fit(31.0, 10.0), dock);
        assert_eq!(TipProfile::fit(32.0, 11.0), TipProfile::popover());
        assert_eq!(TipProfile::fit(39.0, 12.0), side);
        // Others are fitted: exactly as deep as the rectangle.
        assert!(close(TipProfile::fit(36.0, 14.0).depth(), 14.0, 1e-6));
    }

    #[test]
    fn any_rectangle_gives_a_tip_inside_it() {
        for (w, d) in [
            (24.0, 10.0),
            (31.0, 10.0),
            (8.0, 10.0),
            (40.0, 3.0),
            (39.0, 12.0),
        ] {
            let p = TipProfile::fit(w, d);
            assert!(
                p.fillet_radius >= 0.0 && p.apex_radius <= d,
                "{w}×{d}: {p:?}"
            );
            // Exact, or the measured outline that the whole-point size rounds.
            assert!(close(p.depth(), d, 0.5), "{w}×{d}: depth {}", p.depth());
            assert!(p.half_width() <= w / 2.0 + 0.5 + 1e-6, "{w}×{d}: {p:?}");
        }
    }
}
