//! Embedder startup:
//!
//! ```text
//! silica-embedder [--engine DIR] [--size WxH] [--renderer software|gl|impeller|vulkan] [--aot LIBAPP]
//!     [--engine-arg ARG]... [--dart-arg ARG]... [--title TITLE] [--app-id ID] [--multi-window] [--helper] [--parent HANDLE] BUNDLE
//!     [--layer background|bottom|top|overlay --anchor top,left,right
//!      --exclusive-zone N --keyboard none|exclusive|on-demand --namespace NAME]
//!     [--view --layer … --size WxH … --namespace NAME]...
//! ```
//!
//! `BUNDLE` is the `flutter_assets` folder of `flutter build bundle`; `DIR` contains
//! `libflutter_engine.so` and `icudtl.dat` (`cargo xtask engine`). With a release engine
//! the AOT code is needed: `--aot`, or `libapp.so` next to `BUNDLE`
//! (`cargo xtask bundle <app> --release`). `--helper` starts a helper process of the app (the
//! portal's Open and Save dialog): not the app's running instance, and it ends with its window.
//! Every `--view` adds another shell layer to the same engine (#201): its own
//! wlr-layer-shell surface with the layer, anchor, size, exclusive zone, keyboard mode and
//! namespace that follow it; Dart draws it as one more view (`silica/layer` `views`).
//! `--parent HANDLE` (an xdg-foreign v2 handle, bare or `wayland:HANDLE`) makes another app's
//! window the parent of this one: the dialog is attached to the app that asked for it.

use std::path::PathBuf;
use std::process::ExitCode;

