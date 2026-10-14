//! The settings files in `$XDG_CONFIG_HOME/silica`: Ossido applies them at start and when
//! they change (inotify).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use common::Headless;
use serde_json::{Value, json};
use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};

/// Writes `name` like Settings does: a temporary file renamed over the real one.
fn write_setting(ossido: &Headless, name: &str, value: &Value) {
    let dir = ossido.dir.join("config/silica");
    std::fs::create_dir_all(&dir).unwrap();
    let temporary = dir.join(format!("{name}.tmp"));
    std::fs::write(&temporary, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    std::fs::rename(&temporary, dir.join(name)).unwrap();
}

/// Calls `command` until `check` accepts the result (5 s at most).
fn wait_for(ossido: &Headless, command: &str, check: impl Fn(&Value) -> bool) -> Value {
    let mut client = ossido.client();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let value = client.call(command, Value::Null).unwrap();
        if check(&value) {
            return value;
        }
        assert!(Instant::now() < deadline, "{command}: still {value}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[derive(Default)]
struct Keyboard {
    repeat: Vec<(i32, i32)>,
    keymaps: usize,
}

impl Dispatch<WlRegistry, ()> for Keyboard {
    fn event(
        _: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
            && interface == "wl_seat"
        {
            let seat: WlSeat = registry.bind(name, 7, qh, ());
            seat.get_keyboard(qh, ());
        }
    }
}

impl Dispatch<WlSeat, ()> for Keyboard {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: wayland_client::protocol::wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlKeyboard, ()> for Keyboard {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::RepeatInfo { rate, delay } => state.repeat.push((rate, delay)),
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                ..
            } => state.keymaps += 1,
            _ => {}
        }
    }
}

fn connect(socket: &Path) -> (Connection, wayland_client::EventQueue<Keyboard>, Keyboard) {
    let connection = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    connection.display().get_registry(&queue.handle(), ());
    let mut keyboard = Keyboard::default();
    queue.roundtrip(&mut keyboard).unwrap();
    queue.roundtrip(&mut keyboard).unwrap();
    (connection, queue, keyboard)
}

#[test]
fn keyboard_json_sets_repeat_and_sources_live() {
    let ossido = Headless::start("keyboard-json");
    let (_connection, mut queue, mut keyboard) = connect(&ossido.wayland);
    // The defaults first (no file).
    let state = ossido.client().call("keyboard.state", Value::Null).unwrap();
    assert_eq!(state["repeatDelay"], 375);
    assert_eq!(state["repeatRate"], 11);
    assert_eq!(keyboard.repeat, [(11, 375)]);
    assert_eq!(keyboard.keymaps, 1);
    // With one source the shortcut does not switch.
    let next = ossido
        .client()
        .call("keyboard.next_source", Value::Null)
        .unwrap();
    assert_eq!(next["switched"], false);

    write_setting(
        &ossido,
        "keyboard.json",
        &json!({"repeatDelay": 250, "repeatRate": 30, "layouts": ["us", "it"]}),
    );
    let state = wait_for(&ossido, "keyboard.state", |s| s["repeatRate"] == 30);
    assert_eq!(state["repeatDelay"], 250);
    assert_eq!(state["layouts"], json!(["us", "it"]));
    assert_eq!(state["active"], 0);
    // The bound keyboard got the new repeat and a new keymap.
    queue.roundtrip(&mut keyboard).unwrap();
    assert_eq!(keyboard.repeat.last(), Some(&(30, 250)));
    assert_eq!(keyboard.keymaps, 2);

    let next = ossido
        .client()
        .call("keyboard.next_source", Value::Null)
        .unwrap();
    assert_eq!(next, json!({"switched": true, "active": 1}));

    // A layout xkb does not know keeps the keymap; the repeat still applies.
    write_setting(
        &ossido,
        "keyboard.json",
        &json!({"repeatDelay": 600, "repeatRate": 0, "layouts": ["us", "nosuchlayout"]}),
    );
    let state = wait_for(&ossido, "keyboard.state", |s| s["repeatDelay"] == 600);
    assert_eq!(state["repeatRate"], 0);
    assert_eq!(state["layouts"], json!(["us", "it"]));

    // Removed: the defaults again.
    std::fs::remove_file(ossido.dir.join("config/silica/keyboard.json")).unwrap();
    let state = wait_for(&ossido, "keyboard.state", |s| s["repeatDelay"] == 375);
    assert_eq!(state["repeatRate"], 11);
    assert_eq!(state["layouts"], json!([]));
}
