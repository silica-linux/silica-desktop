//! HiDPI scaled modes (#216): the scene is composited at 2x the "looks like" size into an
//! offscreen framebuffer and this module reduces it to the display's native mode.
//!
//! The reduction is a separable Lanczos (two passes: horizontal into an intermediate texture
//! as wide as the output, then vertical into the target). The ratio is always between 1 and 2,
//! so a window of `lobes` native pixels to each side is at most 13 taps per axis. Only the
//! damaged parts are filtered: every damaged rectangle of the source grows by the filter's
//! reach, becomes a rectangle of the output, and the intermediate texture is refreshed for the
//! rows that rectangle reads ([`plan`]).
//!
//! The GPU shader (`shaders/scale.frag`) and the CPU reference ([`resample`]) compute the same
//! weights; the tests compare them. `docs/displays.md` has the choice of the filter and the
//! measurements.

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::gles::{
    GlesError, GlesRenderer, GlesTarget, GlesTexProgram, GlesTexture, Uniform, UniformName,
    UniformType,
};
use smithay::backend::renderer::sync::SyncPoint;
use smithay::backend::renderer::{Bind, Frame, Offscreen, Renderer};
use smithay::utils::{Buffer, Physical, Rectangle, Size, Transform};
use tracing::warn;

/// The shader of one axis of the reduction.
pub const SHADER: &str = include_str!("shaders/scale.frag");
pub const UNIFORMS: [(&str, UniformType); 4] = [
    ("dir", UniformType::_2f),
    ("src_size", UniformType::_2f),
    ("out_size", UniformType::_2f),
    ("lobes", UniformType::_1f),
];

/// Most rectangles drawn per pass; beyond this the damage becomes its bounding box.
pub const MAX_RECTS: usize = 16;

/// The reconstruction filters compared in `docs/displays.md`. The GPU runs the Lanczos ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Filter {
    /// Mean of the source pixels under the output pixel.
    Box,
    /// Triangle (what bilinear sampling does when the reduction is below 2).
    Bilinear,
    /// Catmull-Rom (bicubic, B = 0, C = 1/2).
    CatmullRom,
    /// Lanczos with 2 or 3 lobes.
    Lanczos(u32),
}

impl Filter {
    /// The filter Ossido uses on the GPU.
    pub const DEFAULT: Filter = Filter::Lanczos(2);

    /// Half the width of the kernel, in output pixels.
    pub fn radius(self) -> f64 {
        match self {
            Filter::Box => 0.5,
            Filter::Bilinear => 1.0,
            Filter::CatmullRom => 2.0,
            Filter::Lanczos(lobes) => f64::from(lobes),
        }
    }

