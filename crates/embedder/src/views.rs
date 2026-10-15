//! Several windows in one app process (silica-desktop#146): the pieces that do not touch
//! Wayland objects directly. `window.rs` keeps one `View` per window, with the engine's view
//! id (0 is the main window, the engine's implicit view).
//!
//! The `silica/window` channel (JSONMethodCodec) grows these calls:
//! - `openWindow {title, width, height, minWidth?, minHeight?, maxWidth?, maxHeight?,
//!   kind?, parent?, toolbarHeight?}` → the new view id, once the engine has added the view
//!   (the app then builds its widgets in `View(view: …)`); null if it could not open;
//! - `closeWindow {viewId}`: the window goes away (the main one as its close button does);
//! - `setTitle {viewId, title}`: the title of a window (the main one also follows
//!   `SystemChrome.setApplicationSwitcherDescription`);
//! - every other call (`setFullSizeContent`, `startMove`, `setSize`, `setSizeLimits`) takes an
//!   optional `viewId`, 0 when missing.
//!
//! And sends to Dart, on the same channel:
//! - `closeRequested {viewId}`: the close button of a window other than the main one; the
//!   app answers `true` and closes it itself with `closeWindow` (after asking to save a
//!   draft), anything else closes it right away;
//! - `windowClosed {viewId}`: a window is gone (from either side).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use silica_protocols::glass::client::silica_glass_manager_v1::SilicaGlassManagerV1;
use silica_protocols::glass::client::silica_glass_v1::SilicaGlassV1;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle};

use crate::glass::{Glass, GlassSync};

/// The engine's implicit view: the main window, created with the process.
pub const MAIN_VIEW: i64 = 0;

/// The most windows a process may open besides the main one (a runaway loop in an app must
/// not exhaust the compositor).
pub const MAX_WINDOWS: usize = 64;

/// The kind of a window: a document or browser window, or a utility (inspector, Info).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Regular,
    Utility,
}

/// A window asked by the app with `openWindow`, in logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowSpec {
    pub title: String,
    pub size: (u32, u32),
    pub min_size: Option<(u32, u32)>,
    pub max_size: Option<(u32, u32)>,
    pub kind: Kind,
    /// The view the window belongs to (`xdg_toplevel.set_parent`): it stays above it.
    pub parent: Option<i64>,
    /// Full-size content with the app's toolbar this high, from the first frame.
    pub toolbar_height: Option<u32>,
}

/// A side asked by the app, in logical pixels: 1–8192, rounded.
pub fn window_side(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 1.0).then(|| value.round().min(8192.0) as u32)
}

/// A toolbar height asked by the app: 0–200, rounded (0 goes back to Ossido's bar).
pub fn toolbar_height(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 0.0).then(|| value.round().min(200.0) as u32)
}

/// The view a `silica/window` call is for (`viewId`, 0 when missing or not an integer).
pub fn view_arg(call: &Value) -> i64 {
    call.pointer("/args/viewId")
        .and_then(Value::as_i64)
        .unwrap_or(MAIN_VIEW)
}

impl WindowSpec {
    /// The window of an `openWindow` call's arguments; None without a valid size.
    pub fn parse(args: &Value) -> Option<Self> {
        let side = |key: &str| args.get(key).and_then(Value::as_f64).and_then(window_side);
        let pair = |w: &str, h: &str| side(w).zip(side(h));
        let title: String = args
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .chars()
            .take(256)
            .collect();
        let mut min_size = pair("minWidth", "minHeight");
        let max_size = pair("maxWidth", "maxHeight");
        // A minimum above the maximum would be a protocol error: the maximum wins.
        if let (Some(min), Some(max)) = (min_size, max_size) {
            min_size = Some((min.0.min(max.0), min.1.min(max.1)));
        }
        let mut size = pair("width", "height")?;
        if let Some(min) = min_size {
            size = (size.0.max(min.0), size.1.max(min.1));
        }
        if let Some(max) = max_size {
            size = (size.0.min(max.0), size.1.min(max.1));
        }
        Some(Self {
            title,
            size,
            min_size,
            max_size,
            kind: match args.get("kind").and_then(Value::as_str) {
                Some("utility") => Kind::Utility,
                _ => Kind::Regular,
            },
            parent: args.get("parent").and_then(Value::as_i64),
            toolbar_height: args
                .get("toolbarHeight")
                .and_then(Value::as_f64)
                .and_then(toolbar_height)
                .filter(|h| *h > 0),
        })
    }
}

