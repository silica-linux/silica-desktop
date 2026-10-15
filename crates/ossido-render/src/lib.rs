//! Ossido's GLES renderer and glass shaders.
//!
//! The effects are smithay render elements that Ossido puts in the output's list. The
//! parameters come from the macOS 27 measurements in `plan/reference/`, cited next to every
//! constant.

pub mod animation;
pub mod backdrop;
pub mod bar_material;
pub mod border;
pub mod cache;
pub mod corners;
pub mod genie;
pub mod glass;
pub mod glass_profile;
pub mod shadow;
pub mod sprite;
pub mod title;
pub mod titlebar;

use smithay::backend::renderer::gles::{
    GlesPixelProgram, GlesRenderer, GlesTexProgram, UniformName, UniformType,
};
use tracing::warn;

/// The programs compiled for a renderer. If a shader does not compile, the effect is missing
/// but Ossido keeps drawing (never crash).
#[derive(Clone, Default)]
pub struct Shaders {
    pub corners: Option<GlesTexProgram>,
    pub shadow: Option<GlesPixelProgram>,
    pub titlebar: Option<GlesPixelProgram>,
    /// The 1 pt inner border of windows in dark.
    pub border: Option<GlesPixelProgram>,
    pub glass: Option<glass::GlassPrograms>,
    pub tone: Option<GlesTexProgram>,
    /// The funnel deformation of minimizing into the Dock.
    pub genie: Option<GlesTexProgram>,
    /// The spaces bar material (tone, mirrored blur).
    pub bar: Option<bar_material::BarPrograms>,
}

/// The renderer programs, compiled on the first request and kept in the EGL context.
pub fn shaders(renderer: &mut GlesRenderer) -> Shaders {
    if let Some(shaders) = renderer.egl_context().user_data().get::<Shaders>() {
        return shaders.clone();
    }
    let corners = compile_texture(renderer, "corners", corners::SHADER, &corners::UNIFORMS);
    let shadow = compile_pixel(renderer, "shadow", shadow::SHADER, &shadow::UNIFORMS);
    let titlebar = compile_pixel(renderer, "title bar", titlebar::SHADER, &titlebar::UNIFORMS);
    let border = compile_pixel(renderer, "window border", border::SHADER, &border::UNIFORMS);
    let blur = compile_texture(renderer, "blur", glass::BLUR_SHADER, &glass::BLUR_UNIFORMS);
    let material = compile_texture(
        renderer,
        "material",
        glass::MATERIAL_SHADER,
        &glass::MATERIAL_UNIFORMS,
    );
    let glass = blur
        .zip(material)
        .map(|(blur, material)| glass::GlassPrograms { blur, material });
    let tone = compile_texture(
        renderer,
        "bar backdrop",
        backdrop::SHADER,
        &backdrop::UNIFORMS,
    );
    let genie = compile_texture(renderer, "genie", genie::SHADER, &genie::UNIFORMS);
    let bar_tone = compile_texture(
        renderer,
        "spaces bar tone",
        bar_material::TONE_SHADER,
        &bar_material::TONE_UNIFORMS,
    );
    let bar_blur = compile_texture(
        renderer,
        "spaces bar blur",
        bar_material::BLUR_SHADER,
        &bar_material::BLUR_UNIFORMS,
    );
    let bar = bar_tone
        .zip(bar_blur)
        .map(|(tone, blur)| bar_material::BarPrograms { tone, blur });
    let shaders = Shaders {
        corners,
        shadow,
        titlebar,
        border,
        glass,
        tone,
        genie,
        bar,
    };
    renderer
        .egl_context()
        .user_data()
        .insert_if_missing(|| shaders.clone());
    shaders
}

fn compile_texture(
    renderer: &mut GlesRenderer,
    name: &str,
    source: &str,
    uniforms: &[(&str, UniformType)],
) -> Option<GlesTexProgram> {
    let names: Vec<UniformName<'_>> = uniforms
        .iter()
        .map(|(name, type_)| UniformName::new(*name, *type_))
        .collect();
    match renderer.compile_custom_texture_shader(source, &names) {
        Ok(program) => Some(program),
        Err(err) => {
            warn!("shader {name} not compiled, effect disabled: {err:?}");
            None
        }
    }
}

fn compile_pixel(
    renderer: &mut GlesRenderer,
    name: &str,
    source: &str,
    uniforms: &[(&str, UniformType)],
) -> Option<GlesPixelProgram> {
    let names: Vec<UniformName<'_>> = uniforms
        .iter()
        .map(|(name, type_)| UniformName::new(*name, *type_))
        .collect();
    match renderer.compile_custom_pixel_shader(source, &names) {
        Ok(program) => Some(program),
        Err(err) => {
            warn!("shader {name} not compiled, effect disabled: {err:?}");
            None
        }
    }
}
