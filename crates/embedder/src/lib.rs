//! Embedder Flutter in Rust: un client Wayland che ospita l'engine Flutter.
//!
//! - Il thread principale è il thread di piattaforma: gira il ciclo Wayland (calloop) ed
//!   esegue i task che l'engine vi accoda.
//! - L'engine disegna con il renderer software sul suo thread raster; ogni frame arriva
//!   qui per canale e va in un buffer `wl_shm`. Il renderer GL arriva dopo il gate 1.
//! - Gli eventi del puntatore diventano `FlutterPointerEvent` in pixel fisici.

mod ffi;
mod window;

use std::path::PathBuf;

pub use window::run;

pub struct Options {
    /// Cartella `flutter_assets` del bundle.
    pub bundle: PathBuf,
    pub library: PathBuf,
    pub icu_data: PathBuf,
    /// Dimensione iniziale della finestra in pixel logici, se il compositor non ne sceglie una.
    pub size: (u32, u32),
}

/// `target/flutter-engine/<ENGINE_VERSION>` del workspace, dove lo mette `cargo xtask engine`.
pub fn default_engine_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/flutter-engine")
        .join(include_str!("../ENGINE_VERSION").trim())
}
