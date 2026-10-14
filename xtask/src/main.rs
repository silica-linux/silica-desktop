//! `cargo xtask`: repository automation.
//!
//! - `ci`: fmt, flutter analyze/test, clippy with warnings as errors, tests, layers, assets, unsafe.
//! - `layers`: checks the forbidden dependencies between layers.
//! - `assets`: blocks assets without a row in `ASSETS.md`.
//! - `run`: starts Ossido (`--nested` or `--headless`, `--control PORT`); with `--shell` also
//!   the full shell (wallpaper, menu bar, Dock, Faro, notifications) and the private session
//!   bus with `silica-daemon`, in release.
//! - `unsafe`: `unsafe` allowed only in the embedder FFI.
//! - `engine`, `bindings`, `keymap`, `bundle <app>`: Flutter for the embedder (see `flutter.rs`).

mod bench;
mod bench_glass;
mod flutter;
mod install;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

pub(crate) type Result<T> = std::result::Result<T, String>;

/// Forbidden dependencies: (crate, dependency, even transitive, reason).
const FORBIDDEN: &[(&str, &str, &str)] = &[(
    "ossido",
    "silica-daemon",
    "the compositor does not depend on the system services",
)];

/// Crates that bring Flutter: no workspace crate below the shell uses them.
const FLUTTER_CRATES: &[&str] = &["flutter_rust_bridge"];

/// Extensions treated as assets by `cargo xtask assets`.
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
            Some(other) => Err(format!("usage: cargo xtask engine [--release], not {other}")),
        },
        Some("bindings") => flutter::bindings(),
        Some("bench") => match args.get(1).map(String::as_str) {
            Some("glass") => bench_glass::glass(args.get(2..).unwrap_or_default()),
            _ => Err("uso: cargo xtask bench glass [--runs N] [--seconds S] [--scale N]".to_owned()),
        },
        Some("keymap") => flutter::keymap(),
        Some("bundle") => match (args.get(1), args.get(2).map(String::as_str)) {
            (Some(app), None) => flutter::bundle(app),
            (Some(app), Some("--release")) => flutter::bundle_release(app),
            _ => Err("usage: cargo xtask bundle <shell | demo | path of an app> [--release]".to_owned()),
        },
        Some("install-app") => match args.get(1) {
            Some(app) => install::install_app(app),
            None => Err("usage: cargo xtask install-app <path of an app>".to_owned()),
        },
        Some("install-portal") => install::install_portal(),
        _ => Err(
            "usage: cargo xtask <ci|layers|assets|unsafe|run|engine|bindings|keymap|bundle|install-app|install-portal> [arguments]"
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
        .ok_or_else(|| "repository root not found".to_owned())
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
        .map_err(|e| format!("cannot start {line}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{line} failed ({status})"))
    }
}

fn cargo(args: &[&str]) -> Result<()> {
    status(cargo_command(args))
}

fn ci() -> Result<()> {
    cargo(&["fmt", "--all", "--check"])?;
    // The embedder tests run the demo app: they need the engine and the bundle.
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
    // The icons: the silica-icons clone next to this repository (~/silica/silica-icons),
    // unless SILICA_ICONS_DIR is already set (docs/icons.md).
    if std::env::var_os("SILICA_ICONS_DIR").is_none()
        && let Some(icons) = root()?.parent().map(|dir| dir.join("silica-icons"))
        && icons.is_dir()
    {
        eprintln!("xtask: SILICA_ICONS_DIR={}", icons.display());
        command.env("SILICA_ICONS_DIR", icons);
    }
    // On WSL2 Mesa picks llvmpipe for EGL clients: with /dev/dxg the GPU goes through d3d12.
    if Path::new("/dev/dxg").exists() && std::env::var_os("GALLIUM_DRIVER").is_none() {
        eprintln!("xtask: GALLIUM_DRIVER=d3d12");
        command.env("GALLIUM_DRIVER", "d3d12");
    }
    status(command)
}