use silica_embedder::{
    Anchors, LayerKeyboard, LayerKind, LayerOptions, LayerView, Options, Renderer,
};
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
    let mut multi_window = false;
    let mut helper = false;
    let mut parent = None;
    // The layer groups closed by `--view`: the first is the main view, the others more views.
    let mut groups: Vec<(Option<LayerOptions>, (u32, u32))> = Vec::new();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--engine" => engine_dir = Some(value()?.into()),
            "--size" => {
                let text = value()?;
                let parse = |n: &str| n.parse::<u32>().ok().filter(|n| (1..=16384).contains(n));
                size = text
                    .split_once('x')
                    .and_then(|(w, h)| parse(w).zip(parse(h)))
                    .ok_or(format!("invalid size: {text}"))?;
            }
            "--layer" => {
                layer_options(&mut layer).layer = match value()?.as_str() {
                    "background" => LayerKind::Background,
                    "bottom" => LayerKind::Bottom,
                    "top" => LayerKind::Top,
                    "overlay" => LayerKind::Overlay,
                    other => return Err(format!("unknown layer: {other}")),
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
                        other => return Err(format!("unknown edge: {other}")),
                    }
                }
            }
            "--exclusive-zone" => {
                let zone = value()?;
                layer_options(&mut layer).exclusive_zone =
                    zone.parse().map_err(|_| format!("invalid zone: {zone}"))?;
            }
            "--keyboard" => {
                layer_options(&mut layer).keyboard = match value()?.as_str() {
                    "none" => LayerKeyboard::None,
                    "exclusive" => LayerKeyboard::Exclusive,
                    "on-demand" => LayerKeyboard::OnDemand,
                    other => return Err(format!("unknown keyboard mode: {other}")),
                };
            }
            "--namespace" => layer_options(&mut layer).namespace = value()?,
            // Another shell layer in the same engine (#201): the layer options after it are its.
            "--view" => {
                groups.push((layer.take(), size));
                size = (800, 600);
            }
            "--aot" => aot = Some(PathBuf::from(value()?)),
            "--engine-arg" => engine_args.push(value()?),
            "--dart-arg" => dart_args.push(value()?),
            "--title" => title = value()?,
            "--app-id" => app_id = value()?,
            "--multi-window" => multi_window = true,
            "--helper" => helper = true,
            "--parent" => parent = silica_embedder::parent_handle(&value()?),
            "--renderer" => {
                renderer = match value()?.as_str() {
                    "software" => Renderer::Software,
                    "gl" => Renderer::Gl,
                    "impeller" => Renderer::Impeller,
                    "vulkan" => Renderer::Vulkan,
                    other => return Err(format!("unknown renderer: {other}")),
                };
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown argument: {other}"));
            }
            // The first free argument is the bundle; the following ones belong to the app
            // (the files to open from the .desktop entry): they reach Dart as `--dart-arg`.
            other if bundle.is_none() => bundle = Some(PathBuf::from(other)),
            other => dart_args.push(other.to_owned()),
        }
    }
    groups.push((layer, size));
    let mut groups = groups.into_iter();
    let (layer, size) = groups.next().unwrap_or((None, size));
    let layer_views = groups
        .map(|(layer, size)| {
            layer
                .map(|layer| LayerView { layer, size })
                .ok_or("--view needs its own --layer")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !layer_views.is_empty() && layer.is_none() {
        return Err("--view is for shell layers: the main view needs --layer too".to_owned());
    }
    let engine_dir = engine_dir.unwrap_or_else(silica_embedder::default_engine_dir);
    let bundle: PathBuf = bundle.ok_or("the bundle folder (flutter_assets) is missing")?;
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
        layer_views,
        renderer,
        aot,
        engine_args,
        dart_args,
        title,
        app_id,
        multi_window,
        helper,
        parent,
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
    fn the_portal_dialog_is_a_helper() {
        // The portal appends `--helper` and the dialog's arguments after Mustache's Exec.
        let options = parse(args(&[
            "--app-id",
            "dev.frascella.Mustache",
            "--multi-window",
            "/bundle/flutter_assets",
            "--helper",
            "--dart-arg",
            "--dialog=open",
        ]))
        .unwrap();
        assert!(options.helper);
        assert!(options.multi_window);
        assert_eq!(options.dart_args, ["--dialog=open"]);
        let plain = parse(args(&["/bundle/flutter_assets"])).unwrap();
        assert!(!plain.helper);
        assert_eq!(plain.parent, None);
    }

    #[test]
    fn the_portal_dialog_gets_its_parent_window() {
        // The portal adds `--parent` with the requesting window's xdg-foreign handle.
        let options = parse(args(&[
            "/bundle/flutter_assets",
            "--helper",
            "--parent",
            "Zx81handle",
            "--dart-arg",
            "--dialog=open",
        ]))
        .unwrap();
        assert_eq!(options.parent.as_deref(), Some("Zx81handle"));
        assert_eq!(options.dart_args, ["--dialog=open"]);
        let prefixed = parse(args(&["/b", "--parent", "wayland:abc"])).unwrap();
        assert_eq!(prefixed.parent.as_deref(), Some("abc"));
        let empty = parse(args(&["/b", "--parent", ""])).unwrap();
        assert_eq!(empty.parent, None);
        assert!(parse(args(&["/b", "--parent"])).is_err());
    }

    #[test]
    fn every_view_is_a_layer_of_its_own() {
        let options = parse(args(&[
            "--layer",
            "background",
            "--anchor",
            "top,bottom,left,right",
            "--size",
            "1280x800",
            "--namespace",
            "wallpaper",
            "--view",
            "--layer",
            "top",
            "--anchor",
            "top,left,right",
            "--size",
            "1280x28",
            "--exclusive-zone",
            "28",
            "--namespace",
            "menubar",
            "--view",
            "--layer",
            "overlay",
            "--keyboard",
            "on-demand",
            "--namespace",
            "faro",
            "--dart-arg",
            "shell",
            "/bundle/flutter_assets",
        ]))
        .unwrap();
        let main = options.layer.unwrap();
        assert_eq!(main.namespace, "wallpaper");
        assert_eq!(options.size, (1280, 800));
        assert_eq!(options.layer_views.len(), 2);
        let bar = &options.layer_views[0];
        assert_eq!(bar.layer.namespace, "menubar");
        assert_eq!(bar.layer.exclusive_zone, 28);
        assert!(bar.layer.anchor.top && !bar.layer.anchor.bottom);
        assert_eq!(bar.size, (1280, 28));
        let faro = &options.layer_views[1];
        assert_eq!(faro.layer.layer, LayerKind::Overlay);
        assert_eq!(faro.layer.keyboard, LayerKeyboard::OnDemand);
        assert_eq!(faro.size, (800, 600));
        assert_eq!(options.dart_args, ["shell"]);
        // A view without its layer, or views without a main layer, are refused.
        assert!(parse(args(&["--layer", "top", "--view", "/b"])).is_err());
        assert!(parse(args(&["--view", "--layer", "top", "/b"])).is_err());
        assert!(parse(args(&["/b"])).unwrap().layer_views.is_empty());
    }

    #[test]
    fn arguments_after_the_bundle_go_to_the_app() {
        // A .desktop entry without field codes receives the files at the end: before, the
        // bundle was replaced by the last path and the app did not start (Mustache's ⌘N).
        let options = parse(args(&[
            "--engine",
            "/e",
            "--dart-arg",
            "--one",
            "/bundle/flutter_assets",
            "/home/me",
            "/home/me/note.txt",
        ]))
        .unwrap();
        assert_eq!(options.bundle, PathBuf::from("/bundle/flutter_assets"));
        assert_eq!(
            options.dart_args,
            ["--one", "/home/me", "/home/me/note.txt"]
        );
    }
}
