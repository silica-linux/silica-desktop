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

#[test]
fn full_size_content_keeps_only_the_buttons_over_the_app() {
    let ossido = start("decor-full-size");
    let mut window = TestWindow::open_decorated(&ossido.wayland, "tutta", (240, 140));
    let before = windows(&ossido).remove(0);
    // L'app disegna una barra degli strumenti alta 52: Ossido toglie la sua barra.
    window.set_full_size_content(52);
    window.commit_frames(1, std::time::Duration::ZERO);
    let info = windows(&ossido).remove(0);
    let image = shot(&ossido);
    // Sopra il contenuto non c'è più la barra bianca: il fondo grigio.
    assert_near(
        &image,
        (info.x + 150, info.y - 26),
        [0x7a, 0x7b, 0x80],
        6,
        "niente barra sopra il contenuto",
    );
    // I pulsanti stanno sopra il contenuto, centrati nella barra dell'app (y 26).
    let on_content = |i: i32| (info.x + 26 + 23 * i, info.y + 26);
    assert_near(&image, on_content(0), [0xf8, 0x7d, 0x77], 8, "chiusura");
    assert_near(
        &image,
        on_content(2),
        [0x85, 0xd6, 0x60],
        8,
        "schermo intero",
    );
    // Il resto della fascia è dell'app (rosso).
    assert_near(
        &image,
        (info.x + 150, info.y + 26),
        [255, 0, 0],
        0,
        "barra dell'app",
    );
    // Il clic sul rosso chiude; altrove arriva all'app.
    let mut client = ossido.client();
    client
        .call(INPUT_CLICK, json!({"x": info.x + 26, "y": info.y + 26}))
        .unwrap();
    assert!(window.close_requested());
    // 0: di nuovo la barra di Ossido.
    window.set_full_size_content(0);
    window.commit_frames(1, std::time::Duration::ZERO);
    let image = shot(&ossido);
    let info = windows(&ossido).remove(0);
    assert_near(
        &image,
        (info.x + 150, info.y - 26),
        [255, 255, 255],
        1,
        "la barra torna",
    );
    let _ = before;
}

/// L'app sposta la finestra dalla sua barra degli strumenti (`xdg_toplevel.move`): solo con il
/// pulsante ancora premuto, e il rilascio la lascia dove è.
#[test]
fn the_app_moves_its_window_from_the_toolbar() {
    let ossido = start("decor-app-move");
    let mut window = TestWindow::open_decorated(&ossido.wayland, "sposta", (240, 140));
    window.set_full_size_content(52);
    let info = windows(&ossido).remove(0);
    let mut client = ossido.client();
    let (x, y) = (info.x + 150, info.y + 26);
    client
        .call(INPUT_POINTER_MOVE, json!({"x": x, "y": y}))
        .unwrap();
    client
        .call(
            INPUT_POINTER_BUTTON,
            json!({"button": "left", "pressed": true}),
        )
        .unwrap();
    window.start_move();
    client
        .call(INPUT_POINTER_MOVE, json!({"x": x + 60, "y": y + 40}))
        .unwrap();
    client
        .call(
            INPUT_POINTER_BUTTON,
            json!({"button": "left", "pressed": false}),
        )
        .unwrap();
    let moved = windows(&ossido).remove(0);
    assert_eq!((moved.x, moved.y), (info.x + 60, info.y + 40));
    // Dopo il rilascio la richiesta non vale più: la finestra non segue il puntatore.
    window.start_move();
    client
        .call(INPUT_POINTER_MOVE, json!({"x": x + 200, "y": y + 100}))
        .unwrap();
    let still = windows(&ossido).remove(0);
    assert_eq!((still.x, still.y), (moved.x, moved.y));
}

/// I comandi di disposizione: le metà e il riempimento dell'area utile, cornice compresa (la
/// barra di 52 sopra il contenuto); due finestre affiancate.
#[test]
fn tile_commands_place_the_frame_in_the_usable_area() {
    use silica_control::commands::{WINDOW_ARRANGE_2, WINDOW_TILE_LEFT, WINDOW_TILE_RIGHT};
    let ossido = start("decor-tile");
    let _a = TestWindow::open_decorated(&ossido.wayland, "a", (300, 200));
    let _b = TestWindow::open_decorated(&ossido.wayland, "b", (300, 200));
    let half = (SIZE.0 / 2) as i32;
    let mut client = ossido.client();
    let by_title = |title: &str| {
        windows(&ossido)
            .into_iter()
            .find(|w| w.title.as_deref() == Some(title))
            .unwrap()
    };
    client.call(WINDOW_TILE_LEFT, json!({})).unwrap();
    let b = by_title("b");
    assert_eq!((b.x, b.y), (0, 52));
    client.call(WINDOW_TILE_RIGHT, json!({})).unwrap();
    let b = by_title("b");
    assert_eq!((b.x, b.y), (half, 52));
    // Disponi due: "b" (con il focus) a sinistra, "a" a destra.
    client.call(WINDOW_ARRANGE_2, json!({})).unwrap();
    assert_eq!(by_title("b").x, 0);
    assert_eq!(by_title("a").x, half);
}