    /// The kernel at distance `x` in output pixels.
    pub fn kernel(self, x: f64) -> f64 {
        let x = x.abs();
        match self {
            Filter::Box => {
                if x < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Filter::Bilinear => (1.0 - x).max(0.0),
            Filter::CatmullRom => {
                if x < 1.0 {
                    1.5 * x * x * x - 2.5 * x * x + 1.0
                } else if x < 2.0 {
                    -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
                } else {
                    0.0
                }
            }
            Filter::Lanczos(lobes) => {
                let a = f64::from(lobes);
                if x < 1e-4 {
                    1.0
                } else if x >= a {
                    0.0
                } else {
                    let px = std::f64::consts::PI * x;
                    a * px.sin() * (px / a).sin() / (px * px)
                }
            }
        }
    }
}

/// The source pixels and weights (normalised) of output pixel `o` when `src` pixels become
/// `dst` along one axis; the indices past the edges repeat the edge pixel.
pub fn taps(filter: Filter, o: usize, src: usize, dst: usize) -> Vec<(usize, f64)> {
    if src == 0 || dst == 0 {
        return Vec::new();
    }
    let ratio = src as f64 / dst as f64;
    let centre = (o as f64 + 0.5) * ratio - 0.5;
    let step = ratio.max(1.0);
    let reach = filter.radius() * step;
    let first = (centre - reach).ceil() as i64;
    let last = (centre + reach).floor() as i64;
    let top = src as i64 - 1;
    let mut out: Vec<(usize, f64)> = Vec::new();
    let mut total = 0.0;
    for i in first..=last {
        let w = filter.kernel((i as f64 - centre) / step);
        if w == 0.0 {
            continue;
        }
        out.push((i.clamp(0, top) as usize, w));
        total += w;
    }
    if total.abs() < 1e-9 {
        return vec![((centre.round() as i64).clamp(0, top) as usize, 1.0)];
    }
    for (_, w) in &mut out {
        *w /= total;
    }
    out
}

/// The CPU reference: an RGBA image (premultiplied, 8 bits) `sw` x `sh` reduced to `dw` x
/// `dh`. Horizontal first, then vertical, with the clamp the shader applies. The GPU passes
/// are compared with it in the tests; it is slow and not used at run time.
pub fn resample(
    filter: Filter,
    rgba: &[u8],
    (sw, sh): (usize, usize),
    (dw, dh): (usize, usize),
) -> Vec<u8> {
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 || rgba.len() < sw * sh * 4 {
        return vec![0; dw * dh * 4];
    }
    let at = |data: &[f64], i: usize| data.get(i).copied().unwrap_or(0.0);
    let source: Vec<f64> = rgba.iter().map(|&v| f64::from(v)).collect();
    let columns: Vec<_> = (0..dw).map(|x| taps(filter, x, sw, dw)).collect();
    let mut mid = Vec::with_capacity(dw * sh * 4);
    for y in 0..sh {
        for column in &columns {
            for c in 0..4 {
                let sum: f64 = column
                    .iter()
                    .map(|&(i, w)| w * at(&source, (y * sw + i) * 4 + c))
                    .sum();
                mid.push(sum.clamp(0.0, 255.0));
            }
        }
    }
    let rows: Vec<_> = (0..dh).map(|y| taps(filter, y, sh, dh)).collect();
    let mut out = Vec::with_capacity(dw * dh * 4);
    for row in &rows {
        for x in 0..dw {
            let mut px = [0f64; 4];
            for (c, slot) in px.iter_mut().enumerate() {
                *slot = row
                    .iter()
                    .map(|&(i, w)| w * at(&mid, (i * dw + x) * 4 + c))
                    .sum::<f64>()
                    .clamp(0.0, 255.0);
            }
            let alpha = px[3];
            out.extend(px.iter().enumerate().map(|(c, v)| {
                let v = if c < 3 { v.min(alpha) } else { *v };
                v.round() as u8
            }));
        }
    }
    out
}

/// What the passes draw for a damage: `dst` rectangles of the output (pass 2, vertical) and
/// `mid` rectangles of the intermediate texture, `dst`'s width by the source rows they read
/// (pass 1, horizontal). Neither list has overlapping rectangles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub mid: Vec<Rectangle<i32, Physical>>,
    pub dst: Vec<Rectangle<i32, Physical>>,
}

fn right(r: &Rectangle<i32, Physical>) -> i32 {
    r.loc.x.saturating_add(r.size.w)
}

fn bottom(r: &Rectangle<i32, Physical>) -> i32 {
    r.loc.y.saturating_add(r.size.h)
}

fn hull(a: Rectangle<i32, Physical>, b: Rectangle<i32, Physical>) -> Rectangle<i32, Physical> {
    let (x0, y0) = (a.loc.x.min(b.loc.x), a.loc.y.min(b.loc.y));
    let (x1, y1) = (right(&a).max(right(&b)), bottom(&a).max(bottom(&b)));
    Rectangle::new((x0, y0).into(), (x1 - x0, y1 - y0).into())
}

/// Replaces the overlapping rectangles by their hull until none overlap; more than
/// [`MAX_RECTS`] left become one.
pub fn merge(rects: Vec<Rectangle<i32, Physical>>) -> Vec<Rectangle<i32, Physical>> {
    let mut rects: Vec<Rectangle<i32, Physical>> = {
        let mut out: Vec<Rectangle<i32, Physical>> = Vec::new();
        for rect in rects.into_iter().filter(|r| r.size.w > 0 && r.size.h > 0) {
            let mut current = rect;
            while let Some(at) = out.iter().position(|o| o.overlaps(current)) {
                current = hull(out.swap_remove(at), current);
            }
            out.push(current);
        }
        out
    };
    if rects.len() > MAX_RECTS {
        let all = rects.iter().copied().reduce(hull);
        return all.into_iter().collect();
    }
    rects.sort_by_key(|r| (r.loc.y, r.loc.x));
    rects
}

