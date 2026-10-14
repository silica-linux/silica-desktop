//! Dark appearance of windows (`--appearance dark`): title bar, buttons, 1 pt inner border
//! and shadow, with the colors measured on macOS 27 (`plan/reference/windows.md`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::client::TestWindow;
use common::{Headless, pixel_in};
use serde_json::{Value, json};
use silica_control::commands::{INPUT_POINTER_MOVE, WINDOW_LIST, WINDOW_MOVE, WindowInfo};

const SIZE: (u32, u32) = (640, 400);
const BAR: i32 = 32;
const GRAY: &str = "7a7b80";

fn start(name: &str, appearance: &str) -> Headless {
    Headless::start_with(
        name,
        SIZE,
        &["--background", GRAY, "--appearance", appearance],
    )
}

fn windows(ossido: &Headless) -> Vec<WindowInfo> {
    serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap()
}

fn shot(ossido: &Headless, name: &str) -> Vec<u8> {
    let shot = common::screenshot(ossido);
    if let Some(dir) = std::env::var_os("OSSIDO_SHOTS") {
        std::fs::copy(
            ossido.dir.join("shot.png"),
            std::path::Path::new(&dir).join(format!("{name}.png")),
        )
        .unwrap();
    }
    shot
}

fn rgb(shot: &[u8], (x, y): (i32, i32)) -> [u8; 3] {
    let p = pixel_in(shot, SIZE.0, x as u32, y as u32);
    [p[0], p[1], p[2]]
}

/// The pixel is within `tolerance` of the expected color, channel by channel.
fn assert_near(shot: &[u8], at: (i32, i32), expected: [u8; 3], tolerance: u8, what: &str) {
    let got = rgb(shot, at);
    let close = got
        .iter()
        .zip(expected)
        .all(|(g, e)| g.abs_diff(e) <= tolerance);
    assert!(close, "{what} at {at:?}: {got:?}, expected {expected:?}");
}

/// Center of a bar button (0 close, 1 minimize, 2 full screen).
fn button(window: &WindowInfo, index: i32) -> (i32, i32) {
    (window.x + 16 + 23 * index, window.y - BAR + 16)
}

#[test]
fn dark_windows_use_the_measured_bar_buttons_and_inner_border() {
    let ossido = start("dark-active", "dark");
    let _window = TestWindow::open_decorated(&ossido.wayland, "dark", (240, 140));
    let info = windows(&ossido).remove(0);
    let shot = shot(&ossido, "appearance-dark");

    // Active bar: like the content in dark, `#272728`.
    assert_near(
        &shot,
        (info.x + 150, info.y - 16),
        [39, 39, 40],
        1,
        "background",
    );
    // Dark active buttons at the center: `#f26056`, `#fdc002`, `#39c101`.
    assert_near(&shot, button(&info, 0), [0xf2, 0x60, 0x56], 8, "close");
    assert_near(&shot, button(&info, 1), [0xfd, 0xc0, 0x02], 8, "minimize");
    assert_near(
        &shot,
        button(&info, 2),
        [0x39, 0xc1, 0x01],
        8,
        "full screen",
    );
    // 1 pt inner border `#3d3d3d`: above the bar, on the left and at the bottom on the content.
    assert_near(
        &shot,
        (info.x + 150, info.y - BAR),
        [0x3d; 3],
        2,
        "top border",
    );
    assert_near(&shot, (info.x, info.y + 70), [0x3d; 3], 2, "left border");
    assert_near(
        &shot,
        (info.x + 120, info.y + 139),
        [0x3d; 3],
        2,
        "bottom border",
    );
    // Right inside the content the client's color stays.
    assert_near(&shot, (info.x + 1, info.y + 70), [255, 0, 0], 0, "content");
}

#[test]
fn inactive_dark_windows_have_their_own_bar_and_gray_buttons() {
    let ossido = start("dark-inactive", "dark");
    let _first = TestWindow::open_decorated(&ossido.wayland, "first", (240, 140));
    let first = windows(&ossido).remove(0);
    let _second = TestWindow::open_decorated(&ossido.wayland, "second", (120, 80));
    let second = windows(&ossido).remove(0);
    let mut client = ossido.client();
    client
        .call(WINDOW_MOVE, json!({"id": second.id, "x": 450, "y": 250}))
        .unwrap();
    client
        .call(INPUT_POINTER_MOVE, json!({"x": 5, "y": 395}))
        .unwrap();
    let shot = shot(&ossido, "appearance-dark-inactive");

    // Inactive bar `#2f2f30`, 1 pt separator `#363636` above the content.
    assert_near(
        &shot,
        (first.x + 150, first.y - 16),
        [47, 47, 48],
        1,
        "background",
    );
    assert_near(
        &shot,
        (first.x + 150, first.y - 1),
        [54, 54, 54],
        1,
        "separator",
    );
    // Inactive buttons: center `#4d4d4d`.
    for index in 0..3 {
        assert_near(
            &shot,
            button(&first, index),
            [0x4d; 3],
            4,
            "inactive button",
        );
    }
    // Inactive border: `#404041` on top, `#3a3a3b` on the sides.
    assert_near(
        &shot,
        (first.x + 150, first.y - BAR),
        [0x40, 0x40, 0x41],
        2,
        "top border",
    );
    assert_near(
        &shot,
        (first.x, first.y + 70),
        [0x3a, 0x3a, 0x3b],
        2,
        "left border",
    );
}

#[test]
fn the_dark_shadow_is_stronger_than_the_light_one() {
    let mut below = Vec::new();
    for appearance in ["light", "dark"] {
        let ossido = start(&format!("shadow-{appearance}"), appearance);
        let _window = TestWindow::open_decorated(&ossido.wayland, "shadow", (240, 140));
        let info = windows(&ossido).remove(0);
        let shot = shot(&ossido, &format!("appearance-shadow-{appearance}"));
        // Below the window (shadow shifted down) and on the side, over gray.
        let under = rgb(&shot, (info.x + 120, info.y + 140 + 8))[1];
        let side = rgb(&shot, (info.x - 6, info.y + 70))[1];
        // The contact ring: in dark almost a black border (alpha 0.95 against 0.32).
        let ring = rgb(&shot, (info.x - 1, info.y + 70))[1];
        below.push((under, side, ring));
    }
    let (light, dark) = (below[0], below[1]);
    assert!(
        dark.0 + 10 < light.0,
        "below: light {light:?}, dark {dark:?}"
    );
    assert!(
        dark.1 + 5 < light.1,
        "beside: light {light:?}, dark {dark:?}"
    );
    assert!(
        dark.2 < 20 && light.2 > 60,
        "ring: light {light:?}, dark {dark:?}"
    );
}

#[test]
fn appearance_flag_is_validated() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ossido"))
        .args(["--appearance", "sepia"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
