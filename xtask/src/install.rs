//! `cargo xtask install-app <path>`: installs the `.desktop` entry of a Silica Flutter app
//! (one repository per app, next to this one) in `~/.local/share/applications`, so the
//! shell finds it with its icon: Dock, switcher, Faro, launcher.
//!
//! The app repository has a `linux/<id>.desktop.in` template with two placeholders:
//! `@EMBEDDER@` (the release embedder with the shell's engine and renderer) and `@BUNDLE@`
//! (its release bundle). If they are missing, the bundle and the embedder are built first.

use std::path::{Path, PathBuf};

use crate::{Result, cargo_command, flutter, root, status};

pub fn install_app(app: &str) -> Result<()> {
    let dir = std::path::absolute(app).map_err(|e| format!("{app}: {e}"))?;
    let templates: Vec<PathBuf> = std::fs::read_dir(dir.join("linux"))
        .map_err(|e| format!("{}/linux: {e}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.to_string_lossy().ends_with(".desktop.in"))
        .collect();
    if templates.is_empty() {
        return Err(format!("{}/linux: no .desktop.in template", dir.display()));
    }
    let bundle = dir.join("build/release/flutter_assets");
    if !dir.join("build/release/libapp.so").exists() {
        flutter::bundle_release(app)?;
    }
    let root = root()?;
    let embedder = root.join("target/release/silica-embedder");
    if !embedder.exists() {
        status(cargo_command(&[
            "build",
            "--release",
            "--package",
            "silica-embedder",
        ]))?;
    }
    let engine = flutter::release_engine_dir()?;
    if !engine.join("libflutter_engine.so").exists() {
        return Err(format!(
            "{} missing: cargo xtask engine --release (docs/engine-release.md)",
            engine.display()
        ));
    }
    let command = format!(
        "{} --engine {} --renderer vulkan",
        embedder.display(),
        engine.display()
    );
    let target = applications_dir()?;
    std::fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
    for template in templates {
        let text = std::fs::read_to_string(&template)
            .map_err(|e| format!("{}: {e}", template.display()))?;
        let entry = text
            .replace("@EMBEDDER@", &command)
            .replace("@BUNDLE@", &bundle.display().to_string());
        let name = template
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".in"))
            .ok_or_else(|| format!("{}: invalid name", template.display()))?;
        let out = target.join(name);
        std::fs::write(&out, entry).map_err(|e| format!("{}: {e}", out.display()))?;
        eprintln!("xtask: installed {}", out.display());
    }
    Ok(())
}

/// `$XDG_DATA_HOME/applications`, or `~/.local/share/applications`.
fn applications_dir() -> Result<PathBuf> {
    Ok(data_home()?.join("applications"))
}

/// `$XDG_DATA_HOME`, or `~/.local/share`.
fn data_home() -> Result<PathBuf> {
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(data));
    }
    let home = std::env::var_os("HOME").ok_or("HOME not set")?;
    Ok(Path::new(&home).join(".local/share"))
}

/// `$XDG_CONFIG_HOME`, or `~/.config`.
fn config_home() -> Result<PathBuf> {
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(config));
    }
    let home = std::env::var_os("HOME").ok_or("HOME not set")?;
    Ok(Path::new(&home).join(".config"))
}

/// `cargo xtask install-portal`: Silica's portal backend for the user. The `.portal` file
/// tells xdg-desktop-portal there is a FileChooser backend (`UseIn=Silica`), the
/// `silica-portals.conf` configuration picks it when `XDG_CURRENT_DESKTOP=Silica`, the
/// D-Bus service starts `silica-portal` on the first request. The dialogs are Mustache's
/// (installed with `install-app`).
pub fn install_portal() -> Result<()> {
    let root = root()?;
    let binary = root.join("target/release/silica-portal");
    if !binary.exists() {
        status(cargo_command(&[
            "build",
            "--release",
            "--package",
            "silica-portal",
        ]))?;
    }
    let data = root.join("crates/portal/data");
    let read =
        |name: &str| std::fs::read_to_string(data.join(name)).map_err(|e| format!("{name}: {e}"));
    let files = [
        (
            data_home()?.join("xdg-desktop-portal/portals/silica.portal"),
            read("silica.portal")?,
        ),
        (
            config_home()?.join("xdg-desktop-portal/silica-portals.conf"),
            read("silica-portals.conf")?,
        ),
        (
            data_home()?.join("dbus-1/services/org.freedesktop.impl.portal.desktop.silica.service"),
            read("org.freedesktop.impl.portal.desktop.silica.service.in")?
                .replace("@PORTAL@", &binary.display().to_string()),
        ),
    ];
    for (path, text) in files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        eprintln!("xtask: installed {}", path.display());
    }
    Ok(())
}
