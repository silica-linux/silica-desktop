//! xdg-foreign v2: a window of another process as the parent of this one, and this process's
//! windows handed to another one.
//!
//! - `exportHandle {viewId?}` on `silica/window` exports the window and answers
//!   `"wayland:<handle>"` (the `parent_window` format of xdg-desktop-portal), or null without
//!   xdg-foreign. The export lives as long as the window: the handle stays valid for every
//!   dialog the app opens over it.
//! - `--parent <handle>` imports another app's window and makes it the parent of this
//!   process's window before Ossido places it: the portal's Open and Save dialog opens over
//!   the app that asked for it, stays above it, and gives the focus back to it.

/// The prefix of a Wayland window identifier for xdg-desktop-portal (`parent_window`).
pub const HANDLE_PREFIX: &str = "wayland:";

/// The handle of `--parent`: bare or with the portal's `wayland:` prefix; none if empty.
pub fn parent_handle(value: &str) -> Option<String> {
    let handle = value.strip_prefix(HANDLE_PREFIX).unwrap_or(value).trim();
    (!handle.is_empty()).then(|| handle.to_owned())
}

/// What `exportHandle` answers for a handle from the compositor.
pub fn window_identifier(handle: &str) -> String {
    format!("{HANDLE_PREFIX}{handle}")
}

/// A window's export: the protocol object, the handle once the compositor sent it, and the
/// `exportHandle` calls waiting for it.
pub struct Export<O, R> {
    pub object: O,
    pub handle: Option<String>,
    pub waiting: Vec<R>,
}

impl<O, R> Export<O, R> {
    pub fn new(object: O) -> Self {
        Self {
            object,
            handle: None,
            waiting: Vec::new(),
        }
    }

    /// The window identifier if the handle is known; otherwise `reply` is taken to wait
    /// for it.
    pub fn ask(&mut self, reply: &mut Option<R>) -> Option<String> {
        if let Some(handle) = &self.handle {
            return Some(window_identifier(handle));
        }
        self.waiting.extend(reply.take());
        None
    }

    /// The compositor sent the handle: the calls waiting for it, to answer with the returned
    /// identifier.
    pub fn ready(&mut self, handle: String) -> (String, Vec<R>) {
        let identifier = window_identifier(&handle);
        self.handle = Some(handle);
        (identifier, std::mem::take(&mut self.waiting))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_handles_are_bare_or_prefixed() {
        assert_eq!(parent_handle("abc123").as_deref(), Some("abc123"));
        assert_eq!(parent_handle("wayland:abc123").as_deref(), Some("abc123"));
        assert_eq!(parent_handle(""), None);
        assert_eq!(parent_handle("wayland:"), None);
    }

    #[test]
    fn export_answers_now_or_once_the_handle_arrives() {
        let mut export: Export<(), u32> = Export::new(());
        let mut first = Some(1);
        assert_eq!(export.ask(&mut first), None);
        assert_eq!(first, None);
        assert_eq!(export.ask(&mut Some(2)), None);
        assert_eq!(export.ask(&mut None), None);
        let (identifier, waiting) = export.ready("h".to_owned());
        assert_eq!(identifier, "wayland:h");
        assert_eq!(waiting, [1, 2]);
        let mut third = Some(3);
        assert_eq!(export.ask(&mut third).as_deref(), Some("wayland:h"));
        assert_eq!(third, Some(3));
        assert!(export.waiting.is_empty());
    }
}
