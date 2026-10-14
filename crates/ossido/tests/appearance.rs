//! Aspetto scuro delle finestre (`--appearance dark`): barra del titolo, pulsanti, bordo
//! interno di 1 pt e ombra, con i colori misurati su macOS 27 (`plan/reference/windows.md`).

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
const BAR: i32 = 52;
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

/// Il pixel è entro `tolerance` dal colore atteso, canale per canale.
fn assert_near(shot: &[u8], at: (i32, i32), expected: [u8; 3], tolerance: u8, what: &str) {
    let got = rgb(shot, at);
    let close = got
        .iter()
        .zip(expected)
        .all(|(g, e)| g.abs_diff(e) <= tolerance);
    assert!(close, "{what} in {at:?}: {got:?}, atteso {expected:?}");
}

/// Centro di un pulsante della barra (0 chiudi, 1 riduci, 2 schermo intero).
fn button(window: &WindowInfo, index: i32) -> (i32, i32) {
    (window.x + 26 + 23 * index, window.y - BAR + 26)
}

#[test]
fn dark_windows_use_the_measured_bar_buttons_and_inner_border() {
    let ossido = start("dark-active", "dark");
    let _window = TestWindow::open_decorated(&ossido.wayland, "scura", (240, 140));
    let info = windows(&ossido).remove(0);
    let shot = shot(&ossido, "appearance-dark");

    // Barra attiva: come il contenuto in scuro, `#272728`.
    assert_near(&shot, (info.x + 150, info.y - 26), [39, 39, 40], 1, "fondo");
    // Pulsanti attivi scuri al centro: `#f26056`, `#fdc002`, `#39c101`.
    assert_near(&shot, button(&info, 0), [0xf2, 0x60, 0x56], 8, "chiusura");
    assert_near(&shot, button(&info, 1), [0xfd, 0xc0, 0x02], 8, "riduzione");
    assert_near(
        &shot,
        button(&info, 2),
        [0x39, 0xc1, 0x01],
        8,
        "schermo intero",
    );
    // Bordo interno di 1 pt `#3d3d3d`: sopra la barra, a sinistra e in fondo sul contenuto.
    assert_near(
        &shot,
        (info.x + 150, info.y - BAR),
        [0x3d; 3],
        2,
        "bordo sopra",
    );
    assert_near(
        &shot,
        (info.x, info.y + 70),
        [0x3d; 3],
        2,
        "bordo a sinistra",
    );
    assert_near(
        &shot,
        (info.x + 120, info.y + 139),
        [0x3d; 3],
        2,
        "bordo sotto",
    );
    // Subito dentro il contenuto resta quello del client.
    assert_near(
        &shot,
        (info.x + 1, info.y + 70),
        [255, 0, 0],
        0,
        "contenuto",
    );
}

#[test]
fn inactive_dark_windows_have_their_own_bar_and_gray_buttons() {
    let ossido = start("dark-inactive", "dark");
    let _first = TestWindow::open_decorated(&ossido.wayland, "prima", (240, 140));
    let first = windows(&ossido).remove(0);
    let _second = TestWindow::open_decorated(&ossido.wayland, "seconda", (120, 80));
    let second = windows(&ossido).remove(0);
    let mut client = ossido.client();
    client
        .call(WINDOW_MOVE, json!({"id": second.id, "x": 450, "y": 250}))
        .unwrap();
    client
        .call(INPUT_POINTER_MOVE, json!({"x": 5, "y": 395}))
        .unwrap();
    let shot = shot(&ossido, "appearance-dark-inactive");

    // Barra inattiva `#2f2f30`, separatore di 1 pt `#363636` sopra il contenuto.
    assert_near(
        &shot,
        (first.x + 150, first.y - 26),
        [47, 47, 48],
        1,
        "fondo",
    );
    assert_near(
        &shot,
        (first.x + 150, first.y - 1),
        [54, 54, 54],
        1,
        "separatore",
    );
    // Pulsanti inattivi: centro `#4d4d4d`.
    for index in 0..3 {
        assert_near(
            &shot,
            button(&first, index),
            [0x4d; 3],
            4,
            "pulsante inattivo",
        );
    }
    // Bordo inattivo: `#404041` sopra, `#3a3a3b` ai lati.
    assert_near(
        &shot,
        (first.x + 150, first.y - BAR),
        [0x40, 0x40, 0x41],
        2,
        "bordo sopra",
    );
    assert_near(
        &shot,
        (first.x, first.y + 70),
        [0x3a, 0x3a, 0x3b],
        2,
        "bordo a sinistra",
    );
}

#[test]
fn the_dark_shadow_is_stronger_than_the_light_one() {
    let mut below = Vec::new();
    for appearance in ["light", "dark"] {
        let ossido = start(&format!("shadow-{appearance}"), appearance);
        let _window = TestWindow::open_decorated(&ossido.wayland, "ombra", (240, 140));
        let info = windows(&ossido).remove(0);
        let shot = shot(&ossido, &format!("appearance-shadow-{appearance}"));
        // Sotto la finestra (ombra spostata in basso) e a lato, sul grigio.
        let under = rgb(&shot, (info.x + 120, info.y + 140 + 8))[1];
        let side = rgb(&shot, (info.x - 6, info.y + 70))[1];
        // L'anello di contatto: in scuro quasi un bordo nero (alfa 0,95 contro 0,32).
        let ring = rgb(&shot, (info.x - 1, info.y + 70))[1];
        below.push((under, side, ring));
    }
    let (light, dark) = (below[0], below[1]);
    assert!(
        dark.0 + 10 < light.0,
        "sotto: chiaro {light:?}, scuro {dark:?}"
    );
    assert!(
        dark.1 + 5 < light.1,
        "a lato: chiaro {light:?}, scuro {dark:?}"
    );
    assert!(
        dark.2 < 20 && light.2 > 60,
        "anello: chiaro {light:?}, scuro {dark:?}"
    );
}

#[test]
fn appearance_flag_is_validated() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ossido"))
        .args(["--appearance", "seppia"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
