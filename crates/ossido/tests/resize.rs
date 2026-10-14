//! Ridimensionamento dai bordi della cornice (mustache-spec 1.1, MU-08): 2 pt fuori e 3 dentro
//! a sinistra e in alto, 4 fuori e 3 dentro a destra e in basso, angoli diagonali per 16 pt.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::Headless;
use common::client::TestWindow;
use serde_json::{Value, json};
use silica_control::commands::{INPUT_POINTER_BUTTON, INPUT_POINTER_MOVE, WINDOW_LIST, WindowInfo};

const SIZE: (u32, u32) = (1280, 800);
const BAR: i32 = 52;
const W: i32 = 300;
const H: i32 = 200;

fn window(ossido: &Headless) -> WindowInfo {
    let list: Vec<WindowInfo> =
        serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap();
    list.into_iter().next().unwrap()
}

/// Preme in `from`, trascina di `by` e rilascia.
fn drag(ossido: &Headless, from: (f64, f64), by: (f64, f64)) {
    let mut client = ossido.client();
    client
        .call(INPUT_POINTER_MOVE, json!({"x": from.0, "y": from.1}))
        .unwrap();
    client
        .call(
            INPUT_POINTER_BUTTON,
            json!({"button": "left", "pressed": true}),
        )
        .unwrap();
    client
        .call(
            INPUT_POINTER_MOVE,
            json!({"x": from.0 + by.0, "y": from.1 + by.1}),
        )
        .unwrap();
    client
        .call(
            INPUT_POINTER_BUTTON,
            json!({"button": "left", "pressed": false}),
        )
        .unwrap();
}

/// Una finestra decorata 300 × 200 e la sua cornice (barra compresa): x, y, larghezza, altezza.
fn open(name: &str) -> (Headless, TestWindow, (f64, f64, f64, f64)) {
    let ossido = Headless::start_with(name, SIZE, &["--background", "7a7b80"]);
    let mut win = TestWindow::open_decorated(&ossido.wayland, "bordi", (W as u32, H as u32));
    win.configured_size();
    let info = window(&ossido);
    let frame = (
        f64::from(info.x),
        f64::from(info.y - BAR),
        f64::from(W),
        f64::from(H + BAR),
    );
    (ossido, win, frame)
}

#[test]
fn il_bordo_sinistro_prende_anche_2_pt_fuori() {
    let (ossido, mut win, (x, y, _, _)) = open("resize-left");
    let before = window(&ossido);
    drag(&ossido, (x - 1.5, y + 120.0), (-50.0, 0.0));
    assert_eq!(win.configured_size(), Some((W + 50, H)));
    assert_eq!(
        window(&ossido).x,
        before.x - 50,
        "il lato destro resta fermo"
    );
}

#[test]
fn il_bordo_destro_prende_4_pt_fuori_e_non_oltre() {
    let (ossido, mut win, (x, y, w, _)) = open("resize-right");
    let before = window(&ossido);
    drag(&ossido, (x + w + 3.5, y + 120.0), (40.0, 0.0));
    assert_eq!(win.configured_size(), Some((W + 40, H)));
    assert_eq!(window(&ossido).x, before.x);
    // Appena oltre la zona (4 pt fuori): niente.
    let (ossido, mut win, (x, y, w, _)) = open("resize-right-out");
    let asked = win.configured_size();
    drag(&ossido, (x + w + 4.5, y + 120.0), (40.0, 0.0));
    assert_eq!(win.configured_size(), asked);
}

#[test]
fn l_angolo_ridimensiona_in_due_direzioni() {
    let (ossido, mut win, (x, y, w, h)) = open("resize-corner");
    // 1 pt fuori a destra, 5 pt sopra il fondo: dentro i 16 pt dell'angolo.
    drag(&ossido, (x + w + 1.0, y + h - 5.0), (30.0, 20.0));
    assert_eq!(win.configured_size(), Some((W + 30, H + 20)));
}

#[test]
fn a_tutta_finestra_il_bordo_alto_e_la_cima_dell_app() {
    let ossido = Headless::start_with("resize-full", SIZE, &["--background", "7a7b80"]);
    let mut win = TestWindow::open_decorated(&ossido.wayland, "bordi", (W as u32, H as u32));
    win.set_full_size_content(52);
    let before = window(&ossido);
    // La cornice è il contenuto: la cima è a y della finestra.
    let (x, y) = (f64::from(before.x), f64::from(before.y));
    drag(&ossido, (x + 150.0, y - 1.0), (0.0, -30.0));
    assert_eq!(win.configured_size(), Some((W, H + 30)));
    assert_eq!(window(&ossido).y, before.y - 30);
}
