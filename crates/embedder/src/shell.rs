//! Il canale `silica/shell`, per i componenti della shell: le finestre aperte
//! (`ext-foreign-toplevel-list-v1`) verso Dart, e `activate` e `launch` verso Ossido
//! (`silica-shell-v1`). Fuori dalla shell Ossido non mostra questi protocolli: le
//! chiamate rispondono `false` e l'elenco resta vuoto.
//!
//! Messaggi (JSON method codec):
//! - da Dart: `list` → l'elenco; `activate {id}`; `launch {command}` → `true` se inviato;
//! - verso Dart: `toplevels [{id, title, app_id}]` a ogni cambiamento.

use serde_json::{Value, json};
use silica_protocols::shell::client::silica_shell_manager_v1::SilicaShellManagerV1;
use wayland_client::QueueHandle;
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1;

use crate::text_input::success;

pub const CHANNEL: &str = "silica/shell";

/// Una finestra annunciata da Ossido.
struct Toplevel {
    handle: ExtForeignToplevelHandleV1,
    identifier: String,
    title: String,
    app_id: String,
}

pub struct ShellLink {
    manager: Option<SilicaShellManagerV1>,
    /// La lista delle finestre: tenuta viva perché Ossido continui ad annunciarle.
    _list: Option<ExtForeignToplevelListV1>,
    toplevels: Vec<Toplevel>,
}

impl ShellLink {
    /// I global della shell, se Ossido li mostra a questo processo.
    pub fn new(
        manager: Option<SilicaShellManagerV1>,
        list: Option<ExtForeignToplevelListV1>,
    ) -> Self {
        Self {
            manager,
            _list: list,
            toplevels: Vec::new(),
        }
    }

    /// Una finestra nuova: i dettagli arrivano con gli eventi del suo handle.
    pub fn add(&mut self, handle: ExtForeignToplevelHandleV1) {
        self.toplevels.push(Toplevel {
            handle,
            identifier: String::new(),
            title: String::new(),
            app_id: String::new(),
        });
    }

    /// Un evento di un handle. Restituisce true quando l'elenco va mandato a Dart
    /// (`done` o `closed`).
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

    /// L'elenco delle finestre, come lo vede Dart.
    pub fn list(&self) -> Value {
        Value::Array(
            self.toplevels
                .iter()
                .filter(|t| !t.identifier.is_empty())
                .map(|t| json!({ "id": t.identifier, "title": t.title, "app_id": t.app_id }))
                .collect(),
        )
    }

    /// Il messaggio `toplevels` per Dart.
    pub fn toplevels_message(&self) -> Vec<u8> {
        serde_json::to_vec(&json!({ "method": "toplevels", "args": self.list() }))
            .unwrap_or_default()
    }

    /// Una chiamata da Dart; restituisce la risposta.
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
            Some("activate") => {
                let sent = self.manager.as_ref().is_some_and(|manager| {
                    manager.activate(text("id"));
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
