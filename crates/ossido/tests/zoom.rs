//! The app zooms or minimizes its own window (`xdg_toplevel.set_maximized`, `unset_maximized`,
//! `set_minimized`): the title bar double click of an app that draws its bar (#169).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::Headless;
use common::client::TestWindow;
use serde_json::Value;
use silica_control::commands::{WINDOW_LIST, WindowInfo};

const SIZE: (u32, u32) = (1280, 800);

fn windows(ossido: &Headless) -> Vec<WindowInfo> {
    serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap()
}

#[test]
fn maximize_fills_the_screen_and_unmaximize_goes_back() {
    let ossido = Headless::start_with("zoom-toggle", SIZE, &[]);
    let mut window = TestWindow::open_decorated(&ossido.wayland, "zoom", (300, 200));
    let before = windows(&ossido).remove(0);
    window.request_maximized(true);
    assert!(window.maximized(), "the client is told it is maximized");
    let (w, h) = window.configured_size().unwrap();
    assert!(w > 1000 && h > 600, "the content fills the area: {w}x{h}");
    window.request_maximized(false);
    assert!(!window.maximized());
    let (w, h) = window.configured_size().unwrap();
    assert_eq!(w, 300);
    assert_eq!(h, 200);
    let after = windows(&ossido).remove(0);
    assert_eq!((after.x, after.y), (before.x, before.y));
}

#[test]
fn minimize_request_takes_the_window_off_the_screen() {
    let ossido = Headless::start_with("zoom-minimize", SIZE, &[]);
    let mut window = TestWindow::open_decorated(&ossido.wayland, "min", (300, 200));
    assert_eq!(windows(&ossido).len(), 1);
    window.request_minimized();
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(windows(&ossido).iter().all(|w| w.minimized));
}
