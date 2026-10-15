//! The apps' menus in the global menu bar (silica-menu-v1, silica-desktop#25): an app
//! publishes its model, the shell receives the frontmost app's model (silica-shell-v1
//! version 8) and its choice goes back to the app.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::io::{BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::Headless;
use common::client::{APP_ID, TestWindow};
use serde_json::{Value, json};
use silica_control::commands::{
    APP_MENU, APP_MENU_ACTIVATE, WINDOW_FOCUS, WINDOW_LIST, WindowInfo,
};
use silica_protocols::shell::client::silica_shell_manager_v1::{self, SilicaShellManagerV1};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, QueueHandle};

const CHILD_SOCKET: &str = "SILICA_TEST_MENU_SOCKET";
const CHILD_LISTEN: &str = "SILICA_TEST_MENU_LISTEN";

/// The item the shell child chooses when it sees it.
const CHOSEN: u32 = 7;

#[derive(Default)]
struct Shell {
    manager: Option<SilicaShellManagerV1>,
    /// The `app_menu` events: app id, serial, model (empty without one).
    menus: Vec<(String, u32, String)>,
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
            && interface == "silica_shell_manager_v1"
        {
            state.manager = Some(registry.bind(name, 8, qh, ()));
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
        if let silica_shell_manager_v1::Event::AppMenu {
            app_id,
            serial,
            fd,
            size,
        } = event
        {
            let mut file = std::fs::File::from(fd);
            let mut model = String::new();
            file.read_to_string(&mut model).unwrap();
            assert_eq!(model.len(), size as usize);
            // The file is sealed: the shell cannot change what the others read.
            assert!(std::io::Write::write_all(&mut file, b"x").is_err());
            state.menus.push((app_id, serial, model));
        }
    }
}

