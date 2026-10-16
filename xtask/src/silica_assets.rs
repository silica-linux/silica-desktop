//! `cargo xtask assets [--icons DIR] [--brand DIR] [--fonts DIR] [--sounds DIR] [--out FILE]`:
//! compiles `silica.assets` (docs/assets-format.md) with `silica-assets-compile`.
//!
//! Without `--icons`, the sources are the branch `amalfi-icons` of the silica-icons checkout
//! next to this repository, read with `git archive` into a temporary directory: the user's
//! working tree and index are never touched. When the branch does not exist, the checkout
//! itself is used (with a warning).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::os::{sibling, target_dir};
use crate::{Result, cargo_command, root, status};

/// The branch of silica-icons the system assets are built from by default.
const ICONS_BRANCH: &str = "amalfi-icons";

/// Where `cargo xtask assets` writes by default and `os deploy` reads from.
pub(crate) fn default_output() -> Result<PathBuf> {
    Ok(target_dir()?.join("silica.assets"))
}

/// A temporary directory removed when it is dropped.
struct Scratch {
    path: PathBuf,
    keep: bool,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.keep {
            eprintln!("xtask: icon sources kept in {}", self.path.display());
        } else {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// Extracts `ICONS_BRANCH` of the repository at `repo` into a new temporary directory.
fn extract_branch(repo: &Path) -> Option<Scratch> {
    let candidates = [
        ICONS_BRANCH.to_owned(),
        format!("refs/remotes/origin/{ICONS_BRANCH}"),
    ];
    let reference = candidates.into_iter().find(|reference| {
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "--verify", "--quiet"])
            .arg(format!("{reference}^{{commit}}"))
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })?;
    // RAM-backed when there is one; the extraction is a few hundred files.
    let base = Some(PathBuf::from("/dev/shm"))
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    let scratch = Scratch {
        path: base.join(format!(
            "silica-icons-{ICONS_BRANCH}-{}",
            std::process::id()
        )),
        keep: false,
    };
    let _ = std::fs::remove_dir_all(&scratch.path);
    std::fs::create_dir_all(&scratch.path).ok()?;
    let mut archive = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["archive", &reference])
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let tar = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(&scratch.path)
        .stdin(archive.stdout.take()?)
        .status()
        .ok()?;
    let archived = archive.wait().ok()?;
    (tar.success() && archived.success()).then_some(scratch)
}

/// Builds `silica.assets` and returns its path.
pub(crate) fn build(args: &[String]) -> Result<PathBuf> {
    let mut icons: Option<PathBuf> = None;
    let mut brand: Option<PathBuf> = None;
    let mut fonts: Option<PathBuf> = None;
    let mut sounds: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut keep = false;
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        if flag == "--keep-icons" {
            keep = true;
            continue;
        }
        let slot = match flag.as_str() {
            "--icons" => &mut icons,
            "--brand" => &mut brand,
            "--fonts" => &mut fonts,
            "--sounds" => &mut sounds,
            "--out" => &mut out,
            other => {
                return Err(format!(
                    "unknown option {other} (usage: cargo xtask assets [--icons DIR] [--brand DIR] [--fonts DIR] [--sounds DIR] [--out FILE])"
                ));
            }
        };
        *slot = Some(PathBuf::from(
            iter.next().ok_or_else(|| format!("{flag} needs a value"))?,
        ));
    }
    let root = root()?;
    let out = match out {
        Some(out) => out,
        None => default_output()?,
    };

    // The extracted branch lives until the compiler has finished.
    let mut scratch: Option<Scratch> = None;
    let icons = match icons {
        Some(dir) => dir,
        None => {
            let checkout = sibling("silica-icons")
                .ok_or("no silica-icons checkout next to this repository (use --icons DIR)")?;
            match extract_branch(&checkout) {
                Some(mut extracted) => {
                    eprintln!(
                        "xtask: icons from the branch {ICONS_BRANCH} of {}",
                        checkout.display()
                    );
                    let dir = extracted.path.clone();
                    extracted.keep = keep;
                    scratch = Some(extracted);
                    dir
                }
                None => {
                    eprintln!(
                        "xtask: warning: no branch {ICONS_BRANCH} in {}: using the checked-out files",
                        checkout.display()
                    );
                    checkout
                }
            }
        }
    };
    let brand = brand.or_else(|| sibling("silica-brand"));
    let fonts = fonts.or_else(|| {
        sibling("silica-ui")
            .map(|dir| dir.join("fonts"))
            .filter(|d| d.is_dir())
    });
    // The interface sounds live in this repository until silica-brand has them.
    let sounds = sounds.or_else(|| Some(root.join("data/sounds")).filter(|d| d.is_dir()));

    let mut command = cargo_command(&[
        "run",
        "--release",
        "--locked",
        "--package",
        "silica-assets",
        "--bin",
        "silica-assets-compile",
        "--",
    ]);
    command.arg("--icons").arg(&icons).arg("--out").arg(&out);
    if let Some(brand) = &brand {
        command.arg("--brand").arg(brand);
    }
    if let Some(fonts) = &fonts {
        command.arg("--fonts").arg(fonts);
    }
    if let Some(sounds) = &sounds {
        command.arg("--sounds").arg(sounds);
    }
    let result = status(command);
    drop(scratch);
    result?;
    Ok(out)
}
