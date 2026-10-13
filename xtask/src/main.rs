//! `cargo xtask`: automazione del repository.
//!
//! - `ci`: fmt, flutter analyze/test, clippy con warning come errori, test, layers, assets, unsafe.
//! - `layers`: verifica le dipendenze vietate tra i livelli.
//! - `assets`: blocca gli asset senza riga in `ASSETS.md`.
//! - `run`: avvia Ossido (`--nested` o `--headless`, `--control PORT`); con `--shell` anche
//!   la shell completa (sfondo, barra dei menu, Dock, Faro, notifiche) e il bus di sessione privato
//!   con `silica-daemon`, in release.
//! - `unsafe`: `unsafe` ammesso solo nell'FFI dell'embedder.
//! - `engine`, `bindings`, `keymap`, `bundle <app>`: Flutter per l'embedder (vedi `flutter.rs`).

mod bench;
mod bench_glass;
mod flutter;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

pub(crate) type Result<T> = std::result::Result<T, String>;

/// Dipendenze vietate: (crate, dipendenza anche transitiva, motivo).
const FORBIDDEN: &[(&str, &str, &str)] = &[(
    "ossido",
    "silica-daemon",
    "il compositor non dipende dai servizi di sistema",
)];

/// Crate che portano Flutter: nessun crate del workspace sotto la shell li usa.
const FLUTTER_CRATES: &[&str] = &["flutter_rust_bridge"];

/// Estensioni trattate come asset da `cargo xtask assets`.
const ASSET_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "icns", "ttf", "otf", "woff", "woff2",
    "wav", "ogg", "oga", "mp3", "flac", "xcf", "psd",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci(),
        Some("layers") => layers(),
        Some("assets") => assets(),
        Some("run") => run(args.get(1..).unwrap_or_default()),
        Some("unsafe") => unsafe_check(),
        Some("engine") => match args.get(1).map(String::as_str) {
            Some("--release") => flutter::engine_release(),
            None => flutter::engine(),
            Some(other) => Err(format!("uso: cargo xtask engine [--release], non {other}")),
        },
        Some("bindings") => flutter::bindings(),
        Some("bench") => match args.get(1).map(String::as_str) {
            Some("dock") => bench::dock(args.get(2..).unwrap_or_default()),
            Some("glass") => bench_glass::glass(args.get(2..).unwrap_or_default()),
            _ => Err("uso: cargo xtask bench dock [--runs N] [--seconds S] [--only VARIANTE] [--scale N]\n     cargo xtask bench glass [--runs N] [--seconds S] [--scale N]".to_owned()),
        },
        Some("keymap") => flutter::keymap(),
        Some("bundle") => match (args.get(1), args.get(2).map(String::as_str)) {
            (Some(app), None) => flutter::bundle(app),
            (Some(app), Some("--release")) => flutter::bundle_release(app),
            _ => Err("uso: cargo xtask bundle <app in apps/ | shell> [--release]".to_owned()),
        },
        _ => Err(
            "uso: cargo xtask <ci|layers|assets|unsafe|run|engine|bindings|keymap|bundle> [argomenti]"
                .to_owned(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("xtask: {err}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "radice del repository non trovata".to_owned())
}

pub(crate) fn cargo_command(args: &[&str]) -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()));
    command.args(args);
    if let Ok(root) = root() {
        command.current_dir(root);
    }
    command
}

pub(crate) fn status(mut command: Command) -> Result<()> {
    let line = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    eprintln!("xtask: {line}");
    let status = command
        .status()
        .map_err(|e| format!("{line} non avviabile: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{line} è fallito ({status})"))
    }
}

fn cargo(args: &[&str]) -> Result<()> {
    status(cargo_command(args))
}

fn ci() -> Result<()> {
    cargo(&["fmt", "--all", "--check"])?;
    // I test dell'embedder fanno girare l'app demo: servono engine e bundle.
    flutter::engine()?;
    for project in flutter::projects()? {
        flutter::check(&project)?;
    }
    flutter::bundle("demo")?;
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ])?;
    cargo(&["test", "--workspace", "--locked"])?;
    layers()?;
    assets()?;
    unsafe_check()?;
    eprintln!("xtask: ci ok");
    Ok(())
}

