//! Flutter per l'embedder: engine, binding, bundle delle app.
//!
//! - `engine`: scarica `libflutter_engine.so` e `icudtl.dat` per `crates/embedder/ENGINE_VERSION`.
//! - `bindings`: rigenera `crates/embedder/src/ffi/bindings.rs` con bindgen.
//! - `bundle <app>`: `flutter build bundle` per un'app in `apps/`.
//! - `apps`/`check`: usati da `cargo xtask ci` per `flutter analyze` e `flutter test`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Result, root, status};

const STORAGE: &str = "https://storage.googleapis.com/flutter_infra_release/flutter";

pub fn engine_version() -> Result<String> {
    let path = root()?.join("crates/embedder/ENGINE_VERSION");
    std::fs::read_to_string(&path)
        .map(|v| v.trim().to_owned())
        .map_err(|e| format!("{} illeggibile: {e}", path.display()))
}

pub fn engine_dir() -> Result<PathBuf> {
    Ok(root()?
        .join("target/flutter-engine")
        .join(engine_version()?))
}

/// La radice dell'SDK Flutter: `FLUTTER_ROOT`, oppure da `flutter` nel PATH.
fn sdk_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("FLUTTER_ROOT") {
        return Ok(PathBuf::from(root));
    }
    let output = Command::new("sh")
        .args(["-c", "command -v flutter"])
        .output()
        .map_err(|e| format!("sh non avviabile: {e}"))?;
    let flutter = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if flutter.is_empty() {
        return Err("SDK Flutter non trovato: installalo o imposta FLUTTER_ROOT".to_owned());
    }
    std::fs::canonicalize(&flutter)
        .ok()
        .and_then(|bin| bin.parent()?.parent().map(Path::to_path_buf))
        .ok_or_else(|| format!("radice dell'SDK non ricavabile da {flutter}"))
}

/// `PATH` per i comandi Flutter: aggiunge lo shim di `unzip` se manca quello vero.
fn tool_path() -> Result<String> {
    let path = std::env::var("PATH").unwrap_or_default();
    let has_unzip = Command::new("sh")
        .args(["-c", "command -v unzip"])
        .output()
        .is_ok_and(|o| o.status.success());
    if has_unzip {
        return Ok(path);
    }
    let tools = root()?.join("xtask/tools");
    Ok(format!("{}:{path}", tools.display()))
}

fn flutter_command(args: &[&str]) -> Result<Command> {
    let mut command = Command::new(sdk_root()?.join("bin/flutter"));
    command.args(args).env("PATH", tool_path()?);
    Ok(command)
}

pub fn engine() -> Result<()> {
    let version = engine_version()?;
    let sdk = sdk_root()?;
    let sdk_version = std::fs::read_to_string(sdk.join("bin/internal/engine.version"))
        .map(|v| v.trim().to_owned())
        .map_err(|e| format!("versione dell'engine dell'SDK illeggibile: {e}"))?;
    if sdk_version != version {
        return Err(format!(
            "l'SDK Flutter usa l'engine {sdk_version}, il repo {version}: i bundle compilati \
             dall'SDK girano solo sull'engine dello stesso SDK. Aggiorna \
             crates/embedder/ENGINE_VERSION e rigenera con `cargo xtask bindings`."
        ));
    }

    let dir = engine_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !dir.join("libflutter_engine.so").exists() {
        let zip = dir.join("embedder.zip");
        let mut curl = Command::new("curl");
        curl.args(["-sSfL", "-o"]).arg(&zip).arg(format!(
            "{STORAGE}/{version}/linux-x64/linux-x64-embedder.zip"
        ));
        status(curl)?;
        let mut unzip = Command::new("unzip");
        unzip
            .args(["-o", "-q"])
            .arg(&zip)
            .arg("-d")
            .arg(&dir)
            .env("PATH", tool_path()?);
        status(unzip)?;
        let _ = std::fs::remove_file(&zip);
    }

    let icu = sdk.join("bin/cache/artifacts/engine/linux-x64/icudtl.dat");
    if !icu.exists() {
        status(flutter_command(&[
            "precache",
            "--linux",
            "--no-android",
            "--no-ios",
            "--no-web",
        ])?)?;
    }
    std::fs::copy(&icu, dir.join("icudtl.dat")).map_err(|e| format!("icudtl.dat: {e}"))?;
    eprintln!("xtask: engine {version} in {}", dir.display());
    Ok(())
}

pub fn bindings() -> Result<()> {
    let header = engine_dir()?.join("flutter_embedder.h");
    if !header.exists() {
        return Err("header assente: prima `cargo xtask engine`".to_owned());
    }
    let out = root()?.join("crates/embedder/src/ffi/bindings.rs");
    let mut bindgen = Command::new("bindgen");
    bindgen
        .arg(&header)
        .args([
            "--allowlist-function",
            "Flutter.*",
            "--allowlist-type",
            "Flutter.*",
            "--allowlist-var",
            "FLUTTER_.*|kFlutter.*",
            "--dynamic-loading",
            "FlutterEngineLib",
            "--default-enum-style",
            "newtype",
            "--with-derive-default",
            "--no-layout-tests",
            "--no-doc-comments",
            "--raw-line",
            "// Generato da `cargo xtask bindings` da flutter_embedder.h (BSD, The Flutter Authors).",
            "--raw-line",
            &format!("// Engine: {}. Non modificare a mano.", engine_version()?),
            "-o",
        ])
        .arg(&out);
    status(bindgen)?;
    status({
        let mut fmt = Command::new("rustfmt");
        fmt.args(["--edition", "2024"]).arg(&out);
        fmt
    })
}

pub fn bundle(app: &str) -> Result<()> {
    let dir = root()?.join("apps").join(app);
    if !dir.join("pubspec.yaml").exists() {
        return Err(format!("{} non è un'app Flutter", dir.display()));
    }
    let mut command = flutter_command(&["build", "bundle", "--debug"])?;
    command.current_dir(&dir);
    status(command)?;
    eprintln!(
        "xtask: bundle in {}",
        dir.join("build/flutter_assets").display()
    );
    Ok(())
}

/// Le app Flutter in `apps/` (cartelle con `pubspec.yaml`).
pub fn apps() -> Result<Vec<String>> {
    let dir = root()?.join("apps");
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut apps: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().join("pubspec.yaml").exists())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    apps.sort();
    Ok(apps)
}

/// `flutter analyze` e `flutter test` per un'app in `apps/`.
pub fn check(app: &str) -> Result<()> {
    let dir = root()?.join("apps").join(app);
    for args in [["analyze", "--fatal-infos"].as_slice(), ["test"].as_slice()] {
        let mut command = flutter_command(args)?;
        command.current_dir(&dir);
        status(command)?;
    }
    Ok(())
}
