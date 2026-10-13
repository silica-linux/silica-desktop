//! I protocolli della shell: `ext-foreign-toplevel-list` (le finestre aperte, per il Dock)
//! e `silica-shell-v1` (attivare una finestra, avviare un'app).
//!
//! Solo i componenti della shell li vedono: processi con `SILICA_SHELL=1` nell'ambiente,
//! che Ossido mette quando li lancia (`--shell`, `app.launch` con `shell: true`). Non è
//! una barriera contro altri processi dello stesso utente, che possono mettere la stessa
//! variabile: quella arriverà con security-context (sandbox delle app).

use silica_protocols::shell::SHELL_ENV;
use silica_protocols::shell::server::silica_shell_manager_v1::{self, SilicaShellManagerV1};
use smithay::delegate_foreign_toplevel_list;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::wayland::compositor::with_states;
use smithay::wayland::foreign_toplevel_list::{
    ForeignToplevelHandle, ForeignToplevelListHandler, ForeignToplevelListState,
};
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData};
use tracing::{info, warn};

use crate::state::State;

/// Il processo all'altro capo del socket è un componente della shell (vedi il commento
/// del modulo): pid da SO_PEERCRED, ambiente da `/proc/<pid>/environ`.
pub fn stream_is_shell(stream: &std::os::unix::net::UnixStream) -> bool {
    let Ok(credentials) = rustix::net::sockopt::socket_peercred(stream) else {
        return false;
    };
    let pid = credentials.pid.as_raw_nonzero();
    let Ok(environ) = std::fs::read(format!("/proc/{pid}/environ")) else {
        return false;
    };
    let marker = format!("{SHELL_ENV}=1");
    environ
        .split(|byte| *byte == 0)
        .any(|entry| entry == marker.as_bytes())
}

/// Il client è un componente della shell (deciso alla connessione).
pub fn is_shell_client(client: &Client) -> bool {
    client
        .get_data::<crate::state::ClientState>()
        .is_some_and(|data| data.shell)
}

pub struct ShellManagerState;

impl ShellManagerState {
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<State, SilicaShellManagerV1, _>(
            1,
            ShellGlobal {
                filter: Box::new(is_shell_client),
            },
        );
        Self
    }
}

pub struct ShellGlobal {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

/// La lista delle finestre per la shell, con lo stesso filtro.
pub fn toplevel_list(dh: &DisplayHandle) -> ForeignToplevelListState {
    ForeignToplevelListState::new_with_filter::<State>(dh, is_shell_client)
}

/// Titolo e app id di un toplevel.
fn title_and_app_id(toplevel: &ToplevelSurface) -> (String, String) {
    with_states(toplevel.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|data| data.lock().ok())
            .map(|data| {
                (
                    data.title.clone().unwrap_or_default(),
                    data.app_id.clone().unwrap_or_default(),
                )
            })
            .unwrap_or_default()
    })
}

impl State {
    /// Una finestra nuova entra nell'elenco della shell.
    pub fn publish_toplevel(&mut self, window: &Window) {
        let Some(toplevel) = window.toplevel() else {
            return;
        };
        let (title, app_id) = title_and_app_id(toplevel);
        let handle = self
            .foreign_toplevel_list
            .new_toplevel::<State>(title, app_id);
        window.user_data().insert_if_missing(|| handle);
    }

    /// Titolo o app id cambiati: la shell li riceve.
    pub fn update_toplevel(&mut self, surface: &ToplevelSurface) {
        let Some(window) = self
            .space
            .elements()
            .chain(self.minimized.iter().map(|(w, _)| w))
            .find(|w| w.toplevel() == Some(surface))
        else {
            return;
        };
        let Some(handle) = window.user_data().get::<ForeignToplevelHandle>() else {
            return;
        };
        let (title, app_id) = title_and_app_id(surface);
        handle.send_title(&title);
        handle.send_app_id(&app_id);
        handle.send_done();
    }

    /// La finestra è chiusa: esce dall'elenco.
    pub fn unpublish_toplevel(&mut self, surface: &ToplevelSurface) {
        let window = self
            .space
            .elements()
            .chain(self.minimized.iter().map(|(w, _)| w))
            .find(|w| w.toplevel() == Some(surface))
            .cloned();
        if let Some(handle) = window
            .as_ref()
            .and_then(|w| w.user_data().get::<ForeignToplevelHandle>())
        {
            self.foreign_toplevel_list.remove_toplevel(handle);
        }
        self.minimized
            .retain(|(w, _)| w.toplevel() != Some(surface));
    }

    /// L'identificatore della finestra di questa superficie, vuoto se non è una finestra.
    fn identifier_of(&self, focus: Option<&WlSurface>) -> String {
        focus
            .and_then(|focus| {
                self.space
                    .elements()
                    .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == focus))
                    .and_then(|w| w.user_data().get::<ForeignToplevelHandle>())
                    .map(ForeignToplevelHandle::identifier)
            })
            .unwrap_or_default()
    }

    /// L'identificatore della finestra con il focus, vuoto se nessuna.
    pub fn focused_identifier(&self) -> String {
        let focus = self.seat.get_keyboard().and_then(|k| k.current_focus());
        self.identifier_of(focus.as_ref())
    }

    /// Manda la finestra con il focus ai componenti della shell (se è cambiata). Si chiama
    /// dal callback del cambio di focus, con la superficie: rileggere il focus dalla
    /// tastiera lì dentro ne riprenderebbe il lock.
    pub fn announce_focus(&mut self, focus: Option<&WlSurface>) {
        let identifier = self.identifier_of(focus);
        if self.announced_focus.as_deref() == Some(identifier.as_str()) {
            return;
        }
        self.shell_managers.retain(|manager| manager.is_alive());
        for manager in &self.shell_managers {
            manager.focused(identifier.clone());
        }
        self.announced_focus = Some(identifier);
    }

    /// La finestra con questo identificatore della lista: focus, e torna se era ridotta.
    pub fn activate_identifier(&mut self, identifier: &str) {
        let window = self
            .space
            .elements()
            .chain(self.minimized.iter().map(|(w, _)| w))
            .find(|w| {
                w.user_data()
                    .get::<ForeignToplevelHandle>()
                    .is_some_and(|handle| handle.identifier() == identifier)
            })
            .cloned();
        let Some(window) = window else {
            return;
        };
        self.restore_window(&window);
        self.focus_window(Some(window));
    }
}

impl ForeignToplevelListHandler for State {
    fn foreign_toplevel_list_state(&mut self) -> &mut ForeignToplevelListState {
        &mut self.foreign_toplevel_list
    }
}

delegate_foreign_toplevel_list!(State);

impl GlobalDispatch<SilicaShellManagerV1, ShellGlobal> for State {
    fn bind(
        state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<SilicaShellManagerV1>,
        _global_data: &ShellGlobal,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(resource, ());
        manager.focused(state.focused_identifier());
        state.shell_managers.push(manager);
    }

    fn can_view(client: Client, global_data: &ShellGlobal) -> bool {
        (global_data.filter)(&client)
    }
}

impl Dispatch<SilicaShellManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _manager: &SilicaShellManagerV1,
        request: silica_shell_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            silica_shell_manager_v1::Request::Activate { identifier } => {
                state.activate_identifier(&identifier);
            }
            silica_shell_manager_v1::Request::Launch { command } => match state.launch(&command) {
                Ok(pid) => info!("shell: avviato {command} (pid {pid})"),
                Err(err) => warn!("shell: {err}"),
            },
            silica_shell_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}
