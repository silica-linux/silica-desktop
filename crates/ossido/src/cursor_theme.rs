//! The cursor theme: Silica's cursors (silica-cursors, CC BY-SA 4.0), one SVG per shape with a
//! manifest (`cursors.json`: name, file, hotspot in units of the 32 u canvas, aliases), drawn
//! at 32 pt and rasterized here at the output scale. Looked up in `SILICA_CURSORS_DIR`
//! (development), then `silica/cursors` in the XDG data folders. Without a theme the built-in
//! arrow of `cursor.rs` stays.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use smithay::input::pointer::CursorIcon;
use tracing::{info, warn};

/// The nominal size of the canvas on screen, in logical pixels (cursor-format.md).
pub const CANVAS: f64 = 32.0;

/// A cursor ready to upload: premultiplied RGBA, its size and its hotspot (physical pixels).
#[derive(Debug, Clone)]
pub struct CursorImage {
    pub width: u32,
    pub height: u32,
    pub hotspot: (i32, i32),
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
struct Entry {
    file: PathBuf,
    /// In units of the canvas (0..32).
    hotspot: (f64, f64),
}

/// The shapes found on disk, by name and alias.
#[derive(Debug, Default)]
pub struct CursorTheme {
    entries: HashMap<String, Entry>,
    cache: HashMap<(String, u32), Option<CursorImage>>,
}

impl CursorTheme {
    /// The first folder with a manifest; an empty theme if none.
    pub fn load() -> Self {
        for dir in search_dirs() {
            match Self::from_dir(&dir) {
                Some(theme) => {
                    info!(
                        "cursors: {} shapes from {}",
                        theme.entries.len(),
                        dir.display()
                    );
                    return theme;
                }
                None => continue,
            }
        }
        warn!("cursors: no theme found, built-in arrow only");
        Self::default()
    }

