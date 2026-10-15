//! Reads the app's files ahead (#191): the engine library, the AOT snapshot (`libapp.so`),
//! the ICU data and the bundle's assets are read from disk only once the engine starts, after
//! the renderer's initialisation, and from a cold disk that took 0.3 to 0.5 s of page faults
//! one after the other. A thread asks the kernel to read them (`POSIX_FADV_WILLNEED`, which
//! does not wait for the disk) while the main thread sets up Wayland and the GPU.

use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

/// Bounds of what is read ahead from the bundle: a bundle is a few megabytes, a bigger one
/// (videos, large data) is read on demand as before.
const MAX_FILES: usize = 512;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// How deep the bundle's folders are walked.
const MAX_DEPTH: usize = 8;

/// Starts reading `files` and then the bundle's assets ahead, in the background.
pub fn start(files: Vec<PathBuf>, bundle: PathBuf) {
    let spawned = std::thread::Builder::new()
        .name("prefetch".to_owned())
        .spawn(move || {
            for file in files
                .iter()
                .chain(&bundle_files(&bundle, MAX_FILES, MAX_BYTES))
            {
                will_need(file);
            }
            crate::trace::mark("prefetch requested");
        });
    if let Err(err) = spawned {
        tracing::debug!("prefetch: {err}");
    }
}

/// Asks the kernel to read `path` into the page cache, without waiting for it.
fn will_need(path: &Path) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let len = file.metadata().ok().and_then(|m| NonZeroU64::new(m.len()));
    if len.is_some() {
        let _ = rustix::fs::fadvise(&file, 0, len, rustix::fs::Advice::WillNeed);
    }
}

/// The regular files under `bundle`, top folders first (breadth first), at most
/// `max_files` and `max_bytes` in all.
pub fn bundle_files(bundle: &Path, max_files: usize, max_bytes: u64) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut bytes = 0u64;
    let mut level = vec![bundle.to_path_buf()];
    for _ in 0..MAX_DEPTH {
        let mut next = Vec::new();
        for dir in &level {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                // Symbolic links are not followed: a bundle has none, and a loop stays out.
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    next.push(entry.path());
                } else if kind.is_file() {
                    let size = entry.metadata().map_or(0, |m| m.len());
                    if files.len() >= max_files || bytes.saturating_add(size) > max_bytes {
                        return files;
                    }
                    bytes = bytes.saturating_add(size);
                    files.push(entry.path());
                }
            }
        }
        if next.is_empty() {
            break;
        }
        level = next;
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("silica-prefetch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("fonts/more")).unwrap();
        std::fs::write(root.join("AssetManifest.bin"), [0u8; 10]).unwrap();
        std::fs::write(root.join("fonts/Inter.ttf"), [0u8; 100]).unwrap();
        std::fs::write(root.join("fonts/more/Mono.ttf"), [0u8; 1000]).unwrap();
        std::os::unix::fs::symlink(&root, root.join("fonts/loop")).unwrap();
        root
    }

    #[test]
    fn files_of_the_bundle_top_folders_first() {
        let root = tree("all");
        let files = bundle_files(&root, 100, 1 << 20);
        let names: Vec<_> = files
            .iter()
            .map(|f| f.strip_prefix(&root).unwrap().to_path_buf())
            .collect();
        assert_eq!(
            names,
            [
                "AssetManifest.bin",
                "fonts/Inter.ttf",
                "fonts/more/Mono.ttf"
            ]
            .map(PathBuf::from)
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn bounded_by_count_and_size() {
        let root = tree("bounds");
        assert_eq!(bundle_files(&root, 2, 1 << 20).len(), 2);
        assert_eq!(bundle_files(&root, 100, 110).len(), 2);
        assert_eq!(bundle_files(&root, 100, 5).len(), 0);
        assert!(bundle_files(&root.join("missing"), 100, 1 << 20).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn will_need_ignores_missing_and_empty_files() {
        let root = tree("advise");
        std::fs::write(root.join("empty"), []).unwrap();
        will_need(&root.join("empty"));
        will_need(&root.join("missing"));
        will_need(&root.join("fonts/Inter.ttf"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