/// The passes for `damage` (rectangles of the source, in source pixels) when `src` becomes
/// `dst`, with a filter of `radius` output pixels.
pub fn plan(
    damage: &[Rectangle<i32, Physical>],
    src: Size<i32, Physical>,
    dst: Size<i32, Physical>,
    radius: u32,
) -> Plan {
    if src.w <= 0 || src.h <= 0 || dst.w <= 0 || dst.h <= 0 {
        return Plan::default();
    }
    let (rx, ry) = (
        f64::from(src.w) / f64::from(dst.w),
        f64::from(src.h) / f64::from(dst.h),
    );
    let margin = radius as i32 + 1;
    let bounds = Rectangle::from_size(dst);
    let grown: Vec<_> = damage
        .iter()
        .filter_map(|d| {
            let d = d.intersection(Rectangle::from_size(src))?;
            let x0 = (f64::from(d.loc.x) / rx).floor() as i32 - margin;
            let y0 = (f64::from(d.loc.y) / ry).floor() as i32 - margin;
            let x1 = (f64::from(right(&d)) / rx).ceil() as i32 + margin;
            let y1 = (f64::from(bottom(&d)) / ry).ceil() as i32 + margin;
            Rectangle::new((x0, y0).into(), (x1 - x0, y1 - y0).into()).intersection(bounds)
        })
        .collect();
    plan_from_dst(merge(grown), src, dst, radius)
}

/// The passes that redraw the given rectangles of the output (already grown by the margin).
pub fn plan_from_dst(
    dst_rects: Vec<Rectangle<i32, Physical>>,
    src: Size<i32, Physical>,
    dst: Size<i32, Physical>,
    radius: u32,
) -> Plan {
    if src.w <= 0 || src.h <= 0 || dst.w <= 0 || dst.h <= 0 {
        return Plan::default();
    }
    let ry = f64::from(src.h) / f64::from(dst.h);
    let reach = f64::from(radius) * ry.max(1.0);
    let rows = Rectangle::new((0, 0).into(), (dst.w, src.h).into());
    let mid = merge(
        dst_rects
            .iter()
            .filter_map(|r| {
                let y0 = (f64::from(r.loc.y) * ry - reach).floor() as i32 - 1;
                let y1 = (f64::from(bottom(r)) * ry + reach).ceil() as i32 + 1;
                Rectangle::new((r.loc.x, y0).into(), (r.size.w, y1 - y0).into()).intersection(rows)
            })
            .collect(),
    );
    Plan {
        mid,
        dst: dst_rects,
    }
}

/// The regions of the scanout buffers that changed in the last frames, to redraw on a buffer
/// of a given age (DRM swapchains rotate between buffers that are 1, 2 or 3 frames old).
#[derive(Debug, Clone, Default)]
pub struct DamageHistory {
    frames: Vec<Vec<Rectangle<i32, Physical>>>,
}

/// The longest swapchain this remembers.
const HISTORY: usize = 4;

impl DamageHistory {
    /// Forget everything: the next buffers are redrawn in full.
    pub fn reset(&mut self) {
        self.frames.clear();
    }

    /// The rectangles to redraw on a buffer of `age` for a frame whose own damage is
    /// `current`, or `None` for all of it (age 0, or older than the memory). Call before
    /// [`DamageHistory::push`].
    pub fn for_age(
        &self,
        age: usize,
        current: &[Rectangle<i32, Physical>],
    ) -> Option<Vec<Rectangle<i32, Physical>>> {
        if age == 0 || age - 1 > self.frames.len() {
            return None;
        }
        let mut all = current.to_vec();
        for frame in self.frames.iter().rev().take(age - 1) {
            all.extend(frame.iter().copied());
        }
        Some(merge(all))
    }

    /// Records the damage of a frame that was shown.
    pub fn push(&mut self, damage: Vec<Rectangle<i32, Physical>>) {
        self.frames.push(damage);
        if self.frames.len() > HISTORY {
            self.frames.remove(0);
        }
    }
}

/// The two passes on a renderer, with the intermediate texture kept between frames.
pub struct Downscaler {
    /// None if the shader did not compile: the passes are then plain linear blits.
    program: Option<GlesTexProgram>,
    lobes: u32,
    mid: Option<(Size<i32, Physical>, GlesTexture)>,
}

fn buffer(size: Size<i32, Physical>) -> Size<i32, Buffer> {
    size.to_logical(1).to_buffer(1, Transform::Normal)
}

