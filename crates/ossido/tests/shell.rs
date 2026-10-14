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
use serde_json::{Value, json};
use silica_control::commands::{INPUT_KEY, WINDOW_LIST, WindowInfo};
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
const CHILD_LISTEN: &str = "SILICA_TEST_SHELL_LISTEN";
const CHILD_THUMBNAIL: &str = "SILICA_TEST_SHELL_THUMBNAIL";

#[derive(Default)]
struct Shell {
    globals: Vec<String>,
    list: Option<ExtForeignToplevelListV1>,
    manager: Option<SilicaShellManagerV1>,
    /// (identificatore, titolo) delle finestre annunciate.
    toplevels: Vec<(String, String)>,
    /// Gli identificatori annunciati con `focused`.
    focused: Vec<String>,
    /// Gli eventi `minimized` (identificatore, stato) e la miniatura ricevuta.
    minimized: Vec<(String, u32)>,
    thumbnail: Option<(u32, u32)>,
    /// I comandi inoltrati da Ossido.
    commands: Vec<String>,
    /// I popover di disposizione chiesti (identificatore, x, y).
    window_menus: Vec<(String, i32, i32)>,
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
                "silica_shell_manager_v1" => state.manager = Some(registry.bind(name, 3, qh, ())),
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
            silica_shell_manager_v1::Event::Minimized {
                identifier,
                minimized,
            } => state.minimized.push((identifier, minimized)),
            silica_shell_manager_v1::Event::Thumbnail { width, height, .. } => {
                state.thumbnail = Some((width, height));
            }
            silica_shell_manager_v1::Event::WindowMenu { identifier, x, y } => {
                state.window_menus.push((identifier, x, y));
            }
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
    // Le finestre ridotte annunciate al collegamento e la miniatura della prima.
    if std::env::var_os(CHILD_THUMBNAIL).is_some() {
        if let (Some(manager), Some((identifier, _))) =
            (shell.manager.clone(), shell.minimized.first().cloned())
        {
            let path = std::env::temp_dir().join(format!("silica-thumb-{}", std::process::id()));
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .unwrap();
            file.set_len(256 * 256 * 4).unwrap();
            manager.thumbnail(identifier.clone(), std::os::fd::AsFd::as_fd(&file));
            queue.roundtrip(&mut shell).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            let _ = std::fs::remove_file(&path);
            let (w, h) = shell.thumbnail.unwrap_or_default();
            let opaque = bytes
                .chunks(4)
                .take((w * h) as usize)
                .filter(|px| px[3] > 200)
                .count();
            println!("MINIMIZED {}", shell.minimized.len());
            println!("THUMBNAIL {w} {h} {opaque}");
        }
        return;
    }
    // In ascolto per qualche secondo: il test padre intanto preme i tasti.
    if let Ok(ms) = std::env::var(CHILD_LISTEN) {
        println!("READY");
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(ms.parse().unwrap());
        while std::time::Instant::now() < deadline {
            queue.roundtrip(&mut shell).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        for id in &shell.commands {
            println!("COMMAND {id}");
        }
        for (_, x, y) in &shell.window_menus {
            println!("WINDOW_MENU {x} {y}");
        }
        return;
    }
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
        manager.execute("launcher.toggle".to_owned());
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
    assert!(shell.contains("COMMAND launcher.toggle"), "{shell}");

    // Il figlio della shell ha attivato "prima": ora ha il focus ed è in cima.
    let list: Vec<WindowInfo> =
        serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap();
    assert_eq!(list[0].title.as_deref(), Some("prima"), "{list:?}");
    assert!(list[0].focused);
}

#[test]
fn the_logo_key_drives_the_app_switcher() {
    let ossido = Headless::start("shell-switcher");
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "shell_child",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_SOCKET, &ossido.wayland)
        .env(CHILD_LISTEN, "2500")
        .env("SILICA_SHELL", "1")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(800));
    let mut c = ossido.client();
    let mut key = |key: &str, pressed: Option<bool>| {
        let params = match pressed {
            Some(pressed) => json!({"key": key, "pressed": pressed}),
            None => json!({"key": key}),
        };
        c.call(INPUT_KEY, params).unwrap();
    };
    // ⌘Tab Tab, poi ⌘ rilasciato: due passi e la conferma.
    key("Super_L", Some(true));
    key("Tab", None);
    key("Tab", None);
    key("Super_L", Some(false));
    // ⌘⇧Tab ed Esc: indietro e annullato; il rilascio dopo non conferma niente.
    key("Super_L", Some(true));
    key("Shift_L", Some(true));
    key("Tab", None);
    key("Shift_L", Some(false));
    key("Escape", None);
    key("Super_L", Some(false));
    let output = child.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    let commands: Vec<&str> = out
        .lines()
        .filter_map(|l| l.strip_prefix("COMMAND "))
        .collect();
    assert_eq!(
        commands,
        [
            "switcher.next",
            "switcher.next",
            "switcher.commit",
            "switcher.previous",
            "switcher.cancel"
        ],
        "{out}"
    );
}

