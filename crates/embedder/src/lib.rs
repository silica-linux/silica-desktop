//! Embedder Flutter in Rust: un client Wayland che ospita l'engine Flutter.
//!
//! - Il thread principale è il thread di piattaforma: gira il ciclo Wayland (calloop) ed
//!   esegue i task che l'engine vi accoda.
//! - L'engine disegna sul suo thread raster con OpenGL ES (EGL sulla `wl_surface`, lo
//!   swap fa il commit) oppure in software: ogni frame arriva qui per canale e va in un
//!   buffer `wl_shm`. Il software è il default perché gira ovunque, anche su Ossido headless.
//! - Gli eventi del puntatore diventano `FlutterPointerEvent` in pixel fisici; i tasti
//!   `FlutterKeyEvent`, e quelli che il framework non consuma vanno al campo di testo
//!   attivo (`flutter/textinput`).

mod codec;
mod ffi;
mod glass;
mod keyboard;
mod keymap;
mod text_input;
mod window;

use std::path::PathBuf;

pub use window::run;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Background,
    Bottom,
    Top,
    Overlay,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Anchors {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LayerKeyboard {
    #[default]
    None,
    Exclusive,
    OnDemand,
}

/// L'app come superficie della shell (wlr-layer-shell) invece che come finestra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerOptions {
    pub layer: LayerKind,
    pub anchor: Anchors,
    /// Pixel logici riservati sul bordo ancorato; le finestre non ci vanno sopra.
    pub exclusive_zone: i32,
    pub keyboard: LayerKeyboard,
    pub namespace: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Renderer {
    #[default]
    Software,
    /// OpenGL ES con Skia.
    Gl,
    /// OpenGL ES con Impeller, il renderer della shell (gate 1). Su un driver GL software
    /// (llvmpipe) Impeller va in crash nell'engine: lì si ripiega su Skia.
    Impeller,
}

/// Driver GL che rasterizzano sulla CPU, dal `GL_RENDERER` di Mesa.
pub fn is_software_gl(renderer: &str) -> bool {
    let renderer = renderer.to_ascii_lowercase();
    ["llvmpipe", "softpipe", "swrast", "lavapipe"]
        .iter()
        .any(|name| renderer.contains(name))
}

pub struct Options {
    /// Cartella `flutter_assets` del bundle.
    pub bundle: PathBuf,
    pub library: PathBuf,
    pub icu_data: PathBuf,
    /// Dimensione iniziale della finestra in pixel logici, se il compositor non ne sceglie una.
    /// Per un layer, i lati ancorati su entrambi i bordi li decide il compositor.
    pub size: (u32, u32),
    pub layer: Option<LayerOptions>,
    pub renderer: Renderer,
    /// `libapp.so` compilato AOT, per un engine release; con un engine debug si ignora.
    pub aot: Option<PathBuf>,
    /// Opzioni passate all'engine così come sono (`--engine-arg`).
    pub engine_args: Vec<String>,
    /// Argomenti di `main(List<String> args)` dell'app (`--dart-arg`).
    pub dart_args: Vec<String>,
}

/// `target/flutter-engine/<ENGINE_VERSION>` del workspace, dove lo mette `cargo xtask engine`.
pub fn default_engine_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/flutter-engine")
        .join(include_str!("../ENGINE_VERSION").trim())
}

/// L'engine release compilato da `cargo xtask engine --release` (`docs/engine-release.md`).
pub fn release_engine_dir() -> PathBuf {
    let mut dir = default_engine_dir().into_os_string();
    dir.push("-release");
    PathBuf::from(dir)
}

#[cfg(test)]
mod tests {
    use super::is_software_gl;

    #[test]
    fn software_gl_drivers_are_recognised() {
        assert!(is_software_gl("llvmpipe (LLVM 21.1.8, 256 bits)"));
        assert!(is_software_gl("softpipe"));
        assert!(!is_software_gl("Mesa Intel(R) Iris(R) Xe Graphics (RPL-P)"));
        assert!(!is_software_gl(
            "D3D12 (NVIDIA GeForce RTX 4070 Laptop GPU)"
        ));
        assert!(!is_software_gl(""));
    }
}