    /// The theme in `dir` (`cursors.json` next to the SVGs).
    pub fn from_dir(dir: &Path) -> Option<Self> {
        let manifest = std::fs::read_to_string(dir.join("cursors.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&manifest).ok()?;
        let mut entries = HashMap::new();
        for cursor in json.get("cursors")?.as_array()? {
            let Some(name) = cursor.get("name").and_then(|n| n.as_str()) else {
                continue;
            };
            let Some(file) = cursor.get("file").and_then(|f| f.as_str()) else {
                continue;
            };
            let hotspot = cursor
                .get("hotspot")
                .and_then(|h| h.as_array())
                .and_then(|h| Some((h.first()?.as_f64()?, h.get(1)?.as_f64()?)))
                .unwrap_or((0.0, 0.0));
            let entry = Entry {
                file: dir.join(file),
                hotspot,
            };
            for alias in cursor
                .get("aliases")
                .and_then(|a| a.as_array())
                .into_iter()
                .flatten()
                .filter_map(|a| a.as_str())
            {
                entries.entry(alias.to_owned()).or_insert(entry.clone());
            }
            entries.insert(name.to_owned(), entry);
        }
        (!entries.is_empty()).then_some(Self {
            entries,
            cache: HashMap::new(),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The image of `icon` at `scale` (whole or fractional), cached; falls back to `default`.
    pub fn image(&mut self, icon: CursorIcon, scale: f64) -> Option<CursorImage> {
        let name = icon.name();
        let key = (name.to_owned(), (scale * 100.0).round() as u32);
        if let Some(image) = self.cache.get(&key) {
            return image.clone();
        }
        let image = self
            .entries
            .get(name)
            .or_else(|| self.entries.get("default"))
            .and_then(|entry| rasterize(entry, scale));
        self.cache.insert(key, image.clone());
        image
    }
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::var_os("SILICA_CURSORS_DIR").filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(dir));
    }
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    if let Some(home) = data_home {
        dirs.push(home.join("silica/cursors"));
    }
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    for dir in data_dirs.split(':').filter(|d| !d.is_empty()) {
        dirs.push(Path::new(dir).join("silica/cursors"));
    }
    dirs
}

fn rasterize(entry: &Entry, scale: f64) -> Option<CursorImage> {
    let data = match std::fs::read(&entry.file) {
        Ok(data) => data,
        Err(err) => {
            warn!("cursor {}: {err}", entry.file.display());
            return None;
        }
    };
    let tree = match resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()) {
        Ok(tree) => tree,
        Err(err) => {
            warn!("cursor {}: {err}", entry.file.display());
            return None;
        }
    };
    let side = (CANVAS * scale).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(side, side)?;
    let size = tree.size();
    let fit = f64::from(side) / f64::from(size.width().max(1.0));
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(fit as f32, fit as f32),
        &mut pixmap.as_mut(),
    );
    let unit = f64::from(side) / CANVAS;
    Some(CursorImage {
        width: side,
        height: side,
        hotspot: (
            (entry.hotspot.0 / CANVAS * CANVAS * unit).round() as i32,
            (entry.hotspot.1 / CANVAS * CANVAS * unit).round() as i32,
        ),
        rgba: pixmap.take(),
    })
}

/// The cursor for the sides of a resize (MU-08): ↔ on the left and right edges, ↕ on the top
/// and bottom ones, the diagonals near the corners.
pub fn resize_icon(edges: crate::resize::Edges) -> CursorIcon {
    match (edges.left, edges.right, edges.top, edges.bottom) {
        (true, _, true, _) | (_, true, _, true) => CursorIcon::NwseResize,
        (true, _, _, true) | (_, true, true, _) => CursorIcon::NeswResize,
        (true, _, _, _) | (_, true, _, _) => CursorIcon::EwResize,
        _ => CursorIcon::NsResize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resize::Edges;

    fn theme_dir() -> Option<PathBuf> {
        let dir = PathBuf::from(std::env::var_os("HOME")?).join("silica/silica-cursors");
        dir.join("cursors.json").exists().then_some(dir)
    }

    #[test]
    fn resize_edges_map_to_the_measured_shapes() {
        let e = |left, right, top, bottom| Edges {
            left,
            right,
            top,
            bottom,
        };
        assert_eq!(
            resize_icon(e(true, false, false, false)),
            CursorIcon::EwResize
        );
        assert_eq!(
            resize_icon(e(false, false, false, true)),
            CursorIcon::NsResize
        );
        assert_eq!(
            resize_icon(e(true, false, true, false)),
            CursorIcon::NwseResize
        );
        assert_eq!(
            resize_icon(e(false, true, false, true)),
            CursorIcon::NwseResize
        );
        assert_eq!(
            resize_icon(e(false, true, true, false)),
            CursorIcon::NeswResize
        );
        assert_eq!(
            resize_icon(e(true, false, false, true)),
            CursorIcon::NeswResize
        );
    }

    #[test]
    fn the_theme_rasterizes_at_the_output_scale() {
        // Only where silica-cursors is checked out next to this repository.
        let Some(dir) = theme_dir() else {
            return;
        };
        let mut theme = CursorTheme::from_dir(&dir).expect("manifest");
        let one = theme
            .image(CursorIcon::Default, 1.0)
            .expect("default at 1x");
        assert_eq!((one.width, one.height), (32, 32));
        assert_eq!(one.rgba.len(), 32 * 32 * 4);
        let two = theme.image(CursorIcon::Text, 2.0).expect("text at 2x");
        assert_eq!((two.width, two.height), (64, 64));
        // The I-beam's hotspot is the centre of the canvas.
        assert_eq!(two.hotspot, (32, 32));
        // Something is drawn.
        assert!(one.rgba.chunks(4).any(|p| p[3] > 0));
        // Aliases resolve.
        assert!(theme.image(CursorIcon::Pointer, 1.0).is_some());
    }
}
