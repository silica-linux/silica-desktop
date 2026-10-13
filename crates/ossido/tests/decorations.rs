//! Decorazioni lato server: barra del titolo, pulsanti della finestra e loro comandi,
//! con i colori misurati su macOS 27 (`plan/reference/windows.md`).

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
use silica_control::commands::{
    INPUT_CLICK, INPUT_POINTER_BUTTON, INPUT_POINTER_MOVE, WINDOW_FOCUS, WINDOW_FULLSCREEN,
    WINDOW_LIST, WindowInfo,
};
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::Mode;

const SIZE: (u32, u32) = (640, 400);
const BAR: i32 = 52;

fn start(name: &str) -> Headless {
    Headless::start_with(name, SIZE, &["--background", "7a7b80"])
}

fn windows(ossido: &Headless) -> Vec<WindowInfo> {
    serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap()
}

fn shot(ossido: &Headless) -> Vec<u8> {
    let shot = common::screenshot(ossido);
    if let Some(dir) = std::env::var_os("OSSIDO_SHOTS") {
        std::fs::copy(
            ossido.dir.join("shot.png"),
            std::path::Path::new(&dir).join("decorations.png"),
        )
        .unwrap();
    }
    shot
}

/// Il pixel è entro `tolerance` dal colore atteso, canale per canale.
fn assert_near(shot: &[u8], (x, y): (i32, i32), expected: [u8; 3], tolerance: u8, what: &str) {
    let got = pixel_in(shot, SIZE.0, x as u32, y as u32);
    let close = got
        .iter()
        .zip(expected)
        .all(|(g, e)| g.abs_diff(e) <= tolerance);
    assert!(close, "{what} in ({x}, {y}): {got:?}, atteso {expected:?}");
}

/// Centro di un pulsante della barra (0 chiudi, 1 riduci, 2 schermo intero) della finestra.
fn button(window: &WindowInfo, index: i32) -> (i32, i32) {
    (window.x + 26 + 23 * index, window.y - BAR + 26)
}

#[test]
fn titlebar_is_negotiated_and_drawn_with_the_measured_buttons() {
    let ossido = start("decor-draw");
    let mut window = TestWindow::open_decorated(&ossido.wayland, "barra", (240, 140));
    assert_eq!(window.decoration_mode(), Some(Mode::ServerSide));
    let info = windows(&ossido).remove(0);
    // La barra sta sopra il contenuto, dentro l'area utile.
    assert!(
        info.y >= BAR,
        "barra fuori dallo schermo: contenuto a y {}",
        info.y
    );
    let shot = shot(&ossido);

    // Fondo bianco della barra attiva; vertice superiore sinistro tagliato (fondo grigio).
    assert_near(
        &shot,
        (info.x + 150, info.y - 26),
        [255, 255, 255],
        1,
        "fondo della barra",
    );
    assert_near(
        &shot,
        (info.x, info.y - BAR),
        [0x7a, 0x7b, 0x80],
        40,
        "angolo della barra",
    );
    // Pulsanti attivi: chiusura #f87d77, riduzione #fcc93b, schermo intero #85d660 al centro.
    assert_near(&shot, button(&info, 0), [0xf8, 0x7d, 0x77], 8, "chiusura");
    assert_near(&shot, button(&info, 1), [0xfc, 0xc9, 0x3b], 8, "riduzione");
    assert_near(
        &shot,
        button(&info, 2),
        [0x85, 0xd6, 0x60],
        8,
        "schermo intero",
    );
    // Il contenuto resta rosso sotto la barra.
    assert_near(
        &shot,
        (info.x + 120, info.y + 70),
        [255, 0, 0],
        0,
        "contenuto",
    );
}

