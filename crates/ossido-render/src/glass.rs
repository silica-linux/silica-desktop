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

/// Dock lens: on the sides up to ~5 pt inwards at 6 pt from the edge, nothing beyond
/// ~22 pt; on top ≤ 1 pt, at the bottom ~3 pt (glass.md, "Glass · Dock · Refraction").
const DOCK_LENS: Lens = Lens {
    side: &[
        (0.0, 0.0),
        (6.0, 11.0),
        (10.0, 14.3),
        (14.0, 16.5),
        (18.0, 18.7),
        (22.0, 22.0),
    ],
    top: &[
        (0.0, 0.0),
        (6.0, 7.0),
        (10.0, 10.9),
        (14.0, 14.5),
        (18.0, 18.1),
        (22.0, 22.0),
    ],
    bottom: &[
        (0.0, 0.0),
        (6.0, 9.0),
        (10.0, 12.6),
        (14.0, 15.5),
        (18.0, 18.4),
        (22.0, 22.0),
    ],
};

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

/// The lens of the regular glass, measured on the Control Center modules from the displacement maps
/// with the edge's own light taken out (the gray capture subtracted: a glow of 10 levels
/// would read as ~20 px; `xtask/tools/glass_parity.py`). Near the edge the glass shows the
/// background from well inside, folded: at 2 pt from the top or bottom edge the background
/// ~43 pt inwards, at 6 pt ~23 pt, flattening to ~18 pt around 10 pt, then nothing past
/// ~24 pt. The sides fold less (~34 pt at 2 pt). The same in pixels on the 64 pt and the
/// 140 pt modules: on the narrow ones the fold reaches past the middle. The point at 0 is
/// extrapolated (the rim covers it). glass.md's first measure (the outer band sampling the
/// background outside) read the glow as displacement.
const LIQUID_LENS: Lens = Lens {
    side: &[
        (0.0, 42.0),
        (2.0, 34.0),
        (4.0, 25.0),
        (6.0, 18.5),
        (8.0, 15.3),
        (11.0, 12.5),
        (16.0, 16.3),
        (22.0, 22.0),
    ],
    top: &[
        (0.0, 50.0),
        (2.0, 43.0),
        (4.0, 33.0),
        (6.0, 23.5),
        (8.0, 20.5),
        (12.0, 18.0),
        (18.0, 19.5),
        (24.0, 24.0),
    ],
    bottom: &[
        (0.0, 50.0),
        (2.0, 43.0),
        (4.0, 33.0),
        (6.0, 23.5),
        (8.0, 20.5),
        (12.0, 18.0),
        (18.0, 19.5),
        (24.0, 24.0),
    ],
};

/// Menus, light: σ ≈ 9 pt, negligible sharp component. The transfer flattens towards black
/// and white: the ramps, which only span the middle levels, want a steeper matrix than the
/// stripes. Matrix with b = (158.3, 158.3, 158.4) and quadratic and cubic terms, fitted on the
/// sRGB-converted gray, ramp and stripe captures together (`xtask/tools/glass_parity.py`;
/// rmse 2.62; 2.95 with a matrix alone, 3.15 with menubar.md's).
pub const THICK_LIGHT: MaterialParams = MaterialParams {
    sigma: 9.0,
    sharp: 0.0,
    matrix: [
        [0.644, -0.333, -0.186],
        [-0.223, 0.417, -0.109],
        [-0.322, -0.423, 0.825],
    ],
    offset: [158.3 / 255.0, 158.3 / 255.0, 158.4 / 255.0],
    curve: [[0.739, 0.866, 0.889], [-0.548, -0.643, -0.663]],
    // In menus the blur hides the refraction: ≤ 1–2 pt on the sides.
    lens: Lens {
        side: &[
            (0.0, 0.0),
            (6.0, 7.5),
            (10.0, 11.3),
            (14.0, 14.75),
            (18.0, 18.2),
            (22.0, 22.0),
        ],
        top: &[],
        bottom: &[],
    },
    edge: MENU_EDGE,
    merge_pt: MERGE_PT,
};

/// Menus, dark: σ ≈ 9 pt as in light (8.7 from the stripes). The dark transfer bends: black
/// gives ~56, the gray `#7a7b80` the measured `#6f7075`, white only ~127 (the stripes'
/// plateaus). The chroma of menubar.md's dark matrix ("Color") with its luminance gain raised
/// from 110 to 152 levels, b = (55.5, 55.8, 56.4) and a quadratic term of −80 levels, fitted
/// on the sRGB-converted gray and stripe captures (`xtask/tools/glass_parity.py`; rmse 2.2,
/// 6.5 with a matrix alone). The ramp captures of the series have windows behind the menu.
/// Edge as in light (the dark menu's rim is not measured in the model's form).
pub const THICK_DARK: MaterialParams = MaterialParams {
    matrix: [
        [0.946, -0.292, -0.058],
        [-0.095, 0.635, 0.055],
        [-0.122, -0.336, 1.056],
    ],
    offset: [55.5 / 255.0, 55.8 / 255.0, 56.4 / 255.0],
    curve: [[-0.315, -0.315, -0.313], [0.0; 3]],
    edge: MENU_DARK_EDGE,
    ..THICK_LIGHT
};