fn run(args: &[String]) -> Result<()> {
    let mut forwarded: Vec<String> = Vec::new();
    let mut shell = false;
    for arg in args {
        match arg.as_str() {
            "--nested" => forwarded.extend(["--backend".to_owned(), "winit".to_owned()]),
            "--headless" => forwarded.extend(["--backend".to_owned(), "headless".to_owned()]),
            "--shell" => shell = true,
            other => forwarded.push(other.to_owned()),
        }
    }
    let mut command = if shell {
        forwarded.extend(shell_components()?);
        let mut command = Command::new(root()?.join("target/release/ossido"));
        command.args(&forwarded);
        command
    } else {
        let mut command = cargo_command(&["run", "--package", "ossido", "--"]);
        command.args(&forwarded);
        command
    };
    if let Some(socket) = wslg_socket() {
        eprintln!("xtask: WAYLAND_DISPLAY={}", socket.display());
        command.env("WAYLAND_DISPLAY", socket);
    }
    // In WSL2 Mesa sceglie llvmpipe per i client EGL: con /dev/dxg la GPU passa da d3d12.
    if Path::new("/dev/dxg").exists() && std::env::var_os("GALLIUM_DRIVER").is_none() {
        eprintln!("xtask: GALLIUM_DRIVER=d3d12");
        command.env("GALLIUM_DRIVER", "d3d12");
    }
    status(command)
}

/// La shell completa: Ossido ed embedder in release, il bundle della shell (costruito se
/// manca), e i componenti come argomenti `--spawn-shell` (sfondo, barra dei menu, Dock,
/// Faro, notifiche).
fn shell_components() -> Result<Vec<String>> {
    let root = root()?;
    status(cargo_command(&[
        "build",
        "--release",
        "--package",
        "ossido",
        "--package",
        "silica-embedder",
        "--package",
        "silica-daemon",
    ]))?;
    let engine = flutter::release_engine_dir()?;
    if !engine.join("libflutter_engine.so").exists() {
        return Err(format!(
            "{} assente: cargo xtask engine --release (docs/engine-release.md)",
            engine.display()
        ));
    }
    let bundle = root.join("shell/build/release/flutter_assets");
    if !bundle.with_file_name("libapp.so").exists() {
        flutter::bundle_release("shell")?;
    }
    let embedder = format!(
        "{} --engine {} --renderer impeller",
        root.join("target/release/silica-embedder").display(),
        engine.display()
    );
    let bundle = bundle.display();
    let components = [
        format!(
            "--layer background --anchor top,bottom,left,right --size 1280x800 --namespace wallpaper --dart-arg wallpaper {bundle}"
        ),
        format!(
            "--layer top --anchor top,left,right --size 1280x30 --exclusive-zone 30 --namespace menubar --dart-arg menubar {bundle}"
        ),
        format!(
            "--layer top --anchor left,right,bottom --size 1280x200 --exclusive-zone 84 --namespace dock --dart-arg dock {bundle}"
        ),
        format!(
            "--layer overlay --anchor top,bottom,left,right --size 1280x800 --keyboard none --namespace faro --dart-arg faro {bundle}"
        ),
        format!(
            "--layer overlay --anchor top,bottom,left,right --size 1280x800 --exclusive-zone -1 --keyboard none --namespace notifications --dart-arg notifications {bundle}"
        ),
    ];
    // Bus di sessione privato con silica-daemon (notifiche): annidato, quelli della
    // sessione ospite sono già presi.
    Ok(std::iter::once("--session-bus".to_owned())
        .chain(
            components
                .iter()
                .flat_map(|args| ["--spawn-shell".to_owned(), format!("{embedder} {args}")]),
        )
        .collect())
}

/// In WSL2 il link `$XDG_RUNTIME_DIR/wayland-0` verso WSLg a volte manca: in quel caso
/// winit va puntato direttamente al socket di WSLg.
pub(crate) fn wslg_socket() -> Option<PathBuf> {
    let display = std::env::var("WAYLAND_DISPLAY").ok()?;
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let wslg = Path::new("/mnt/wslg/runtime-dir").join(&display);
    let missing = !display.starts_with('/') && !Path::new(&runtime).join(&display).exists();
    (missing && wslg.exists()).then_some(wslg)
}