#[test]
fn hovering_the_buttons_shows_the_symbols_and_inactive_buttons_are_gray() {
    let ossido = start("decor-hover");
    let _first = TestWindow::open_decorated(&ossido.wayland, "prima", (240, 140));
    let first = windows(&ossido).remove(0);
    let mut client = ossido.client();
    let (cx, cy) = button(&first, 0);
    client
        .call(INPUT_POINTER_MOVE, json!({"x": cx, "y": cy}))
        .unwrap();
    // Al centro della chiusura c'è l'incrocio della X, `#a30000`.
    assert_near(
        &shot(&ossido),
        (cx, cy),
        [0xa3, 0x00, 0x00],
        12,
        "simbolo di chiusura",
    );

    // Una seconda finestra lontana rende inattiva la prima: pulsanti grigi `#eaebec`.
    let _second = TestWindow::open_decorated(&ossido.wayland, "seconda", (120, 80));
    let second = windows(&ossido).remove(0);
    client
        .call(
            silica_control::commands::WINDOW_MOVE,
            json!({"id": second.id, "x": 450, "y": 250}),
        )
        .unwrap();
    client
        .call(INPUT_POINTER_MOVE, json!({"x": 5, "y": 395}))
        .unwrap();
    let inactive = shot(&ossido);
    for index in 0..3 {
        assert_near(
            &inactive,
            button(&first, index),
            [0xea, 0xeb, 0xec],
            6,
            "pulsante inattivo",
        );
    }

    // Puntatore sul gruppo della finestra inattiva: colori attivi e simboli, resta inattiva.
    let (mx, my) = button(&first, 1);
    client
        .call(INPUT_POINTER_MOVE, json!({"x": mx, "y": my}))
        .unwrap();
    assert_near(
        &shot(&ossido),
        (mx, my),
        [0xc1, 0x44, 0x00],
        14,
        "simbolo di riduzione",
    );
    // Premuto sulla riduzione: il simbolo diventa più chiaro e saturo (`#ff5a00`).
    client
        .call(INPUT_POINTER_BUTTON, json!({"pressed": true}))
        .unwrap();
    assert_near(
        &shot(&ossido),
        (mx, my),
        [0xff, 0x5a, 0x00],
        14,
        "riduzione premuta",
    );
    // Uscendo prima del rilascio l'azione si annulla.
    client
        .call(INPUT_POINTER_MOVE, json!({"x": mx + 60, "y": my + 60}))
        .unwrap();
    client
        .call(INPUT_POINTER_BUTTON, json!({"pressed": false}))
        .unwrap();
    assert!(
        !windows(&ossido)
            .iter()
            .find(|w| w.id == first.id)
            .unwrap()
            .minimized
    );
}

#[test]
fn titlebar_buttons_close_minimize_and_fullscreen() {
    let ossido = start("decor-buttons");
    let mut window = TestWindow::open_decorated(&ossido.wayland, "pulsanti", (240, 140));
    let info = windows(&ossido).remove(0);
    let mut client = ossido.client();
    let click = |client: &mut silica_control::Client, (x, y): (i32, i32)| {
        client.call(INPUT_CLICK, json!({"x": x, "y": y})).unwrap();
    };

    // Riduci: sparisce dallo schermo, window.focus la riporta dov'era.
    click(&mut client, button(&info, 1));
    let list = windows(&ossido);
    assert!(list[0].minimized, "{list:?}");
    client.call(WINDOW_FOCUS, json!({"id": info.id})).unwrap();
    let list = windows(&ossido);
    assert!(!list[0].minimized && list[0].focused, "{list:?}");
    assert_eq!((list[0].x, list[0].y), (info.x, info.y));

    // Schermo intero e ritorno (pulsante verde, poi il comando).
    // Lo stato passa al client con un configure; vale dal commit successivo.
    click(&mut client, button(&info, 2));
    window.decoration_mode();
    window.commit_frames(1, std::time::Duration::ZERO);
    let list = windows(&ossido);
    assert!(list[0].fullscreen, "{list:?}");
    client.call(WINDOW_FULLSCREEN, Value::Null).unwrap();
    window.decoration_mode();
    window.commit_frames(1, std::time::Duration::ZERO);
    let list = windows(&ossido);
    assert!(!list[0].fullscreen, "{list:?}");
    assert_eq!((list[0].x, list[0].y), (info.x, info.y));

    // Chiudi: la richiesta arriva al client.
    click(&mut client, button(&info, 0));
    assert!(window.close_requested());
}

#[test]
fn dragging_the_titlebar_moves_the_window() {
    let ossido = start("decor-drag");
    let _window = TestWindow::open_decorated(&ossido.wayland, "trascina", (240, 140));
    let info = windows(&ossido).remove(0);
    let mut client = ossido.client();
    let (x, y) = (info.x + 150, info.y - 26);
    client
        .call(INPUT_POINTER_MOVE, json!({"x": x, "y": y}))
        .unwrap();
    client
        .call(INPUT_POINTER_BUTTON, json!({"pressed": true}))
        .unwrap();
    client
        .call(INPUT_POINTER_MOVE, json!({"x": x + 40, "y": y + 30}))
        .unwrap();
    client
        .call(INPUT_POINTER_BUTTON, json!({"pressed": false}))
        .unwrap();
    let moved = windows(&ossido).remove(0);
    assert_eq!((moved.x, moved.y), (info.x + 40, info.y + 30));
}