pub const BLUR_SHADER: &str = include_str!("shaders/blur.frag");
pub const BLUR_UNIFORMS: [(&str, UniformType); 2] =
    [("step", UniformType::_2f), ("sigma", UniformType::_1f)];
pub const MATERIAL_SHADER: &str = include_str!("shaders/material.frag");
pub const MATERIAL_UNIFORMS: [(&str, UniformType); 17] = [
    ("tex_size", UniformType::_2f),
    ("shapes", UniformType::Matrix4x4),
    ("radii", UniformType::Matrix4x4),
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
];

impl MaterialParams {
    /// The fill over the reference gray (`#7a7b80`), 0–1 luminance: the level the edge
    /// offsets were measured against.
    pub fn fill_on_gray(&self) -> f32 {
        let gray = [122.0 / 255.0, 123.0 / 255.0, 128.0 / 255.0];
        let l = (gray[0] + gray[1] + gray[2]) / 3.0;
        let [q2, q3] = self.curve;
        let channel = |row: [f32; 3], offset: f32, q2: f32, q3: f32| {
            row[0] * gray[0] + row[1] * gray[1] + row[2] * gray[2] + offset + (q2 + q3 * l) * l * l
        };
        let r = channel(self.matrix[0], self.offset[0], q2[0], q3[0]);
        let g = channel(self.matrix[1], self.offset[1], q2[1], q3[1]);
        let b = channel(self.matrix[2], self.offset[2], q2[2], q3[2]);
        ((r + g + b) / 3.0).clamp(0.01, 0.99)
    }
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
}

impl PhysicalRegion {
    /// Same shape and same material: the blurred background can be reused even if the
    /// opacity changed (a fade does not redo the blur on every frame).
    fn same_shape(&self, other: &Self) -> bool {
        self.rect == other.rect
            && self.radius == other.radius
            && self.material == other.material
            && self.scale == other.scale
            && self.tip == other.tip
            && self.group == other.group
    }
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
    /// One blurred texture per material present, in the order of the regions.
    textures: Vec<(MaterialParams, GlesTexture)>,
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
                let reach =
                    f64::from(region.material.lens.outward_reach() + region.material.merge_pt);
                ((region.material.sigma * 3.0 + reach) * scale).ceil() as i32 + 2
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
            let mut textures: Vec<(MaterialParams, GlesTexture)> = Vec::new();
            for region in regions {
                if textures
                    .iter()
                    .any(|(material, _)| *material == region.material)
                {
                    continue;
                }
                let blurred = blur(
                    renderer,
                    programs,
                    &sharp,
                    bbox.size,
                    region.material,
                    scale,
                )?;
                textures.push((region.material, blurred));
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
                let material = shapes.first()?.material;
                let texture = cached
                    .textures
                    .iter()
                    .find(|(m, _)| *m == material)?
                    .1
                    .clone();
                let scale = shapes.first()?.scale;
                // The smooth union goes out of the regions by at most k / 4 = merge_pt / 2.
                let bulge = (f64::from(material.merge_pt) * scale * 0.5).ceil() as i32 + 1;
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

/// The regions grouped by material and client group (in the order they appear), at most
/// `MAX_SHAPES` per group: the regions of a group merge with each other.
fn groups(regions: &[PhysicalRegion]) -> Vec<Vec<PhysicalRegion>> {
    let mut groups: Vec<Vec<PhysicalRegion>> = Vec::new();
    for region in regions {
        match groups.iter_mut().find(|group| {
            group.len() < MAX_SHAPES
                && group.first().is_some_and(|first| {
                    first.material == region.material
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

/// Blurs the capture with the material's σ and mixes in the sharp share.
fn blur(
    renderer: &mut GlesRenderer,
    programs: &GlassPrograms,
    sharp: &GlesTexture,
    size: Size<i32, Physical>,
    material: MaterialParams,
    scale: f64,
) -> Result<GlesTexture, GlesError> {
    let sigma = material.sigma * scale;
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
        1.0 - material.sharp,
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
        let px = first.scale as f32;
        // The regions relative to the texture: up to 8 rectangles in two mat4 (one column per
        // rectangle: x, y, width, height) and the radii in a mat4.
        let mut rects = [[0.0_f32; 16]; 2];
        let mut radii = [0.0_f32; 16];
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
                UniformValue::_1f(first.material.fill_on_gray()),
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
