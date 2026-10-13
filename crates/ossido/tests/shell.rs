//! Protocolli della shell: solo i componenti della shell (`SILICA_SHELL=1`) vedono
//! l'elenco delle finestre e `silica_shell_manager_v1`, che attiva una finestra.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::os::unix::net::UnixStream;
use std::process::Command;

use common::Headless;
use common::client::TestWindow;
use serde_json::Value;
use silica_control::commands::{WINDOW_LIST, WindowInfo};
use silica_protocols::shell::client::silica_shell_manager_v1::{self, SilicaShellManagerV1};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, QueueHandle, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, EVT_TOPLEVEL_OPCODE, ExtForeignToplevelListV1,
};

const CHILD_SOCKET: &str = "SILICA_TEST_SHELL_SOCKET";

#[derive(Default)]
struct Shell {
    globals: Vec<String>,
    list: Option<ExtForeignToplevelListV1>,
    manager: Option<SilicaShellManagerV1>,
    /// (identificatore, titolo) delle finestre annunciate.
    toplevels: Vec<(String, String)>,
    /// Gli identificatori annunciati con `focused`.
    focused: Vec<String>,
    /// I comandi inoltrati da Ossido.
    commands: Vec<String>,
}

impl Dispatch<WlRegistry, ()> for Shell {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            match interface.as_str() {
                "ext_foreign_toplevel_list_v1" => {
                    state.list = Some(registry.bind(name, 1, qh, ()));
                }
                "silica_shell_manager_v1" => state.manager = Some(registry.bind(name, 1, qh, ())),
                _ => {}
            }
            state.globals.push(interface);
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for Shell {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { .. } = event {
            state.toplevels.push((String::new(), String::new()));
        }
    }

    event_created_child!(Shell, ExtForeignToplevelListV1, [
        EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for Shell {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(last) = state.toplevels.last_mut() else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => last.0 = identifier,
            ext_foreign_toplevel_handle_v1::Event::Title { title } => last.1 = title,
            _ => {}
        }
    }
}

impl Dispatch<SilicaShellManagerV1, ()> for Shell {
    fn event(
        state: &mut Self,
        _: &SilicaShellManagerV1,
        event: silica_shell_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            silica_shell_manager_v1::Event::Focused { identifier } => {
                state.focused.push(identifier)
            }
            silica_shell_manager_v1::Event::Command { id } => state.commands.push(id),
            _ => {}
        }
    }
}

/// Si collega, legge i global e le finestre; se è un componente della shell attiva la
/// finestra "prima". Stampa i global trovati, uno per riga.
fn shell_client(socket: &str) {
    let conn = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let mut shell = Shell::default();
    conn.display().get_registry(&qh, ());
    queue.roundtrip(&mut shell).unwrap();
    queue.roundtrip(&mut shell).unwrap();
    let target = shell
        .toplevels
        .iter()
        .find(|(_, title)| title == "prima")
        .map(|(identifier, _)| identifier.clone());
    if let (Some(manager), Some(identifier)) = (shell.manager.clone(), target) {
        manager.activate(identifier);
        queue.roundtrip(&mut shell).unwrap();
        // Un comando del catalogo chiesto dalla shell torna a tutta la shell.
        manager.execute("faro.toggle".to_owned());
        manager.execute("notifications.toggle".to_owned());
        queue.roundtrip(&mut shell).unwrap();
    }
    for id in &shell.commands {
        println!("COMMAND {id}");
    }
    for global in &shell.globals {
        println!("GLOBAL {global}");
    }
    for (_, title) in &shell.toplevels {
        println!("TOPLEVEL {title}");
    }
    for identifier in &shell.focused {
        let title = shell
            .toplevels
            .iter()
            .find(|(id, _)| id == identifier)
            .map_or("", |(_, title)| title.as_str());
        println!("FOCUSED {title}");
    }
}

/// Il processo figlio: gira solo quando il test padre lo lancia con il socket.
#[test]
fn shell_child() {
    if let Ok(socket) = std::env::var(CHILD_SOCKET) {
        shell_client(&socket);
    }
}

fn run_child(socket: &std::path::Path, shell: bool) -> String {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "shell_child",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_SOCKET, socket);
    if shell {
        command.env("SILICA_SHELL", "1");
    } else {
        command.env_remove("SILICA_SHELL");
    }
    let output = command.output().unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn only_shell_components_see_the_window_list_and_can_activate() {
    let ossido = Headless::start("shell");
    let _first = TestWindow::open(&ossido.wayland, "prima");
    let _second = TestWindow::open(&ossido.wayland, "seconda");

    let outsider = run_child(&ossido.wayland, false);
    assert!(outsider.contains("GLOBAL wl_compositor"), "{outsider}");
    assert!(!outsider.contains("silica_shell_manager_v1"), "{outsider}");
    assert!(
        !outsider.contains("ext_foreign_toplevel_list_v1"),
        "{outsider}"
    );

    let shell = run_child(&ossido.wayland, true);
    assert!(shell.contains("GLOBAL silica_shell_manager_v1"), "{shell}");
    assert!(
        shell.contains("TOPLEVEL prima") && shell.contains("TOPLEVEL seconda"),
        "{shell}"
    );

    // Al collegamento il focus era su "seconda"; dopo l'attivazione su "prima".
    let focused: Vec<&str> = shell.lines().filter(|l| l.starts_with("FOCUSED")).collect();
    assert_eq!(focused, ["FOCUSED seconda", "FOCUSED prima"], "{shell}");

    assert!(shell.contains("COMMAND faro.toggle"), "{shell}");
    assert!(shell.contains("COMMAND notifications.toggle"), "{shell}");

    // Il figlio della shell ha attivato "prima": ora ha il focus ed è in cima.
    let list: Vec<WindowInfo> =
        serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap();
    assert_eq!(list[0].title.as_deref(), Some("prima"), "{list:?}");
    assert!(list[0].focused);
}
