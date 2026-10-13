//! Flutter per l'embedder: engine, binding, bundle delle app.
//!
//! - `engine`: scarica `libflutter_engine.so` e `icudtl.dat` per `crates/embedder/ENGINE_VERSION`.
//! - `engine --release`: compila l'engine release dai sorgenti (`docs/engine-release.md`):
//!   Google pubblica l'engine per embedder custom solo in debug.
//! - `bindings`: rigenera `crates/embedder/src/ffi/bindings.rs` con bindgen.
//! - `bundle <shell | demo | percorso> [--release]`: il bundle della shell, dell'app di prova
//!   o di un'app esterna (il suo repository); in release anche `libapp.so`.
//! - `projects`/`check`: usati da `cargo xtask ci` per `flutter analyze` e `flutter test`.

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

/// L'engine release compilato da `engine --release`.
pub fn release_engine_dir() -> Result<PathBuf> {
    Ok(root()?
        .join("target/flutter-engine")
        .join(format!("{}-release", engine_version()?)))
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

/// Compila `libflutter_engine.so` release dal checkout in `SILICA_ENGINE_SRC`, già
/// preparato con gclient alla revisione di `ENGINE_VERSION` (`docs/engine-release.md`).
pub fn engine_release() -> Result<()> {
    let version = engine_version()?;
    let source = std::env::var_os("SILICA_ENGINE_SRC")
        .map(PathBuf::from)
        .ok_or(
            "SILICA_ENGINE_SRC non impostata: il checkout dell'engine si prepara come in \
         docs/engine-release.md",
        )?;
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&source)
        .output()
        .map_err(|e| format!("git non avviabile: {e}"))?;
    let head = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    if head != version {
        return Err(format!(
            "{} è alla revisione {head}, il repo vuole {version}",
            source.display()
        ));
    }
    let src = source.join("engine/src");
    let ninja = source.join("third_party/ninja/ninja");
    if !ninja.exists() {
        return Err(format!("{} assente: serve `gclient sync`", ninja.display()));
    }

    // Gli script della build chiamano vpython3 di depot_tools: basta python3.
    let shims = root()?.join("target/xtask-shims");
    std::fs::create_dir_all(&shims).map_err(|e| format!("{}: {e}", shims.display()))?;
    let vpython = shims.join("vpython3");
    if !vpython.exists() {
        std::os::unix::fs::symlink("/usr/bin/python3", &vpython)
            .map_err(|e| format!("{}: {e}", vpython.display()))?;
    }
    let path = format!(
        "{}:{}",
        shims.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let mut gn = Command::new("python3");
    gn.arg("flutter/tools/gn")
        .args([
            "--runtime-mode",
            "release",
            "--no-rbe",
            "--no-goma",
            "--no-enable-unittests",
        ])
        .current_dir(&src)
        .env("PATH", &path);
    status(gn)?;
    let mut build = Command::new(&ninja);
    build
        .args([
            "-C",
            "out/host_release",
            "flutter/shell/platform/embedder:flutter_engine",
        ])
        .current_dir(&src)
        .env("PATH", &path);
    status(build)?;

    let dir = release_engine_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::copy(
        src.join("out/host_release/libflutter_engine.so"),
        dir.join("libflutter_engine.so"),
    )
    .map_err(|e| format!("libflutter_engine.so: {e}"))?;
    std::fs::copy(engine_dir()?.join("icudtl.dat"), dir.join("icudtl.dat"))
        .map_err(|e| format!("icudtl.dat (prima `cargo xtask engine`): {e}"))?;
    eprintln!("xtask: engine release {version} in {}", dir.display());
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

/// La cartella di un progetto Flutter da costruire: `shell` è la shell, `demo` l'app di
/// prova dei test dell'embedder, altrimenti il percorso di un'app esterna (le app di Silica
/// stanno nei loro repository).
fn app_dir(app: &str) -> Result<PathBuf> {
    let root = root()?;
    Ok(match app {
        "shell" => root.join("shell"),
        "demo" => root.join("testapps/demo"),
        path => std::path::absolute(path).map_err(|e| format!("{path}: {e}"))?,
    })
}

pub fn bundle(app: &str) -> Result<()> {
    let dir = app_dir(app)?;
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

/// Bundle release in `build/release/`: asset e `libapp.so`, il Dart compilato AOT con
/// il frontend e il gen_snapshot dell'SDK (gli stessi passi di `flutter build linux`).
pub fn bundle_release(app: &str) -> Result<()> {
    let dir = app_dir(app)?;
    if !dir.join("pubspec.yaml").exists() {
        return Err(format!("{} non è un'app Flutter", dir.display()));
    }
    let out = dir.join("build/release");
    let mut assets = flutter_command(&["build", "bundle", "--release", "--asset-dir"])?;
    assets.arg(out.join("flutter_assets")).current_dir(&dir);
    status(assets)?;

    let cache = sdk_root()?.join("bin/cache");
    let package = app_package(&dir)?;
    let dill = out.join("app.dill");
    let mut kernel = Command::new(cache.join("dart-sdk/bin/dartaotruntime"));
    kernel
        .arg(cache.join("dart-sdk/bin/snapshots/frontend_server_aot.dart.snapshot"))
        .arg("--sdk-root")
        .arg(cache.join("artifacts/engine/common/flutter_patched_sdk_product/"))
        .args([
            "--target=flutter",
            "--aot",
            "--tfa",
            "--no-print-incremental-dependencies",
            "-Ddart.vm.product=true",
            "-Ddart.vm.profile=false",
            "--packages",
            ".dart_tool/package_config.json",
            "--output-dill",
        ])
        .arg(&dill)
        .arg(format!("package:{package}/main.dart"))
        .current_dir(&dir);
    status(kernel)?;

    let mut snapshot = Command::new(cache.join("artifacts/engine/linux-x64-release/gen_snapshot"));
    snapshot
        .args(["--deterministic", "--snapshot_kind=app-aot-elf", "--strip"])
        .arg(format!("--elf={}", out.join("libapp.so").display()))
        .arg(&dill);
    status(snapshot)?;
    let _ = std::fs::remove_file(&dill);
    eprintln!("xtask: bundle release in {}", out.display());
    Ok(())
}

/// Il nome del pacchetto Dart dal `pubspec.yaml`.
fn app_package(dir: &Path) -> Result<String> {
    let pubspec = std::fs::read_to_string(dir.join("pubspec.yaml"))
        .map_err(|e| format!("pubspec.yaml: {e}"))?;
    pubspec
        .lines()
        .find_map(|line| line.strip_prefix("name:"))
        .map(|name| name.trim().to_owned())
        .ok_or_else(|| "pubspec.yaml senza name".to_owned())
}

/// I progetti Dart del repo: la shell e le app di prova in `testapps/`, come percorsi
/// relativi (`shell`, `testapps/demo`). `silica_ui` e le app hanno i loro repository.
pub fn projects() -> Result<Vec<String>> {
    let root = root()?;
    let mut projects = Vec::new();
    if root.join("shell/pubspec.yaml").exists() {
        projects.push("shell".to_owned());
    }
    let dir = root.join("testapps");
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    projects.extend(
        entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().join("pubspec.yaml").exists())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .map(|name| format!("testapps/{name}")),
    );
    projects.sort();
    Ok(projects)
}

/// `flutter analyze` e `flutter test` per un progetto di [`projects`].
pub fn check(project: &str) -> Result<()> {
    let dir = root()?.join(project);
    for args in [["analyze", "--fatal-infos"].as_slice(), ["test"].as_slice()] {
        let mut command = flutter_command(args)?;
        command.current_dir(&dir);
        status(command)?;
    }
    Ok(())
}

/// Estrae una mappa `chiave: Tipo.nome,` da `keyboard_maps.g.dart` e risolve i nomi.
fn dart_map(
    maps: &str,
    map_name: &str,
    values: &std::collections::HashMap<String, u64>,
) -> Result<Vec<(u32, u64)>> {
    let start = maps
        .find(&format!("{map_name} = "))
        .ok_or_else(|| format!("{map_name} assente"))?;
    let body = maps
        .get(start..)
        .and_then(|rest| rest.split_once("};"))
        .map(|(body, _)| body)
        .ok_or_else(|| format!("{map_name} non chiusa"))?;
    let mut entries = Vec::new();
    for line in body.lines().skip(1) {
        let Some((key, value)) = line.trim().trim_end_matches(',').split_once(':') else {
            continue;
        };
        let key = key.trim();
        let key = match key.strip_prefix("0x") {
            Some(hex) => u32::from_str_radix(hex, 16),
            None => key.parse(),
        }
        .map_err(|e| format!("{map_name}: chiave {key}: {e}"))?;
        let name = value
            .trim()
            .rsplit('.')
            .next()
            .ok_or_else(|| format!("{map_name}: valore {value}"))?;
        let code = values
            .get(name)
            .ok_or_else(|| format!("{map_name}: {name} non definito"))?;
        entries.push((key, *code));
    }
    entries.sort_unstable();
    entries.dedup_by_key(|(key, _)| *key);
    Ok(entries)
}

/// Rigenera `crates/embedder/src/keymap.rs` dalle tabelle dell'SDK Flutter.
pub fn keymap() -> Result<()> {
    let services = sdk_root()?.join("packages/flutter/lib/src/services");
    let read = |name: &str| {
        std::fs::read_to_string(services.join(name)).map_err(|e| format!("{name}: {e}"))
    };
    let keys = read("keyboard_key.g.dart")?;
    let maps = read("keyboard_maps.g.dart")?;

    // `static const LogicalKeyboardKey nome = LogicalKeyboardKey(0x...);`
    let mut logical = std::collections::HashMap::new();
    let mut physical = std::collections::HashMap::new();
    for line in keys.lines() {
        let Some(rest) = line.trim().strip_prefix("static const ") else {
            continue;
        };
        let Some((kind, rest)) = rest.split_once(' ') else {
            continue;
        };
        let Some((name, value)) = rest.split_once(" = ") else {
            continue;
        };
        let Some(hex) = value.split_once("(0x").and_then(|(_, v)| v.split_once(')')) else {
            continue;
        };
        let Ok(code) = u64::from_str_radix(hex.0, 16) else {
            continue;
        };
        match kind {
            "LogicalKeyboardKey" => logical.insert(name.to_owned(), code),
            "PhysicalKeyboardKey" => physical.insert(name.to_owned(), code),
            _ => None,
        };
    }

    let table = |name: &str, doc: &str, entries: &[(u32, u64)]| {
        let rows: String = entries
            .iter()
            .map(|(key, value)| format!("    (0x{key:08x}, 0x{value:011x}),\n"))
            .collect();
        format!("/// {doc}\npub const {name}: &[(u32, u64)] = &[\n{rows}];\n")
    };
    let out = format!(
        "//! Generato da `cargo xtask keymap` da keyboard_maps.g.dart dell'SDK Flutter\n\
         //! (BSD, The Flutter Authors). Non modificare a mano.\n\n{}\n{}\n{}",
        table(
            "LINUX_TO_PHYSICAL",
            "Keycode xkb (evdev + 8) → chiave fisica di Flutter. Ordinata per chiave.",
            &dart_map(&maps, "kLinuxToPhysicalKey", &physical)?,
        ),
        table(
            "GTK_TO_LOGICAL",
            "Keysym xkb → chiave logica di Flutter. Ordinata per chiave.",
            &dart_map(&maps, "kGtkToLogicalKey", &logical)?,
        ),
        table(
            "GTK_NUMPAD",
            "Keysym del tastierino numerico → chiave logica di Flutter. Ordinata per chiave.",
            &dart_map(&maps, "kGtkNumpadMap", &logical)?,
        ),
    );
    let path = root()?.join("crates/embedder/src/keymap.rs");
    std::fs::write(&path, out).map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!("xtask: {}", path.display());
    Ok(())
}
