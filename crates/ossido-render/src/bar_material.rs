//! The spaces bar material of Panorama (`plan/reference/missioncontrol.md` › MC-BLUR): the
//! wallpaper under the bar, and only that (no windows, no Dock), goes through a compressive
//! tone curve and then a Gaussian blur of σ ≈ 26 pt, mirrored at the bar's edges. No tint.
//! The capture is redone only when the wallpaper, the bar's height or its look change.

use std::cell::RefCell;

use smithay::backend::renderer::element::{Element, Id, RenderElement};
use smithay::backend::renderer::gles::{
    GlesError, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformType, UniformValue,
};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::utils::{Physical, Rectangle, Scale, Size};

use crate::backdrop::BlurredElement;
use crate::glass::{BlurEdge, gaussian, pass, render_to_texture, whole};
use crate::glass_profile::BlurScale;

pub const TONE_SHADER: &str = include_str!("shaders/bar_tone.frag");
pub const TONE_UNIFORMS: [(&str, UniformType); 3] = [
    ("light", UniformType::_1f),
    ("expansion", UniformType::_1f),
    ("height_pt", UniformType::_1f),
];
pub const BLUR_SHADER: &str = include_str!("shaders/blur_mirror.frag");
pub const BLUR_UNIFORMS: [(&str, UniformType); 2] =
    [("step", UniformType::_2f), ("sigma", UniformType::_1f)];

/// The programs of the bar material.
#[derive(Clone)]
pub struct BarPrograms {
    pub tone: GlesTexProgram,
    pub blur: GlesTexProgram,
    /// The box prefilter and the upsampling of the reduced-resolution blur (the glass's).
    pub downsample: GlesTexProgram,
    pub upsample: GlesTexProgram,
}

/// At what fraction of the resolution the bar's blur (σ 26–27 pt) runs, chosen like the glass
/// kinds' (`glass_profile::BlurScale`) by the equivalence test: mean / max against full
/// resolution 0.21 / 1 at f2, 0.25 / 1 at f4, 0.29 / 1 at f8 (1×, 1.5× and 2×).
pub const BLUR_SCALE: BlurScale = BlurScale {
    below_2x: 8,
    from_2x: 8,
};

/// The look of the bar the capture was made for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarTone {
    pub light: bool,
    /// 0 compact, 1 expanded (the highlight down the bar).
    pub expansion: f64,
    pub sigma_pt: f64,
}

type Signature = (
    Vec<(Id, CommitCounter, Rectangle<i32, Physical>)>,
    Rectangle<i32, Physical>,
    [u64; 3],
    // The output scale and the clear color (bits).
    [u64; 5],
);

/// The bar material between frames.
#[derive(Default)]
pub struct BarMaterial {
    state: RefCell<Option<(Signature, GlesTexture, Id)>>,
}

impl BarMaterial {
    /// The material for the bar `area` (output pixels, at rest) from the `wallpaper` elements
    /// (from the front), drawn at `dst` with `alpha`.
    #[allow(clippy::too_many_arguments)]
    pub fn element<E: RenderElement<GlesRenderer>>(
        &self,
        renderer: &mut GlesRenderer,
        programs: &BarPrograms,
        wallpaper: &[E],
        area: Rectangle<i32, Physical>,
        dst: Rectangle<i32, Physical>,
        scale: f64,
        tone: BarTone,
        clear_color: [f32; 4],
        alpha: f32,
    ) -> Result<Option<BlurredElement>, GlesError> {
        if area.is_empty() || dst.is_empty() {
            return Ok(None);
        }
        let signature: Signature = (
            wallpaper
                .iter()
                .filter(|e| e.geometry(Scale::from(scale)).overlaps(area))
                .map(|e| {
                    (
                        e.id().clone(),
                        e.current_commit(),
                        e.geometry(Scale::from(scale)),
                    )
                })
                .collect(),
            area,
            [
                u64::from(tone.light),
                (tone.expansion * 1000.0).round().to_bits(),
                tone.sigma_pt.to_bits(),
            ],
            [
                scale.to_bits(),
                u64::from(clear_color[0].to_bits()),
                u64::from(clear_color[1].to_bits()),
                u64::from(clear_color[2].to_bits()),
                u64::from(clear_color[3].to_bits()),
            ],
        );
        let mut state = self.state.borrow_mut();
        if !matches!(state.as_ref(), Some((s, ..)) if *s == signature) {
            let sharp = render_to_texture(renderer, wallpaper, area, scale, clear_color)?;
            let factor = BLUR_SCALE.factor(scale);
            let texture = material(renderer, programs, &sharp, area.size, scale, tone, factor)?;
            *state = Some((signature, texture, Id::new()));
        }
        let Some((_, texture, id)) = state.as_ref() else {
            return Err(GlesError::FramebufferBindingError);
        };
        let src = whole(area.size);
        Ok(Some(BlurredElement::new(
            id.clone(),
            texture.clone(),
            src,
            dst,
            alpha,
        )))
    }
}