/// The full shell: Ossido and the embedder in release, the shell bundle (built if
/// missing), and the components as `--spawn-shell` arguments (wallpaper, menu bar, Dock,
/// Faro, notifications, Control Center, app switcher).
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
            "{} missing: cargo xtask engine --release (docs/engine-release.md)",
            engine.display()
        ));
    }
    let bundle = root.join("shell/build/release/flutter_assets");
    if !bundle.with_file_name("libapp.so").exists() {
        flutter::bundle_release("shell")?;
    }
    let embedder = format!(
        "{} --engine {} --renderer vulkan",
        root.join("target/release/silica-embedder").display(),
        engine.display()
    );
    let bundle = bundle.display();
    let components = [
        format!(
            "--layer background --anchor top,bottom,left,right --size 1280x800 --exclusive-zone -1 --namespace wallpaper --dart-arg wallpaper {bundle}"
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
            "--layer overlay --anchor top,bottom,left,right --size 1280x800 --exclusive-zone -1 --keyboard none --namespace switcher --dart-arg switcher {bundle}"
        ),
        format!(
            "--layer overlay --anchor top,bottom,left,right --size 1280x800 --exclusive-zone -1 --keyboard none --namespace notifications --dart-arg notifications {bundle}"
        ),
        format!(
            "--layer overlay --anchor top,bottom,left,right --size 1280x800 --exclusive-zone -1 --keyboard none --namespace controlcenter --dart-arg controlcenter {bundle}"
        ),
    ];
    // Private session bus with silica-daemon (notifications): when nested, the host
    // session's ones are already taken.
    Ok(std::iter::once("--session-bus".to_owned())
        .chain(
            components
                .iter()
                .flat_map(|args| ["--spawn-shell".to_owned(), format!("{embedder} {args}")]),
        )
        .collect())
}

/// On WSL2 the `$XDG_RUNTIME_DIR/wayland-0` link to WSLg is sometimes missing: in that case
/// winit must point straight at WSLg's socket.
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
        .map_err(|e| format!("cannot start cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let meta: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("unreadable cargo metadata: {e}"))?;

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
                errors.push(format!("{name} depends on {forbidden}: {why}"));
            }
        }
        for flutter in FLUTTER_CRATES {
            if reachable.contains(flutter) {
                errors.push(format!(
                    "{name} depends on {flutter}: Flutter sits above the crates"
                ));
            }
        }
    }
    if errors.is_empty() {
        eprintln!("xtask: layers ok ({} crate)", members.len());
        Ok(())
    } else {
        Err(format!("layers violated:\n  {}", errors.join("\n  ")))
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
        .map_err(|e| format!("unreadable ASSETS.md: {e}"))?;
    let declared: BTreeSet<String> = listing
        .lines()
        .filter_map(|line| line.trim().strip_prefix('|'))
        .filter_map(|row| row.split('|').next())
        .map(|cell| cell.trim().trim_matches('`').to_owned())
        .filter(|cell| !cell.is_empty() && cell != "Path" && !cell.starts_with("---"))
        .collect();

    let output = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("cannot start git: {e}"))?;
    if !output.status.success() {
        return Err("git ls-files failed".to_owned());
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
        eprintln!("xtask: assets ok ({} declared)", declared.len());
        Ok(())
    } else {
        Err(format!(
            "assets without a row in ASSETS.md:\n  {}",
            missing.join("\n  ")
        ))
    }
}

/// Where `unsafe` is allowed: the embedder FFI and Ossido's EGL initialization
/// (AGENTS.md, "Never crash").
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
        .map_err(|e| format!("cannot start git: {e}"))?;
    let files = String::from_utf8_lossy(&output.stdout);
    let mut offenders = Vec::new();
    for path in files
        .lines()
        .filter(|p| !UNSAFE_ALLOWED.iter().any(|allowed| p.starts_with(allowed)))
    {
        let text = std::fs::read_to_string(root.join(path)).unwrap_or_default();
        // In two pieces, so that this file does not find itself.
        if text.contains(concat!("allow(", "unsafe_code)")) {
            offenders.push(path.to_owned());
        }
    }
    if offenders.is_empty() {
        eprintln!("xtask: unsafe ok (solo in {})", UNSAFE_ALLOWED.join(", "));
        Ok(())
    } else {
        Err(format!(
            "unsafe allowed outside {}:\n  {}",
            UNSAFE_ALLOWED.join(", "),
            offenders.join("\n  ")
        ))
    }
}