/// What closing a window does (#142 APP-LC, extended to several windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closing {
    /// The app ends (its last window, for an app that quits with it; or the system cannot
    /// keep it running without windows).
    Exit,
    /// The main window is hidden (the engine's implicit view cannot be removed); `last` if
    /// it was the last window on screen (the app learns `lastWindowClosed`).
    HideMain { last: bool },
    /// Another window goes away with its view; `last` as above.
    Remove { last: bool },
}

/// Closing window `view` while `others` other windows are on screen.
pub fn closing(view: i64, others: usize, quit_with_last: bool, hideable: bool) -> Closing {
    let last = others == 0;
    if view != MAIN_VIEW {
        return if last && quit_with_last {
            Closing::Exit
        } else {
            Closing::Remove { last }
        };
    }
    if last && (quit_with_last || !hideable) {
        return Closing::Exit;
    }
    if !hideable {
        // Ossido cannot hide the main window: it stays, so the others keep their app.
        return Closing::HideMain { last: false };
    }
    Closing::HideMain { last }
}

/// The glass of every window, shared with the raster thread (the regions travel with the
/// frames, `GlassSync`).
#[derive(Default)]
pub struct GlassViews {
    views: Mutex<BTreeMap<i64, GlassView>>,
}

struct GlassView {
    glass: Glass,
    surface: WlSurface,
    sync: Arc<GlassSync>,
}

impl GlassViews {
    /// The glass of a new window on `surface`.
    pub fn insert(
        &self,
        view: i64,
        manager: Option<SilicaGlassManagerV1>,
        conn: &Connection,
        surface: &WlSurface,
    ) {
        let sync = Arc::new(GlassSync::new(conn.clone(), surface.clone()));
        let glass = Glass::new(manager, Arc::clone(&sync));
        if let Ok(mut views) = self.views.lock() {
            views.insert(
                view,
                GlassView {
                    glass,
                    surface: surface.clone(),
                    sync,
                },
            );
        }
    }

    /// The window is gone: its glass object goes with it.
    pub fn remove(&self, view: i64) {
        if let Ok(mut views) = self.views.lock() {
            views.remove(&view);
        }
    }

    /// Takes in a `silica/glass` call in the platform task that delivers it (see
    /// `Glass::receive`): `setRegions [regions]` for the main window, `setViewRegions
    /// {viewId, regions}` for the others. Commits the window's surface if its frame is
    /// already on screen.
    pub fn receive<D>(&self, message: &[u8], raster_seq: u64, qh: &QueueHandle<D>)
    where
        D: Dispatch<SilicaGlassV1, ()> + 'static,
    {
        let Ok(call) = serde_json::from_slice::<Value>(message) else {
            return;
        };
        let (view, regions) = match call.get("method").and_then(Value::as_str) {
            Some("setRegions") => (MAIN_VIEW, call.get("args").cloned()),
            Some("setViewRegions") => (view_arg(&call), call.pointer("/args/regions").cloned()),
            _ => return,
        };
        let regions = regions.unwrap_or(Value::Null);
        let Ok(mut views) = self.views.lock() else {
            return;
        };
        let Some(entry) = views.get_mut(&view) else {
            return;
        };
        if entry
            .glass
            .set_from(&regions, raster_seq, &entry.surface, qh)
        {
            entry.surface.commit();
        }
    }

    /// The reply to a `silica/glass` call: whether Ossido offers glass.
    pub fn reply(&self, message: &[u8]) -> Vec<u8> {
        let Ok(call) = serde_json::from_slice::<Value>(message) else {
            return Vec::new();
        };
        match call.get("method").and_then(Value::as_str) {
            Some("setRegions" | "setViewRegions" | "isSupported") => {
                let supported = self.views.lock().ok().is_some_and(|views| {
                    views
                        .get(&MAIN_VIEW)
                        .is_some_and(|view| view.glass.supported())
                });
                crate::text_input::success(json!(supported))
            }
            _ => Vec::new(),
        }
    }

    fn sync(&self, view: i64) -> Option<Arc<GlassSync>> {
        self.views
            .lock()
            .ok()?
            .get(&view)
            .map(|view| Arc::clone(&view.sync))
    }

    /// The frame of raster task `seq` for `view` is about to be committed.
    pub fn before_present(&self, view: i64, seq: u64) {
        if let Some(sync) = self.sync(view) {
            sync.before_present(seq);
        }
    }

    /// The software renderer drew a frame of `view` in raster task `seq`.
    pub fn software_frame(&self, view: i64, seq: u64) {
        if let Some(sync) = self.sync(view) {
            sync.software_frame(seq);
        }
    }