/// The shell child: listens for a while, chooses `CHOSEN` the first time it shows up, then
/// prints the menus received, one per line.
fn shell_child(socket: &str, ms: u64) {
    let conn = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let mut shell = Shell::default();
    conn.display().get_registry(&qh, ());
    queue.roundtrip(&mut shell).unwrap();
    queue.roundtrip(&mut shell).unwrap();
    println!("READY");
    let deadline = Instant::now() + Duration::from_millis(ms);
    let mut chosen = false;
    while Instant::now() < deadline {
        queue.roundtrip(&mut shell).unwrap();
        if !chosen
            && let Some((_, serial, _)) = shell
                .menus
                .iter()
                .find(|(_, _, model)| model.contains(&format!("\"id\":{CHOSEN}")))
            && let Some(manager) = &shell.manager
        {
            manager.activate_menu_item(*serial, CHOSEN);
            // An old serial does nothing.
            manager.activate_menu_item(serial.wrapping_add(1000), CHOSEN);
            chosen = true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    for (app_id, serial, model) in &shell.menus {
        let labels: Vec<String> = serde_json::from_str::<Value>(model)
            .ok()
            .and_then(|m| m["menus"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .flat_map(|menu| menu["items"].as_array().cloned().unwrap_or_default())
            .filter_map(|item| item["label"].as_str().map(str::to_owned))
            .collect();
        println!(
            "APP_MENU {app_id} {} {}",
            if *serial == 0 { "none" } else { "some" },
            labels.join(",")
        );
    }
}

#[test]
fn menu_shell_child() {
    if let (Ok(socket), Ok(ms)) = (std::env::var(CHILD_SOCKET), std::env::var(CHILD_LISTEN)) {
        shell_child(&socket, ms.parse().unwrap());
    }
}

fn model(label: &str) -> String {
    json!({"menus": [
        {"label": "Calc", "role": "app", "items": [
            {"label": "", "role": "quit", "shortcut": {"key": "Q", "meta": true}}
        ]},
        {"label": "View", "items": [
            {"id": CHOSEN, "label": label, "checked": true, "shortcut": {"key": "1", "meta": true}}
        ]}
    ]})
    .to_string()
}

fn window_id(ossido: &Headless, title: &str) -> u64 {
    let list: Vec<WindowInfo> =
        serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap();
    list.into_iter()
        .find(|w| w.title.as_deref() == Some(title))
        .unwrap()
        .id
}

#[test]
fn the_bar_receives_the_frontmost_apps_menus_and_the_choice_goes_back() {
    let ossido = Headless::start("app-menu");
    APP_ID.with(|id| *id.borrow_mut() = "org.silica.Calc".to_owned());
    let mut calc = TestWindow::open(&ossido.wayland, "calc");
    calc.set_menu_model(&model("Basic"));
    APP_ID.with(|id| *id.borrow_mut() = "org.silica.Other".to_owned());
    let _other = TestWindow::open(&ossido.wayland, "other");
    APP_ID.with(|id| id.borrow_mut().clear());

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "menu_shell_child",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_SOCKET, &ossido.wayland)
        .env(CHILD_LISTEN, "2500")
        .env("SILICA_SHELL", "1")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    for line in lines.by_ref() {
        // The harness writes "test menu_shell_child ... " before the child's own output.
        if line.unwrap().ends_with("READY") {
            break;
        }
    }
    // "other" is in front, without menus; then "calc" comes forward with its model.
    let calc_id = window_id(&ossido, "calc");
    ossido
        .client()
        .call(WINDOW_FOCUS, json!({"id": calc_id}))
        .unwrap();
    std::thread::sleep(Duration::from_millis(400));
    // The shell chose the item: the app receives it, once (the old serial did nothing).
    let chosen = calc.menu_activations();
    // A live update reaches the bar; without a model the bar goes back to nothing.
    calc.set_menu_model(&model("Scientific"));
    std::thread::sleep(Duration::from_millis(300));
    calc.destroy_menu();
    let out: Vec<String> = lines.map(Result::unwrap).collect();
    child.wait().unwrap();
    assert_eq!(chosen, [CHOSEN], "{out:?}");
    let menus: Vec<&str> = out
        .iter()
        .filter_map(|l| l.strip_prefix("APP_MENU "))
        .collect();
    assert_eq!(
        menus,
        [
            "org.silica.Other none ",
            "org.silica.Calc some Basic",
            "org.silica.Calc some Scientific",
            "org.silica.Calc none ",
        ],
        "{out:?}"
    );
}

#[test]
fn the_menus_are_commands_too() {
    let ossido = Headless::start("app-menu-commands");
    let mut c = ossido.client();
    // No app in front: no menus, and nothing to choose.
    let state = c.call(APP_MENU, Value::Null).unwrap();
    assert_eq!(state["serial"], 0);
    assert!(c.call(APP_MENU_ACTIVATE, json!({"id": CHOSEN})).is_err());

    APP_ID.with(|id| *id.borrow_mut() = "org.silica.Calc".to_owned());
    let mut calc = TestWindow::open(&ossido.wayland, "calc");
    APP_ID.with(|id| id.borrow_mut().clear());
    calc.set_menu_model(&model("Basic"));
    let state = c.call(APP_MENU, Value::Null).unwrap();
    assert_eq!(state["app_id"], "org.silica.Calc");
    assert_ne!(state["serial"], 0);
    // In normal form: the defaults written out, the empty label of a role item dropped.
    assert_eq!(
        state["model"]["menus"][1]["items"][0],
        json!({"id": CHOSEN, "label": "Basic", "enabled": true, "checked": true,
               "shortcut": {"key": "1", "meta": true}})
    );
    assert_eq!(
        state["model"]["menus"][0]["items"][0],
        json!({"enabled": true, "role": "quit", "shortcut": {"key": "Q", "meta": true}})
    );
    c.call(APP_MENU_ACTIVATE, json!({"id": CHOSEN})).unwrap();
    assert_eq!(calc.menu_activations(), [CHOSEN]);
}

#[test]
fn a_bad_model_disconnects_only_its_app() {
    let ossido = Headless::start("app-menu-bad");
    let mut bad = TestWindow::open(&ossido.wayland, "bad");
    let mut good = TestWindow::open(&ossido.wayland, "good");
    good.set_menu_model(&model("Basic"));
    bad.set_menu_model("{\"menus\": [ not json");
    assert!(bad.disconnected());
    assert!(!good.disconnected());
    assert_eq!(good.menu_activations(), Vec::<u32>::new());
    // Ossido keeps going and still serves the good app's menus.
    let state = ossido.client().call(APP_MENU, Value::Null).unwrap();
    assert_ne!(state["serial"], 0, "{state}");
}
