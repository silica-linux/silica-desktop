//! Avvio dell'embedder: `silica-embedder [--engine DIR] [--size LxA] BUNDLE`.
//!
//! `BUNDLE` è la cartella `flutter_assets` di `flutter build bundle`; `DIR` contiene
//! `libflutter_engine.so` e `icudtl.dat` (`cargo xtask engine`).

use std::path::PathBuf;
use std::process::ExitCode;

use silica_embedder::Options;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_writer(std::io::stderr)
        .init();
    let options = match parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("silica-embedder: {err}");
            return ExitCode::from(2);
        }
    };
    match silica_embedder::run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut engine_dir = std::env::var_os("SILICA_FLUTTER_ENGINE").map(PathBuf::from);
    let mut size = (800, 600);
    let mut bundle = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--engine" => {
                engine_dir = Some(args.next().ok_or("--engine richiede un percorso")?.into())
            }
            "--size" => {
                let text = args.next().ok_or("--size richiede LxA")?;
                let parse = |n: &str| n.parse::<u32>().ok().filter(|n| (1..=16384).contains(n));
                size = text
                    .split_once('x')
                    .and_then(|(w, h)| parse(w).zip(parse(h)))
                    .ok_or(format!("dimensione non valida: {text}"))?;
            }
            other if other.starts_with("--") => {
                return Err(format!("argomento sconosciuto: {other}"));
            }
            other => bundle = Some(PathBuf::from(other)),
        }
    }
    let engine_dir = engine_dir.unwrap_or_else(silica_embedder::default_engine_dir);
    Ok(Options {
        bundle: bundle.ok_or("manca la cartella del bundle (flutter_assets)")?,
        library: engine_dir.join("libflutter_engine.so"),
        icu_data: engine_dir.join("icudtl.dat"),
        size,
    })
}