#[test]
fn activating_a_window_brings_all_the_windows_of_its_app() {
    use common::client::APP_ID;
    use silica_control::commands::WINDOW_FOCUS;
    let ossido = Headless::start("shell-app");
    APP_ID.with(|id| *id.borrow_mut() = "uno".to_owned());
    let _first = TestWindow::open(&ossido.wayland, "prima");
    APP_ID.with(|id| *id.borrow_mut() = "due".to_owned());
    let _other = TestWindow::open(&ossido.wayland, "altra");
    APP_ID.with(|id| *id.borrow_mut() = "uno".to_owned());
    let _third = TestWindow::open(&ossido.wayland, "terza");
    APP_ID.with(|id| id.borrow_mut().clear());
    let titles = |ossido: &Headless| -> Vec<String> {
        let list: Vec<WindowInfo> =
            serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap())
                .unwrap();
        list.iter().filter_map(|w| w.title.clone()).collect()
    };
    // "altra" in cima: prima < terza < altra.
    let other = {
        let list: Vec<WindowInfo> =
            serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap())
                .unwrap();
        list.into_iter()
            .find(|w| w.title.as_deref() == Some("altra"))
            .unwrap()
    };
    ossido
        .client()
        .call(WINDOW_FOCUS, json!({"id": other.id}))
        .unwrap();
    assert_eq!(titles(&ossido), ["altra", "terza", "prima"]);
    // Il figlio della shell attiva "prima": vengono davanti tutte e due le finestre di "uno",
    // con "prima" (quella chiesta) in cima.
    run_child(&ossido.wayland, true);
    assert_eq!(titles(&ossido), ["prima", "terza", "altra"]);
}

#[test]
fn minimized_windows_are_announced_with_a_thumbnail() {
    let ossido = Headless::start_with("shell-thumbnail", (640, 400), &["--animation-scale", "1"]);
    let _window = TestWindow::open_sized(&ossido.wayland, "ridotta", (300, 200));
    ossido
        .client()
        .call(silica_control::commands::WINDOW_MINIMIZE, Value::Null)
        .unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "shell_child",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_SOCKET, &ossido.wayland)
        .env(CHILD_THUMBNAIL, "1")
        .env("SILICA_SHELL", "1")
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(out.contains("MINIMIZED 1"), "{out}");
    // La cornice 300 × 200 (senza ombra) sta nei 256 pt: ridotta a 256 × ~171.
    let line = out
        .lines()
        .find(|l| l.starts_with("THUMBNAIL"))
        .unwrap_or_else(|| panic!("{out}"));
    let numbers: Vec<u32> = line
        .split_whitespace()
        .skip(1)
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(numbers[0], 256, "{line}");
    assert!((160..=180).contains(&numbers[1]), "{line}");
    // Quasi tutti i pixel sono pieni (la finestra rossa, senza ombra intorno).
    assert!(numbers[2] > 256 * 160 * 9 / 10, "{line}");
}

/// Pressione lunga sul pulsante verde: la shell riceve il popover di disposizione con la
/// punta sotto il pulsante, e il rilascio non porta la finestra a schermo intero.
#[test]
fn a_long_press_on_the_zoom_button_asks_the_shell_for_the_window_menu() {
    use silica_control::commands::{INPUT_POINTER_BUTTON, INPUT_POINTER_MOVE};
    let ossido = Headless::start("shell-window-menu");
    let _window = TestWindow::open_decorated(&ossido.wayland, "disponi", (300, 200));
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "shell_child",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_SOCKET, &ossido.wayland)
        .env(CHILD_LISTEN, "2500")
        .env("SILICA_SHELL", "1")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut c = ossido.client();
    let list: Vec<WindowInfo> =
        serde_json::from_value(c.call(WINDOW_LIST, Value::Null).unwrap()).unwrap();
    let info = &list[0];
    // Il pulsante verde: centro x 72, y 26 della barra, che sta sopra il contenuto.
    let (bx, by) = (info.x + 72, info.y - 52 + 26);
    c.call(INPUT_POINTER_MOVE, json!({"x": bx, "y": by}))
        .unwrap();
    c.call(
        INPUT_POINTER_BUTTON,
        json!({"button": "left", "pressed": true}),
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1200));
    c.call(
        INPUT_POINTER_BUTTON,
        json!({"button": "left", "pressed": false}),
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    let menu = out
        .lines()
        .find(|l| l.starts_with("WINDOW_MENU"))
        .unwrap_or_else(|| panic!("nessun popover: {out}"));
    // La punta sotto il pulsante: x del centro, y della base del disco (raggio 7).
    assert_eq!(menu, format!("WINDOW_MENU {} {}", bx, by + 7));
    let list: Vec<WindowInfo> =
        serde_json::from_value(c.call(WINDOW_LIST, Value::Null).unwrap()).unwrap();
    assert!(!list[0].fullscreen, "{list:?}");
}