    /// Raster task `seq` has run: regions waiting for it on any window go out.
    pub fn after_task(&self, seq: u64) {
        let syncs: Vec<Arc<GlassSync>> = match self.views.lock() {
            Ok(views) => views.values().map(|view| Arc::clone(&view.sync)).collect(),
            Err(_) => return,
        };
        for sync in syncs {
            sync.after_task(seq);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_window_call_becomes_a_bounded_window() {
        let spec = WindowSpec::parse(&json!({
            "title": "Info",
            "width": 280.4,
            "height": 600,
            "minWidth": 260,
            "minHeight": 300,
            "kind": "utility",
            "parent": 0,
            "toolbarHeight": 52,
        }))
        .unwrap();
        assert_eq!(
            spec,
            WindowSpec {
                title: "Info".to_owned(),
                size: (280, 600),
                min_size: Some((260, 300)),
                max_size: None,
                kind: Kind::Utility,
                parent: Some(0),
                toolbar_height: Some(52),
            }
        );
    }

    #[test]
    fn sizes_follow_the_limits_and_bad_values_are_refused() {
        // The size grows to the minimum and shrinks to the maximum.
        let spec = WindowSpec::parse(&json!({
            "width": 100, "height": 900,
            "minWidth": 200, "minHeight": 100,
            "maxWidth": 400, "maxHeight": 500,
        }))
        .unwrap();
        assert_eq!(spec.size, (200, 500));
        // A minimum above the maximum is clamped to it (never a protocol error).
        let spec = WindowSpec::parse(&json!({
            "width": 300, "height": 300,
            "minWidth": 500, "minHeight": 100,
            "maxWidth": 400, "maxHeight": 400,
        }))
        .unwrap();
        assert_eq!(spec.min_size, Some((400, 100)));
        assert_eq!(spec.size, (400, 300));
        // No size, a zero or a NaN side: no window.
        assert_eq!(WindowSpec::parse(&json!({"title": "x"})), None);
        assert_eq!(WindowSpec::parse(&json!({"width": 0, "height": 10})), None);
        assert_eq!(
            WindowSpec::parse(&json!({"width": "a", "height": 10})),
            None
        );
        // Unknown kinds are regular windows; a long title is cut.
        let spec = WindowSpec::parse(&json!({
            "width": 10, "height": 10, "kind": "?", "title": "x".repeat(1000),
        }))
        .unwrap();
        assert_eq!(spec.kind, Kind::Regular);
        assert_eq!(spec.title.len(), 256);
        assert_eq!(spec.parent, None);
        assert_eq!(spec.toolbar_height, None);
    }

    #[test]
    fn calls_name_their_view_or_the_main_one() {
        assert_eq!(view_arg(&json!({"method": "startMove"})), MAIN_VIEW);
        assert_eq!(
            view_arg(&json!({"method": "startMove", "args": {"viewId": 3}})),
            3
        );
        assert_eq!(
            view_arg(&json!({"method": "startMove", "args": {"viewId": "3"}})),
            MAIN_VIEW
        );
    }

    #[test]
    fn closing_follows_the_app_lifecycle_across_windows() {
        // The main window alone: hidden (the app keeps running) or the app ends.
        assert_eq!(
            closing(MAIN_VIEW, 0, false, true),
            Closing::HideMain { last: true }
        );
        assert_eq!(closing(MAIN_VIEW, 0, true, true), Closing::Exit);
        assert_eq!(closing(MAIN_VIEW, 0, false, false), Closing::Exit);
        // The main window with an Info window open: only hidden, the app is not windowless.
        assert_eq!(
            closing(MAIN_VIEW, 1, false, true),
            Closing::HideMain { last: false }
        );
        assert_eq!(
            closing(MAIN_VIEW, 1, true, true),
            Closing::HideMain { last: false }
        );
        assert_eq!(
            closing(MAIN_VIEW, 1, false, false),
            Closing::HideMain { last: false }
        );
        // Another window: removed; the last one on screen ends an app that quits with it.
        assert_eq!(closing(2, 1, true, true), Closing::Remove { last: false });
        assert_eq!(closing(2, 0, false, true), Closing::Remove { last: true });
        assert_eq!(closing(2, 0, true, true), Closing::Exit);
    }

    #[test]
    fn toolbar_heights_are_bounded() {
        assert_eq!(toolbar_height(52.4), Some(52));
        assert_eq!(toolbar_height(-1.0), None);
        assert_eq!(toolbar_height(1e6), Some(200));
        assert_eq!(window_side(0.5), None);
        assert_eq!(window_side(1e9), Some(8192));
    }
}