impl Downscaler {
    /// Compiles the shader. `lobes` is 2 or 3.
    pub fn new(renderer: &mut GlesRenderer, lobes: u32) -> Result<Self, GlesError> {
        let names: Vec<UniformName<'_>> = UNIFORMS
            .iter()
            .map(|(name, kind)| UniformName::new(*name, *kind))
            .collect();
        let program = match renderer.compile_custom_texture_shader(SHADER, &names) {
            Ok(program) => Some(program),
            Err(err) => {
                warn!("shader of the scaled mode not compiled, bilinear instead: {err:?}");
                None
            }
        };
        Ok(Self {
            program,
            lobes: lobes.clamp(2, 3),
            mid: None,
        })
    }

    pub fn lobes(&self) -> u32 {
        self.lobes
    }

    /// Reduces the damaged part of `source` (`src_size` pixels) into `target` (`dst_size`),
    /// leaving the rest of `target` alone. Returns the output rectangles drawn, and the sync
    /// point of the last pass. `damage` is in source pixels.
    pub fn run(
        &mut self,
        renderer: &mut GlesRenderer,
        source: &GlesTexture,
        src_size: Size<i32, Physical>,
        target: &mut GlesTarget<'_>,
        dst_size: Size<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
    ) -> Result<(Vec<Rectangle<i32, Physical>>, SyncPoint), GlesError> {
        let plan = plan(damage, src_size, dst_size, self.lobes);
        self.draw(renderer, source, src_size, target, dst_size, &plan)
    }

    /// The same for an explicit [`Plan`], whose `dst` rectangles may be any part of the
    /// output (a scanout buffer of some age needs more than this frame's damage).
    pub fn draw(
        &mut self,
        renderer: &mut GlesRenderer,
        source: &GlesTexture,
        src_size: Size<i32, Physical>,
        target: &mut GlesTarget<'_>,
        dst_size: Size<i32, Physical>,
        plan: &Plan,
    ) -> Result<(Vec<Rectangle<i32, Physical>>, SyncPoint), GlesError> {
        let mid_size: Size<i32, Physical> = (dst_size.w, src_size.h).into();
        let fresh = !matches!(&self.mid, Some((size, _)) if *size == mid_size);
        if fresh {
            let texture: GlesTexture = Offscreen::<GlesTexture>::create_buffer(
                renderer,
                Fourcc::Abgr8888,
                buffer(mid_size),
            )?;
            self.mid = Some((mid_size, texture));
        }
        let Some((_, mid)) = self.mid.as_mut() else {
            return Err(GlesError::FramebufferBindingError);
        };
        let lobes = self.lobes as f32;
        let (sw, sh) = (src_size.w as f32, src_size.h as f32);
        let columns = [
            Uniform::new("dir", (1.0_f32, 0.0_f32)),
            Uniform::new("src_size", (sw, sh)),
            Uniform::new("out_size", (dst_size.w as f32, sh)),
            Uniform::new("lobes", lobes),
        ];
        let rows = [
            Uniform::new("dir", (0.0_f32, 1.0_f32)),
            Uniform::new("src_size", (dst_size.w as f32, sh)),
            Uniform::new("out_size", (dst_size.w as f32, dst_size.h as f32)),
            Uniform::new("lobes", lobes),
        ];
        // Pass 1: columns, source -> intermediate.
        if !plan.mid.is_empty() {
            let mut bound = renderer.bind(mid)?;
            let mut frame = renderer.render(&mut bound, mid_size, Transform::Normal)?;
            frame.clear([0.0; 4].into(), &plan.mid)?;
            frame.render_texture_from_to(
                source,
                Rectangle::from_size(buffer(src_size).to_f64()),
                Rectangle::from_size(mid_size),
                &plan.mid,
                &[],
                Transform::Normal,
                1.0,
                self.program.as_ref(),
                if self.program.is_some() {
                    &columns
                } else {
                    &[]
                },
            )?;
            let _sync = frame.finish()?;
        }
        // Pass 2: rows, intermediate -> target.
        let mut frame = renderer.render(target, dst_size, Transform::Normal)?;
        if !plan.dst.is_empty() {
            frame.clear([0.0; 4].into(), &plan.dst)?;
            frame.render_texture_from_to(
                mid,
                Rectangle::from_size(buffer(mid_size).to_f64()),
                Rectangle::from_size(dst_size),
                &plan.dst,
                &[],
                Transform::Normal,
                1.0,
                self.program.as_ref(),
                if self.program.is_some() { &rows } else { &[] },
            )?;
        }
        let sync = frame.finish()?;
        Ok((plan.dst.clone(), sync))
    }
}

#[cfg(test)]
#[path = "scaler_tests.rs"]
mod tests;
