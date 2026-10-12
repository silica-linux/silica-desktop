//! `cargo xtask`: automazione del repository.
//!
//! - `ci`: fmt, clippy con warning come errori, test, layers, assets.
//! - `layers`: verifica le dipendenze vietate tra i livelli.
//! - `assets`: blocca gli asset senza riga in `ASSETS.md`.
//! - `run`: avvia Ossido (`--nested` o `--headless`, `--control PORT`).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

type Result<T> = std::result::Result<T, String>;

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
        _ => Err("uso: cargo xtask <ci|layers|assets|run> [argomenti]".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("xtask: {err}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "radice del repository non trovata".to_owned())
}

fn cargo_command(args: &[&str]) -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()));
    command.args(args);
    if let Ok(root) = root() {
        command.current_dir(root);
    }
    command
}

fn status(mut command: Command) -> Result<()> {
    let line = format!("{command:?}");
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
    eprintln!("xtask: ci ok");
    Ok(())
}

fn run(args: &[String]) -> Result<()> {
    let mut forwarded = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--nested" => forwarded.extend(["--backend", "winit"]),
            "--headless" => forwarded.extend(["--backend", "headless"]),
            other => forwarded.push(other),
        }
    }
    let mut command = cargo_command(&["run", "--package", "ossido", "--"]);
    command.args(forwarded);
    if let Some(socket) = wslg_socket() {
        eprintln!("xtask: WAYLAND_DISPLAY={}", socket.display());
        command.env("WAYLAND_DISPLAY", socket);
    }
    status(command)
}

/// In WSL2 il link `$XDG_RUNTIME_DIR/wayland-0` verso WSLg a volte manca: in quel caso
/// winit va puntato direttamente al socket di WSLg.
fn wslg_socket() -> Option<PathBuf> {
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