/// Tone at full resolution, then the mirrored blur at the resolution `BLUR_SCALE` allows,
/// back to full size.
pub fn material(
    renderer: &mut GlesRenderer,
    programs: &BarPrograms,
    sharp: &GlesTexture,
    size: Size<i32, Physical>,
    scale: f64,
    tone: BarTone,
    factor: u32,
) -> Result<GlesTexture, GlesError> {
    let uniforms = [
        Uniform::new(
            "light",
            UniformValue::_1f(if tone.light { 1.0 } else { 0.0 }),
        ),
        Uniform::new("expansion", UniformValue::_1f(tone.expansion as f32)),
        Uniform::new(
            "height_pt",
            UniformValue::_1f((f64::from(size.h) / scale) as f32),
        ),
    ];
    let toned = pass(
        renderer,
        sharp,
        whole(size),
        size,
        Some(&programs.tone),
        &uniforms,
        1.0,
        None,
    )?;
    // The blur after the tone: fine stripes average to the mean of their toned values, as
    // measured.
    let blurred = gaussian(
        renderer,
        &programs.blur,
        &programs.downsample,
        BlurEdge::Mirror,
        &toned,
        size,
        tone.sigma_pt * scale,
        factor,
    )?;
    if blurred.size == size {
        return Ok(blurred.texture);
    }
    blurred.back(renderer, &programs.upsample, size, 1.0, None)
}

/// The tone curve of the shader, for tests and documentation: luminance `v` (0–255) at `y`
/// points down the bar.
pub fn tone_curve(v: f64, y: f64, light: bool, expansion: f64) -> f64 {
    let t = ((y - 20.0) / 120.0).clamp(0.0, 1.0);
    let s = (1.0 - (1.0 - t).powf(1.6)) * expansion;
    let base = 18.0 + 0.664 * v - 0.000944 * v * v;
    base + if light { 73.0 } else { 0.0 } + 45.0 * s * (v / 255.0).clamp(0.0, 1.0).powf(2.73)
}

#[cfg(test)]
mod tests {
    use super::tone_curve;

    #[test]
    fn the_tone_curve_goes_through_the_measured_points() {
        let near = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol;
        // Compact bar, dark: black → 18–20, gray #7a7b80 (luminance ~123) → 85–86, white → 121–126.
        assert!(near(tone_curve(0.0, 20.0, false, 0.0), 18.0, 1.0));
        assert!(near(tone_curve(123.0, 20.0, false, 0.0), 85.5, 1.0));
        assert!(near(tone_curve(255.0, 20.0, false, 0.0), 125.0, 2.0));
        // Expanded, dark: white 141 at y 50, 155 at 80, 166 at 110, 170 from 140; gray 92 at
        // the bottom; black stays 18.
        for (y, white) in [(50.0, 141.0), (80.0, 155.0), (110.0, 166.0), (150.0, 170.0)] {
            let got = tone_curve(255.0, y, false, 1.0);
            assert!(near(got, white, 4.5), "white at {y}: {got}");
        }
        assert!(near(tone_curve(123.0, 150.0, false, 1.0), 92.0, 1.0));
        assert!(near(tone_curve(0.0, 150.0, false, 1.0), 18.0, 0.5));
        // Light (derived): gray → #9e9fa5 (158), expanded bottom → #a4a5ab (164).
        assert!(near(tone_curve(123.0, 20.0, true, 0.0), 158.5, 1.0));
        assert!(near(tone_curve(123.0, 150.0, true, 1.0), 164.5, 1.5));
        // 50 % stripes average to (T(0) + T(255)) / 2 ≈ 72 in dark.
        let stripes =
            (tone_curve(0.0, 20.0, false, 0.0) + tone_curve(255.0, 20.0, false, 0.0)) / 2.0;
        assert!(near(stripes, 72.0, 1.5), "{stripes}");
    }
}