fn layers() -> Result<()> {
    let output = cargo_command(&["metadata", "--format-version", "1", "--locked"])
        .output()
        .map_err(|e| format!("cargo metadata non avviabile: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata è fallito: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let meta: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata illeggibile: {e}"))?;

    let mut names: HashMap<&str, &str> = HashMap::new();
    for package in meta
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(id), Some(name)) = (
            package.get("id").and_then(Value::as_str),
            package.get("name").and_then(Value::as_str),
        ) {
            names.insert(id, name);
        }
    }
    let mut deps: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in meta
        .pointer("/resolve/nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = node.get("id").and_then(Value::as_str) else {
            continue;
        };
        let children = node
            .get("dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        deps.insert(id, children);
    }
    let members: Vec<&str> = meta
        .get("workspace_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();

    let mut errors = Vec::new();
    for &member in &members {
        let name = names.get(member).copied().unwrap_or(member);
        let reachable = transitive(member, &deps, &names);
        for (krate, forbidden, why) in FORBIDDEN {
            if name == *krate && reachable.contains(forbidden) {
                errors.push(format!("{name} dipende da {forbidden}: {why}"));
            }
        }
        for flutter in FLUTTER_CRATES {
            if reachable.contains(flutter) {
                errors.push(format!(
                    "{name} dipende da {flutter}: Flutter sta sopra i crate"
                ));
            }
        }
    }
    if errors.is_empty() {
        eprintln!("xtask: layers ok ({} crate)", members.len());
        Ok(())
    } else {
        Err(format!("livelli violati:\n  {}", errors.join("\n  ")))
    }
}

fn transitive<'a>(
    start: &'a str,
    deps: &HashMap<&'a str, Vec<&'a str>>,
    names: &HashMap<&'a str, &'a str>,
) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![start];
    let mut found = BTreeSet::new();
    while let Some(id) = stack.pop() {
        for &child in deps.get(id).into_iter().flatten() {
            if seen.insert(child) {
                if let Some(name) = names.get(child) {
                    found.insert(*name);
                }
                stack.push(child);
            }
        }
    }
    found
}

fn assets() -> Result<()> {
    let root = root()?;
    let listing = std::fs::read_to_string(root.join("ASSETS.md"))
        .map_err(|e| format!("ASSETS.md illeggibile: {e}"))?;
    let declared: BTreeSet<String> = listing
        .lines()
        .filter_map(|line| line.trim().strip_prefix('|'))
        .filter_map(|row| row.split('|').next())
        .map(|cell| cell.trim().trim_matches('`').to_owned())
        .filter(|cell| !cell.is_empty() && cell != "Percorso" && !cell.starts_with("---"))
        .collect();

    let output = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("git non avviabile: {e}"))?;
    if !output.status.success() {
        return Err("git ls-files è fallito".to_owned());
    }
    let files = String::from_utf8_lossy(&output.stdout);
    let missing: Vec<&str> = files
        .lines()
        .filter(|path| {
            Path::new(path)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ASSET_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        })
        .filter(|path| !declared.contains(*path))
        .collect();
    if missing.is_empty() {
        eprintln!("xtask: assets ok ({} dichiarati)", declared.len());
        Ok(())
    } else {
        Err(format!(
            "asset senza riga in ASSETS.md:\n  {}",
            missing.join("\n  ")
        ))
    }
}

/// Dove `unsafe` è ammesso: l'FFI dell'embedder e l'inizializzazione EGL di Ossido
/// (AGENTS.md, "Mai crashare").
const UNSAFE_ALLOWED: [&str; 2] = ["crates/embedder/src/ffi/", "crates/ossido/src/egl.rs"];

fn unsafe_check() -> Result<()> {
    let root = root()?;
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "*.rs",
        ])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("git non avviabile: {e}"))?;
    let files = String::from_utf8_lossy(&output.stdout);
    let mut offenders = Vec::new();
    for path in files
        .lines()
        .filter(|p| !UNSAFE_ALLOWED.iter().any(|allowed| p.starts_with(allowed)))
    {
        let text = std::fs::read_to_string(root.join(path)).unwrap_or_default();
        // In due pezzi, perché questo file non trovi sé stesso.
        if text.contains(concat!("allow(", "unsafe_code)")) {
            offenders.push(path.to_owned());
        }
    }
    if offenders.is_empty() {
        eprintln!("xtask: unsafe ok (solo in {})", UNSAFE_ALLOWED.join(", "));
        Ok(())
    } else {
        Err(format!(
            "unsafe consentito fuori da {}:\n  {}",
            UNSAFE_ALLOWED.join(", "),
            offenders.join("\n  ")
        ))
    }
}
