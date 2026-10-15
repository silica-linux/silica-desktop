//! Flutter embedder in Rust: a Wayland client that hosts the Flutter engine.
//!
//! - The main thread is the platform thread: it runs the Wayland loop (calloop) and
//!   executes the tasks the engine queues on it.
//! - The engine draws on its raster thread with OpenGL ES (EGL on the `wl_surface`, the
//!   swap commits) or in software: every frame arrives here over a channel and goes into a
//!   `wl_shm` buffer. Software is the default because it runs everywhere, even on headless Ossido.
//! - Pointer events become `FlutterPointerEvent` in physical pixels; keys become
//!   `FlutterKeyEvent`, and those the framework does not consume go to the active text
//!   field (`flutter/textinput`).

mod app_lifecycle;
mod appearance;
mod clipboard;
mod codec;
pub mod dbus;
mod drag;
mod ffi;
mod glass;
mod keyboard;
mod keymap;
mod locale;
mod mouse_cursor;
mod pty;
mod shell;
mod termgrid;
mod text_input;
mod trace;
mod window;
mod window_appearance;

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

/// The app as a shell surface (wlr-layer-shell) instead of a window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerOptions {
    pub layer: LayerKind,
    pub anchor: Anchors,
    /// Logical pixels reserved on the anchored edge; windows do not go over them.
    pub exclusive_zone: i32,
    pub keyboard: LayerKeyboard,
    pub namespace: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Renderer {
    #[default]
    Software,
    /// OpenGL ES with Skia.
    Gl,
    /// OpenGL ES with Impeller, the shell renderer (gate 1). On a software GL driver
    /// (llvmpipe) Impeller crashes inside the engine: there it falls back to Skia.
    Impeller,
    /// Vulkan with Impeller (swapchain on the Wayland surface). Without Vulkan it falls back to
    /// OpenGL ES with Impeller.
    Vulkan,
}

/// GL drivers that rasterize on the CPU, from Mesa's `GL_RENDERER`.
pub fn is_software_gl(renderer: &str) -> bool {
    let renderer = renderer.to_ascii_lowercase();
    ["llvmpipe", "softpipe", "swrast", "lavapipe"]
        .iter()
        .any(|name| renderer.contains(name))
}

pub struct Options {
    /// The `flutter_assets` folder of the bundle.
    pub bundle: PathBuf,
    pub library: PathBuf,
    pub icu_data: PathBuf,
    /// Initial window size in logical pixels, if the compositor does not choose one.
    /// For a layer, the sides anchored on both edges are decided by the compositor.
    pub size: (u32, u32),
    pub layer: Option<LayerOptions>,
    pub renderer: Renderer,
    /// AOT-compiled `libapp.so`, for a release engine; ignored with a debug engine.
    pub aot: Option<PathBuf>,
    /// Options passed to the engine as they are (`--engine-arg`).
    pub engine_args: Vec<String>,
    /// Arguments of the app's `main(List<String> args)` (`--dart-arg`).
    pub dart_args: Vec<String>,
    /// Initial window title (`--title`); the app changes it with
    /// `SystemChrome.setApplicationSwitcherDescription`.
    pub title: String,
    /// Window app id (`--app-id`): ties the window to its `.desktop` entry.
    pub app_id: String,
}

/// `target/flutter-engine/<ENGINE_VERSION>` of the workspace, where `cargo xtask engine` puts it.
pub fn default_engine_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/flutter-engine")
        .join(include_str!("../ENGINE_VERSION").trim())
}

/// The release engine built by `cargo xtask engine --release` (`docs/engine-release.md`).
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
