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

/// Parameters of a material: blur, sharp share and color (`out = M · in + b` on the encoded
/// sRGB values, 0–1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialParams {
    /// σ of the Gaussian in points.
    pub sigma: f64,
    /// Share of the sharp image mixed with the blurred one.
    pub sharp: f32,
    pub matrix: [[f32; 3]; 3],
    pub offset: [f32; 3],
    /// The edge lens.
    pub lens: Lens,
    /// The edge row and the inner glow.
    pub edge: Edge,
    /// Gap below which two regions of this material merge (0: never).
    pub merge_pt: f32,
}

/// The glass edge: on top and at the bottom the 1 pt `rim` row is added to the material, then
/// a glow `glow · e^(−t/decay)`; on the sides the row is black with alpha `side_dark`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub rim: [f32; 3],
    pub glow: f32,
    pub glow_decay_pt: f32,
    pub side_dark: f32,
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

/// Edge shared by the measured glasses (Dock, modules, HUD, notifications, Spotlight): on the
/// Dock's gray the 1 pt row on top and at the bottom is +60, +60, +63 over the fill, the
/// inner glow +13, +8, +5, +3, +2, +1 (≈ 13·e^(−t/2.2)), additive even over black;
/// on the sides 1 dark pt (`#3c3c40` over `#818289`: black with alpha 0.535). `dock.md`, `glass.md`.
const EDGE: Edge = Edge {
    rim: [60.0 / 255.0, 60.0 / 255.0, 63.0 / 255.0],
    glow: 13.0 / 255.0,
    glow_decay_pt: 2.2,
    side_dark: 0.535,
};

/// Edge of the Control Center modules (`regular` Liquid Glass): stronger than the Dock's.
/// Top row `#bfc0c7`–`#c7c8cf` over a `#6a6b70`–`#6e6f73` fill (+86), glow +18, +9, +4, +2
/// (decay ~1.4 pt), sides `#27282c` (black with alpha ~0.63). `controlcenter.md`,
/// "CC · luminous edge and fill".
const REGULAR_EDGE: Edge = Edge {
    rim: [86.0 / 255.0, 86.0 / 255.0, 88.0 / 255.0],
    glow: 18.0 / 255.0,
    glow_decay_pt: 1.4,
    side_dark: 0.63,
};

/// Edge of notifications: the 1 pt row is dark all around and is **outside** the frame (the
/// client widens the region by 1 pt per side): `#5e6064` on top and `#636367` at the bottom
/// over a ~`#c0` fill (−98), `#393a3d` on the sides (black with alpha ~0.70). Inside, the
/// first row on top and at the bottom is ~22 levels lighter and the glow is absorbed in
/// 2 pt (`d6 c3 c1 bf`). `notifications.md`, "Banner · material over gray".
const NOTIFICATION_EDGE: Edge = Edge {
    rim: [-98.0 / 255.0, -97.0 / 255.0, -98.0 / 255.0],
    glow: 22.0 / 255.0,
    glow_decay_pt: 0.5,
    side_dark: 0.70,
};

/// Dock edge in dark: row on top and at the bottom `#8e8f95` over the `#68696e` fill
/// (+38, +38, +39), inner glow +8, +5, +3, +2, +1, +1 (≈ 8·e^(−t/2.2)), sides
/// `#2f3033` (black with alpha ~0.545). `dock.md`, background table.
const DARK_EDGE: Edge = Edge {
    rim: [38.0 / 255.0, 38.0 / 255.0, 39.0 / 255.0],
    glow: 8.0 / 255.0,
    glow_decay_pt: 2.2,
    side_dark: 0.545,
};

/// Dock background, light: σ 2.5–3.25 pt (2.75 is used), w 0.28, free matrix
/// (rmse 2.45) with b = (37.3; 39.3; 36.8).
pub const CLEAR_LIGHT: MaterialParams = MaterialParams {
    sigma: 2.75,
    sharp: 0.28,
    matrix: [
        [1.100, -0.314, -0.033],
        [-0.146, 0.805, 0.087],
        [-0.158, -0.328, 1.242],
    ],
    offset: [37.3 / 255.0, 39.3 / 255.0, 36.8 / 255.0],
    lens: DOCK_LENS,
    edge: EDGE,
    merge_pt: MERGE_PT,
};

