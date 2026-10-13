//! `cargo xtask install-app <percorso>`: installa la voce `.desktop` di un'app Flutter di
//! Silica (un repository per app, accanto a questo) in `~/.local/share/applications`, così la
//! shell la trova con la sua icona: Dock, selettore, Faro, launcher.
//!
//! Il repository dell'app ha un modello `linux/<id>.desktop.in` con due segnaposto:
//! `@EMBEDDER@` (l'embedder in release con l'engine e il renderer della shell) e `@BUNDLE@`
//! (il suo bundle in release). Se mancano, bundle ed embedder si costruiscono prima.

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
        return Err(format!(
            "{}/linux: nessun modello .desktop.in",
            dir.display()
        ));
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
            "{} assente: cargo xtask engine --release (docs/engine-release.md)",
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
            .ok_or_else(|| format!("{}: nome non valido", template.display()))?;
        let out = target.join(name);
        std::fs::write(&out, entry).map_err(|e| format!("{}: {e}", out.display()))?;
        eprintln!("xtask: installato {}", out.display());
    }
    Ok(())
}

/// `$XDG_DATA_HOME/applications`, o `~/.local/share/applications`.
fn applications_dir() -> Result<PathBuf> {
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return Ok(Path::new(&data).join("applications"));
    }
    let home = std::env::var_os("HOME").ok_or("HOME assente")?;
    Ok(Path::new(&home).join(".local/share/applications"))
}
