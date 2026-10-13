//! Avvio dell'embedder:
//!
//! ```text
//! silica-embedder [--engine DIR] [--size LxA] [--renderer software|gl|impeller|vulkan] [--aot LIBAPP]
//!     [--engine-arg ARG]... [--dart-arg ARG]... [--title TITOLO] [--app-id ID] BUNDLE
//!     [--layer background|bottom|top|overlay --anchor top,left,right
//!      --exclusive-zone N --keyboard none|exclusive|on-demand --namespace NOME]
//! ```
//!
//! `BUNDLE` è la cartella `flutter_assets` di `flutter build bundle`; `DIR` contiene
//! `libflutter_engine.so` e `icudtl.dat` (`cargo xtask engine`). Con un engine release
//! serve il codice AOT: `--aot`, oppure `libapp.so` accanto a `BUNDLE`
//! (`cargo xtask bundle <app> --release`).

use std::path::PathBuf;
use std::process::ExitCode;

use silica_embedder::{Anchors, LayerKeyboard, LayerKind, LayerOptions, Options, Renderer};
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

fn layer_options(layer: &mut Option<LayerOptions>) -> &mut LayerOptions {
    layer.get_or_insert_with(|| LayerOptions {
        layer: LayerKind::Top,
        anchor: Anchors::default(),
        exclusive_zone: 0,
        keyboard: LayerKeyboard::None,
        namespace: "silica".to_owned(),
    })
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut engine_dir = std::env::var_os("SILICA_FLUTTER_ENGINE").map(PathBuf::from);
    let mut size = (800, 600);
    let mut bundle = None;
    let mut layer: Option<LayerOptions> = None;
    let mut renderer = Renderer::default();
    let mut aot = None;
    let mut engine_args = Vec::new();
    let mut dart_args = Vec::new();
    let mut title = "Silica".to_owned();
    let mut app_id = "dev.frascella.silica.Flutter".to_owned();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} richiede un valore"));
        match arg.as_str() {
            "--engine" => engine_dir = Some(value()?.into()),
            "--size" => {
                let text = value()?;
                let parse = |n: &str| n.parse::<u32>().ok().filter(|n| (1..=16384).contains(n));
                size = text
                    .split_once('x')
                    .and_then(|(w, h)| parse(w).zip(parse(h)))
                    .ok_or(format!("dimensione non valida: {text}"))?;
            }
            "--layer" => {
                layer_options(&mut layer).layer = match value()?.as_str() {
                    "background" => LayerKind::Background,
                    "bottom" => LayerKind::Bottom,
                    "top" => LayerKind::Top,
                    "overlay" => LayerKind::Overlay,
                    other => return Err(format!("livello sconosciuto: {other}")),
                };
            }
            "--anchor" => {
                let edges = value()?;
                let anchor = &mut layer_options(&mut layer).anchor;
                for edge in edges.split(',') {
                    match edge.trim() {
                        "top" => anchor.top = true,
                        "bottom" => anchor.bottom = true,
                        "left" => anchor.left = true,
                        "right" => anchor.right = true,
                        other => return Err(format!("bordo sconosciuto: {other}")),
                    }
                }
            }
            "--exclusive-zone" => {
                let zone = value()?;
                layer_options(&mut layer).exclusive_zone = zone
                    .parse()
                    .map_err(|_| format!("zona non valida: {zone}"))?;
            }
            "--keyboard" => {
                layer_options(&mut layer).keyboard = match value()?.as_str() {
                    "none" => LayerKeyboard::None,
                    "exclusive" => LayerKeyboard::Exclusive,
                    "on-demand" => LayerKeyboard::OnDemand,
                    other => return Err(format!("tastiera sconosciuta: {other}")),
                };
            }
            "--namespace" => layer_options(&mut layer).namespace = value()?,
            "--aot" => aot = Some(PathBuf::from(value()?)),
            "--engine-arg" => engine_args.push(value()?),
            "--dart-arg" => dart_args.push(value()?),
            "--title" => title = value()?,
            "--app-id" => app_id = value()?,
            "--renderer" => {
                renderer = match value()?.as_str() {
                    "software" => Renderer::Software,
                    "gl" => Renderer::Gl,
                    "impeller" => Renderer::Impeller,
                    "vulkan" => Renderer::Vulkan,
                    other => return Err(format!("renderer sconosciuto: {other}")),
                };
            }
            other if other.starts_with("--") => {
                return Err(format!("argomento sconosciuto: {other}"));
            }
            // Il primo argomento libero è il bundle; i successivi sono dell'app (i file da
            // aprire dalla voce .desktop): arrivano a Dart come `--dart-arg`.
            other if bundle.is_none() => bundle = Some(PathBuf::from(other)),
            other => dart_args.push(other.to_owned()),
        }
    }
    let engine_dir = engine_dir.unwrap_or_else(silica_embedder::default_engine_dir);
    let bundle: PathBuf = bundle.ok_or("manca la cartella del bundle (flutter_assets)")?;
    let aot = aot.or_else(|| {
        let sibling = bundle.parent()?.join("libapp.so");
        sibling.exists().then_some(sibling)
    });
    Ok(Options {
        bundle,
        library: engine_dir.join("libflutter_engine.so"),
        icu_data: engine_dir.join("icudtl.dat"),
        size,
        layer,
        renderer,
        aot,
        engine_args,
        dart_args,
        title,
        app_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn arguments_after_the_bundle_go_to_the_app() {
        // Una voce .desktop senza codici di campo riceve i file in fondo: prima il bundle
        // veniva sostituito dall'ultimo percorso e l'app non partiva (⌘N di Mustache).
        let options = parse(args(&[
            "--engine",
            "/e",
            "--dart-arg",
            "--uno",
            "/bundle/flutter_assets",
            "/home/io",
            "/home/io/nota.txt",
        ]))
        .unwrap();
        assert_eq!(options.bundle, PathBuf::from("/bundle/flutter_assets"));
        assert_eq!(
            options.dart_args,
            ["--uno", "/home/io", "/home/io/nota.txt"]
        );
    }
}