/// Dock background, dark: σ 2.5–3.25 pt (2.875 is used, halfway), w 0.28, free matrix
/// (rmse 4.71) with b = (16.2; 15.7; 16.3) (glass.md, "Glass · Dock", "Dark" tables).
pub const CLEAR_DARK: MaterialParams = MaterialParams {
    sigma: 2.875,
    sharp: 0.28,
    matrix: [
        [1.058, -0.307, -0.083],
        [-0.199, 0.729, 0.141],
        [-0.164, -0.327, 1.159],
    ],
    offset: [16.2 / 255.0, 15.7 / 255.0, 16.3 / 255.0],
    lens: DOCK_LENS,
    edge: DARK_EDGE,
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

/// Control Center modules, light: σ 4.5 pt, w 0.04, free matrix (rmse 3.33)
/// with b = (30.6; 32.0; 30.3); identical in dark (glass.md, "Control Center").
pub const REGULAR_LIGHT: MaterialParams = MaterialParams {
    sigma: 4.5,
    sharp: 0.04,
    matrix: [
        [0.946, -0.285, -0.037],
        [-0.126, 0.651, 0.094],
        [-0.140, -0.297, 1.064],
    ],
    offset: [30.6 / 255.0, 32.0 / 255.0, 30.3 / 255.0],
    lens: LIQUID_LENS,
    edge: REGULAR_EDGE,
    merge_pt: MERGE_PT,
};

/// Control Center modules, dark: same blur, dark matrix (rmse 1.82) with
/// b = (31.2; 32.5; 30.8), almost the same as the light one; edge as in light ("same
/// identical look in light and dark over gray", controlcenter.md).
pub const REGULAR_DARK: MaterialParams = MaterialParams {
    matrix: [
        [0.942, -0.289, -0.041],
        [-0.130, 0.646, 0.090],
        [-0.144, -0.304, 1.061],
    ],
    offset: [31.2 / 255.0, 32.5 / 255.0, 30.8 / 255.0],
    ..REGULAR_LIGHT
};

/// The Liquid Glass lens, measured on the Control Center modules: between ~6 and ~24 pt from
/// the edge it magnifies (up to ~4 pt inwards), in the outer ~6 pt it samples the background
/// outside the module, up to ~25 pt past the edge, squeezed. On top the outer band is ~2 pt.
/// The bottom side is not measured: like the top one. Uncertainty ±3 pt in the outer band
/// (glass.md, "Refraction, much stronger").
const LIQUID_LENS: Lens = Lens {
    side: &[
        (0.0, -25.0),
        (6.0, -4.0),
        (10.0, 14.2),
        (24.0, 25.5),
        (30.0, 30.0),
    ],
    top: &[
        (0.0, -27.0),
        (2.0, -5.0),
        (6.0, 9.6),
        (19.0, 21.3),
        (28.0, 28.0),
    ],
    bottom: &[
        (0.0, -27.0),
        (2.0, -5.0),
        (6.0, 9.6),
        (19.0, 21.3),
        (28.0, 28.0),
    ],
};

/// Menus, light: σ ≈ 9 pt, negligible sharp component, matrix (rmse 4.1) with
/// b = (142, 140, 140).
pub const THICK_LIGHT: MaterialParams = MaterialParams {
    sigma: 9.0,
    sharp: 0.0,
    matrix: [
        [0.781, -0.200, -0.124],
        [-0.079, 0.579, -0.034],
        [-0.166, -0.263, 0.899],
    ],
    offset: [142.0 / 255.0, 140.0 / 255.0, 140.0 / 255.0],
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
    edge: EDGE,
    merge_pt: MERGE_PT,
};

/// Menus, dark: σ ≈ 9 pt as in light (8.7 from the stripes), dark matrix (menubar.md,
/// "Color"). The dark transfer is an S-curve the matrix follows poorly (rmse 9.9; with
/// b = (44, 47, 45) the gray `#7a7b80` would give ~96 instead of 111): the offset is
/// recomputed so that over gray the fill is the measured one, `#6f7075`; black then gives
/// ~59, like the stripes (58). Edge as in light (the dark menu's rim is not measured in the
/// model's form).
pub const THICK_DARK: MaterialParams = MaterialParams {
    matrix: [
        [0.891, -0.347, -0.113],
        [-0.150, 0.580, 0.000],
        [-0.177, -0.391, 1.001],
    ],
    offset: [59.4 / 255.0, 59.0 / 255.0, 58.6 / 255.0],
    ..THICK_LIGHT
};

pub const BLUR_SHADER: &str = include_str!("shaders/blur.frag");
pub const BLUR_UNIFORMS: [(&str, UniformType); 2] =
    [("step", UniformType::_2f), ("sigma", UniformType::_1f)];
pub const MATERIAL_SHADER: &str = include_str!("shaders/material.frag");
pub const MATERIAL_UNIFORMS: [(&str, UniformType); 16] = [
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
    ("scale_px", UniformType::_1f),
    ("lens", UniformType::Matrix4x4),
    ("lens_reach", UniformType::_1f),
    ("rim", UniformType::_3f),
    ("edge", UniformType::_2f),
    ("side_dark", UniformType::_1f),
];

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
/// when g < k / 2). Measured limits: Spotlight's round buttons 10 pt from the panel stay
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
    /// A downward tip instead of a rectangle (`add_tip`).
    pub tip: bool,
    /// An upward tip (`add_upward_tip`).
    pub tip_up: bool,
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
            && self.tip_up == other.tip_up
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
            // Negative radius: a tip (the shader recognizes it by the sign); −2 points up.
            if let Some(slot) = radii.get_mut(i) {
                *slot = if shape.tip_up {
                    -2.0
                } else if shape.tip {
                    -1.0
                } else {
                    shape.radius as f32
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
            Uniform::new("scale_px", UniformValue::_1f(px)),
            Uniform::new("lens", lens.uniform(px)),
            Uniform::new("lens_reach", UniformValue::_1f(lens_extent(&lens) * px)),
            Uniform::new(
                "rim",
                UniformValue::_3f(edge.rim[0], edge.rim[1], edge.rim[2]),
            ),
            Uniform::new(
                "edge",
                UniformValue::_2f(edge.glow, edge.glow_decay_pt * px),
            ),
            Uniform::new("side_dark", UniformValue::_1f(edge.side_dark)),
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
