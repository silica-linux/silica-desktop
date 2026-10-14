//! The `silica/shell` channel, for the shell components: the open windows
//! (`ext-foreign-toplevel-list-v1`) to Dart, and `activate` and `launch` to Ossido
//! (`silica-shell-v1`). Outside the shell Ossido does not show these protocols: the
//! calls answer `false` and the list stays empty.
//!
//! Messages (JSON method codec):
//! - from Dart: `list` → the list; `focused` → the focused identifier;
//!   `activate {id}`; `launch {command}`; `execute {id}` (catalog command) → `true`
//!   if sent;
//! - to Dart: `toplevels [{id, title, app_id, minimized}]` and `focused id` on every
//!   change, `command id` when Ossido forwards a command to the shell (e.g.
//!   `faro.toggle`), `broadcast {topic, value}` (a state of another shell component),
//!   `minimized [id…]` (the minimized ones, from the first minimized); `windowMenu {id, x,
//!   y}` (long press on the green button: the arrangement popover with its tip at x, y);
//! - `thumbnail {id}` from Dart asks for the thumbnail of a minimized window: it arrives as
//!   `thumbnail {id, width, height, path}`, a premultiplied RGBA file that Dart reads and
//!   deletes (width 0 if there is none).

use serde_json::{Value, json};
use silica_protocols::shell::client::silica_shell_manager_v1::SilicaShellManagerV1;
use wayland_client::{Proxy, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1;

use crate::text_input::success;

pub const CHANNEL: &str = "silica/shell";

/// Maximum side of the thumbnails (silica-shell-v1 version 2).
const THUMBNAIL_MAX: u32 = 256;

/// A window announced by Ossido.
struct Toplevel {
    handle: ExtForeignToplevelHandleV1,
    identifier: String,
    title: String,
    app_id: String,
}

pub struct ShellLink {
    manager: Option<SilicaShellManagerV1>,
    /// The window list: kept alive so that Ossido keeps announcing them.
    _list: Option<ExtForeignToplevelListV1>,
    toplevels: Vec<Toplevel>,
    /// The focused window (identifier), empty if none.
    focused: String,
    /// The minimized windows, from the first minimized.
    minimized: Vec<String>,
    /// The thumbnails asked for and not yet arrived: identifier and file.
    thumbnails: std::cell::RefCell<Vec<(String, std::path::PathBuf)>>,
    serial: std::cell::Cell<u64>,
}

impl ShellLink {
    /// The shell globals, if Ossido shows them to this process.
    pub fn new(
        manager: Option<SilicaShellManagerV1>,
        list: Option<ExtForeignToplevelListV1>,
    ) -> Self {
        Self {
            manager,
            _list: list,
            toplevels: Vec::new(),
            focused: String::new(),
            minimized: Vec::new(),
            thumbnails: std::cell::RefCell::new(Vec::new()),
            serial: std::cell::Cell::new(0),
        }
    }

    /// The focus changed: the `focused` message for Dart.
    pub fn set_focused(&mut self, identifier: String) -> Vec<u8> {
        self.focused = identifier;
        serde_json::to_vec(&json!({ "method": "focused", "args": self.focused }))
            .unwrap_or_default()
    }

    /// A new window: the details arrive with the events of its handle.
    pub fn add(&mut self, handle: ExtForeignToplevelHandleV1) {
        self.toplevels.push(Toplevel {
            handle,
            identifier: String::new(),
            title: String::new(),
            app_id: String::new(),
        });
    }

    /// An event of a handle. Returns true when the list must be sent to Dart
    /// (`done` or `closed`).
    pub fn event(
        &mut self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
    ) -> bool {
        let Some(index) = self.toplevels.iter().position(|t| &t.handle == handle) else {
            return false;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                if let Some(t) = self.toplevels.get_mut(index) {
                    t.identifier = identifier;
                }
                false
            }
            ext_foreign_toplevel_handle_v1::Event::Title { title } => {
                if let Some(t) = self.toplevels.get_mut(index) {
                    t.title = title;
                }
                false
            }
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                if let Some(t) = self.toplevels.get_mut(index) {
                    t.app_id = app_id;
                }
                false
            }
            ext_foreign_toplevel_handle_v1::Event::Done => true,
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                let closed = self.toplevels.remove(index);
                closed.handle.destroy();
                true
            }
            _ => false,
        }
    }

    /// A window minimized or restored: the `minimized` message for Dart.
    pub fn set_minimized(&mut self, identifier: String, minimized: bool) -> Vec<u8> {
        self.minimized.retain(|id| *id != identifier);
        if minimized {
            self.minimized.push(identifier);
        }
        serde_json::to_vec(&json!({ "method": "minimized", "args": self.minimized }))
            .unwrap_or_default()
    }

    /// Asks Ossido for the thumbnail into a new file in `$XDG_RUNTIME_DIR`.
    fn request_thumbnail(&self, identifier: &str) -> bool {
        let Some(manager) = self.manager.as_ref().filter(|m| m.version() >= 2) else {
            return false;
        };
        if identifier.is_empty() {
            return false;
        }
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map_or_else(std::env::temp_dir, std::path::PathBuf::from)
            .join(format!("silica-thumbnails-{}", std::process::id()));
        let serial = self.serial.get().wrapping_add(1);
        self.serial.set(serial);
        let path = dir.join(format!("{serial}.rgba"));
        let size = u64::from(THUMBNAIL_MAX) * u64::from(THUMBNAIL_MAX) * 4;
        let file = std::fs::create_dir_all(&dir).and_then(|()| {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)?;
            file.set_len(size)?;
            Ok(file)
        });
        match file {
            Ok(file) => {
                manager.thumbnail(identifier.to_owned(), std::os::fd::AsFd::as_fd(&file));
                self.thumbnails
                    .borrow_mut()
                    .push((identifier.to_owned(), path));
                true
            }
            Err(err) => {
                tracing::warn!("thumbnail: {err}");
                false
            }
        }
    }

    /// The thumbnail is in the file: the `thumbnail` message for Dart.
    pub fn thumbnail_ready(&self, identifier: &str, width: u32, height: u32) -> Vec<u8> {
        let mut pending = self.thumbnails.borrow_mut();
        let Some(index) = pending.iter().position(|(id, _)| id == identifier) else {
            return Vec::new();
        };
        let (_, path) = pending.remove(index);
        if width == 0 || height == 0 || width > THUMBNAIL_MAX || height > THUMBNAIL_MAX {
            let _ = std::fs::remove_file(&path);
            return serde_json::to_vec(&json!({
                "method": "thumbnail",
                "args": {"id": identifier, "width": 0, "height": 0, "path": ""},
            }))
            .unwrap_or_default();
        }
        serde_json::to_vec(&json!({
            "method": "thumbnail",
            "args": {"id": identifier, "width": width, "height": height, "path": path},
        }))
        .unwrap_or_default()
    }

    /// The window list, as Dart sees it.
    pub fn list(&self) -> Value {
        Value::Array(
            self.toplevels
                .iter()
                .filter(|t| !t.identifier.is_empty())
                .map(|t| {
                    json!({
                        "id": t.identifier,
                        "title": t.title,
                        "app_id": t.app_id,
                        "minimized": self.minimized.contains(&t.identifier),
                    })
                })
                .collect(),
        )
    }

    /// The `toplevels` message for Dart.
    pub fn toplevels_message(&self) -> Vec<u8> {
        serde_json::to_vec(&json!({ "method": "toplevels", "args": self.list() }))
            .unwrap_or_default()
    }

    /// A call from Dart; returns the reply.
    pub fn handle_call<D: 'static>(&self, message: &[u8], _qh: &QueueHandle<D>) -> Vec<u8> {
        let Ok(call) = serde_json::from_slice::<Value>(message) else {
            return Vec::new();
        };
        let args = call.get("args").cloned().unwrap_or(Value::Null);
        let text = |name: &str| {
            args.get(name)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_default()
        };
        match call.get("method").and_then(Value::as_str) {
            Some("list") => success(self.list()),
            Some("minimized") => success(json!(self.minimized)),
            Some("thumbnail") => success(json!(self.request_thumbnail(&text("id")))),
            Some("focused") => success(json!(self.focused)),
            Some("activate") => {
                let sent = self.manager.as_ref().is_some_and(|manager| {
                    manager.activate(text("id"));
                    true
                });
                success(json!(sent))
            }
            Some("execute") => {
                let id = text("id");
                // With parameters (a JSON object) version 5 is needed.
                let params = call
                    .pointer("/args/params")
                    .filter(|p| p.is_object())
                    .map(ToString::to_string);
                let sent = !id.is_empty()
                    && self.manager.as_ref().is_some_and(|manager| match params {
                        Some(params) if manager.version() >= 5 => {
                            manager.execute_with(id, params);
                            true
                        }
                        Some(_) => false,
                        None => {
                            manager.execute(id);
                            true
                        }
                    });
                success(json!(sent))
            }
            Some("broadcast") => {
                let topic = text("topic");
                let value = text("value");
                let sent = !topic.is_empty()
                    && self
                        .manager
                        .as_ref()
                        .is_some_and(|manager| manager.version() >= 4)
                    && self.manager.as_ref().is_some_and(|manager| {
                        manager.broadcast(topic, value);
                        true
                    });
                success(json!(sent))
            }
            Some("launch") => {
                let command = text("command");
                let sent = !command.is_empty()
                    && self.manager.as_ref().is_some_and(|manager| {
                        manager.launch(command);
                        true
                    });
                success(json!(sent))
            }
            _ => Vec::new(),
        }
    }
}
