//! Renderer GLES di Ossido e shader del vetro.
//!
//! Gli effetti sono elementi di rendering di smithay che Ossido mette nella lista
//! dell'output. I parametri vengono dalle misure di macOS 27 in `plan/reference/`, citate
//! accanto a ogni costante.

pub mod animation;
pub mod cache;
pub mod corners;
pub mod glass;
pub mod shadow;
pub mod titlebar;

use smithay::backend::renderer::gles::{
    GlesPixelProgram, GlesRenderer, GlesTexProgram, UniformName, UniformType,
};
use tracing::warn;

/// I programmi compilati per un renderer. Se uno shader non compila, l'effetto manca ma
/// Ossido continua a disegnare (mai crashare).
#[derive(Clone, Default)]
pub struct Shaders {
    pub corners: Option<GlesTexProgram>,
    pub shadow: Option<GlesPixelProgram>,
    pub titlebar: Option<GlesPixelProgram>,
    pub glass: Option<glass::GlassPrograms>,
}

/// I programmi del renderer, compilati alla prima richiesta e tenuti nel contesto EGL.
pub fn shaders(renderer: &mut GlesRenderer) -> Shaders {
    if let Some(shaders) = renderer.egl_context().user_data().get::<Shaders>() {
        return shaders.clone();
    }
    let corners = compile_texture(renderer, "angoli", corners::SHADER, &corners::UNIFORMS);
    let shadow = compile_pixel(renderer, "ombra", shadow::SHADER, &shadow::UNIFORMS);
    let titlebar = compile_pixel(
        renderer,
        "barra del titolo",
        titlebar::SHADER,
        &titlebar::UNIFORMS,
    );
    let blur = compile_texture(
        renderer,
        "sfocatura",
        glass::BLUR_SHADER,
        &glass::BLUR_UNIFORMS,
    );
    let material = compile_texture(
        renderer,
        "materiale",
        glass::MATERIAL_SHADER,
        &glass::MATERIAL_UNIFORMS,
    );
    let glass = blur
        .zip(material)
        .map(|(blur, material)| glass::GlassPrograms { blur, material });
    let shaders = Shaders {
        corners,
        shadow,
        titlebar,
        glass,
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
            warn!("shader {name} non compilato, effetto disattivato: {err:?}");
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
            warn!("shader {name} non compilato, effetto disattivato: {err:?}");
            None
        }
    }
}
