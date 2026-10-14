//! Transient UI (#149): while a shell menu or panel is open the windows keep the pointer, and
//! a press outside ends it (`dismissed`); per transient the press stops there or goes on.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::io::{BufRead, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};

use common::Headless;
use common::client::{FIRST_WINDOW_AT, TestWindow};
use serde_json::{Value, json};
use silica_control::commands::{INPUT_CLICK, WINDOW_LIST, WindowInfo};
use silica_protocols::shell::client::silica_shell_manager_v1::{
    self, Outside, SilicaShellManagerV1,
};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{self, ZwlrLayerShellV1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, Anchor, ZwlrLayerSurfaceV1,
};

const CHILD_SOCKET: &str = "SILICA_TEST_TRANSIENT_SOCKET";
const CHILD_PRESS: &str = "SILICA_TEST_TRANSIENT_PRESS";

/// The menu: a small overlay in the bottom right corner, away from the windows.
const MENU: (u32, u32) = (60, 40);

#[derive(Default)]
struct App {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    layer_shell: Option<ZwlrLayerShellV1>,
    manager: Option<SilicaShellManagerV1>,
    size: Option<(u32, u32)>,
    dismissed: u32,
}

impl Dispatch<WlRegistry, ()> for App {
    fn event(
        app: &mut Self,
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
                "wl_compositor" => app.compositor = Some(registry.bind(name, 4, qh, ())),
                "wl_shm" => app.shm = Some(registry.bind(name, 1, qh, ())),
                "zwlr_layer_shell_v1" => app.layer_shell = Some(registry.bind(name, 1, qh, ())),
                "silica_shell_manager_v1" => app.manager = Some(registry.bind(name, 6, qh, ())),
                _ => {}
            }
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for App {
    fn event(
        app: &mut Self,
        surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            surface.ack_configure(serial);
            app.size = Some((width, height));
        }
    }
}

impl Dispatch<SilicaShellManagerV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &SilicaShellManagerV1,
        event: silica_shell_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let silica_shell_manager_v1::Event::Dismissed { .. } = event {
            app.dismissed += 1;
        }
    }
}

impl Dispatch<WlShm, ()> for App {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(App: ignore WlCompositor);
delegate_noop!(App: ignore WlSurface);
delegate_noop!(App: ignore WlBuffer);
delegate_noop!(App: WlShmPool);
delegate_noop!(App: ignore ZwlrLayerShellV1);

/// The shell component: opens the menu with its transient, prints READY, then listens for a
/// while and prints how many `dismissed` it received.
fn menu_client(socket: &str, press: Outside) {
    let conn = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let mut app = App::default();
    conn.display().get_registry(&qh, ());
    queue.roundtrip(&mut app).unwrap();
    let surface = app.compositor.as_ref().unwrap().create_surface(&qh, ());
    let layer = app.layer_shell.as_ref().unwrap().get_layer_surface(
        &surface,
        None,
        zwlr_layer_shell_v1::Layer::Overlay,
        "menu".to_owned(),
        &qh,
        (),
    );
    layer.set_anchor(Anchor::Bottom | Anchor::Right);
    layer.set_size(MENU.0, MENU.1);
    surface.commit();
    while app.size.is_none() {
        queue.blocking_dispatch(&mut app).unwrap();
    }
    let (w, h) = app.size.unwrap();
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&vec![255u8; (w * h * 4) as usize]).unwrap();
    let pool = app
        .shm
        .as_ref()
        .unwrap()
        .create_pool(file.as_fd(), (w * h * 4) as i32, &qh, ());
    let buffer = pool.create_buffer(
        0,
        w as i32,
        h as i32,
        (w * 4) as i32,
        wl_shm::Format::Argb8888,
        &qh,
        (),
    );
    surface.attach(Some(&buffer), 0, 0);
    surface.commit();
    let manager = app
        .manager
        .clone()
        .expect("silica_shell_manager_v1 version 6");
    manager.set_transient(&surface, None, press, Outside::PassThrough);
    queue.roundtrip(&mut app).unwrap();
    println!("READY");
    std::io::stdout().flush().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2500);
    while std::time::Instant::now() < deadline {
        queue.roundtrip(&mut app).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    println!("DISMISSED {}", app.dismissed);
}

/// The child process: runs only when the parent test launches it.
#[test]
fn transient_child() {
    if let Ok(socket) = std::env::var(CHILD_SOCKET) {
        let press = match std::env::var(CHILD_PRESS).as_deref() {
            Ok("pass_through") => Outside::PassThrough,
            _ => Outside::Consume,
        };
        menu_client(&socket, press);
    }
}

/// On the first window, outside the menu.
fn on_first() -> (f64, f64) {
    let at = FIRST_WINDOW_AT as f64;
    (at + 5.0, at + 30.0)
}

/// Opens two windows ("second" on top, focused), the menu with this policy, clicks at `at`;
/// returns the child's output and whether "first" got the focus.
fn click_at(name: &str, press: &str, at: (f64, f64)) -> (String, bool) {
    let ossido = Headless::start(name);
    let _first = TestWindow::open(&ossido.wayland, "first");
    let _second = TestWindow::open(&ossido.wayland, "second");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "transient_child",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_SOCKET, &ossido.wayland)
        .env(CHILD_PRESS, press)
        .env("SILICA_SHELL", "1")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = std::io::BufReader::new(stdout).lines();
    let mut out = String::new();
    for line in lines.by_ref() {
        let line = line.unwrap();
        out.push_str(&line);
        out.push('\n');
        // libtest prints "test transient_child ... " on the same line.
        if line.ends_with("READY") {
            break;
        }
    }
    ossido
        .client()
        .call(INPUT_CLICK, json!({"x": at.0, "y": at.1}))
        .unwrap();
    let list: Vec<WindowInfo> =
        serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap();
    let first_focused = list
        .iter()
        .any(|w| w.title.as_deref() == Some("first") && w.focused);
    for line in lines {
        out.push_str(&line.unwrap());
        out.push('\n');
    }
    child.wait().unwrap();
    (out, first_focused)
}

#[test]
fn a_press_outside_a_menu_closes_it_and_stops_there() {
    let (out, first_focused) = click_at("transient-consume", "consume", on_first());
    assert!(out.contains("DISMISSED 1"), "{out}");
    assert!(!first_focused, "the press reached the window: {out}");
}

#[test]
fn a_press_outside_a_panel_closes_it_and_goes_through() {
    let (out, first_focused) = click_at("transient-pass", "pass_through", on_first());
    assert!(out.contains("DISMISSED 1"), "{out}");
    assert!(first_focused, "the press did not reach the window: {out}");
}

#[test]
fn a_press_on_the_menu_itself_keeps_it_open() {
    let x = f64::from(common::WIDTH - MENU.0 / 2);
    let y = f64::from(common::HEIGHT - MENU.1 / 2);
    let (out, _) = click_at("transient-inside", "consume", (x, y));
    assert!(out.contains("DISMISSED 0"), "{out}");
}
