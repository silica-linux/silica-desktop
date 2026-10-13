//! Gli spazi di lavoro (M5.1) su Ossido headless: finestre negli spazi, cambio di spazio,
//! fascia nera durante lo scorrimento (`plan/reference/spaces.md`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::time::Duration;

use common::client::TestWindow;
use common::{HEIGHT, Headless, WIDTH, pixel, screenshot};
use serde_json::{Value, json};
use silica_control::commands::{
    INPUT_KEY, SPACE_ADD, SPACE_LIST, SPACE_NEXT, SPACE_PREVIOUS, SPACE_REMOVE, SpaceInfo,
    WINDOW_FOCUS, WINDOW_LIST, WINDOW_MOVE_TO_SPACE, WindowInfo,
};

fn solid(size: (u32, u32), rgba: [u8; 4]) -> Vec<u8> {
    rgba.repeat((size.0 * size.1) as usize)
}

fn windows(ossido: &Headless) -> Vec<WindowInfo> {
    serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap()
}

fn spaces(ossido: &Headless) -> Vec<SpaceInfo> {
    serde_json::from_value(ossido.client().call(SPACE_LIST, Value::Null).unwrap()).unwrap()
}

fn by_title<'a>(list: &'a [WindowInfo], title: &str) -> &'a WindowInfo {
    list.iter()
        .find(|w| w.title.as_deref() == Some(title))
        .unwrap_or_else(|| panic!("finestra {title} assente: {list:?}"))
}

/// Il pixel al centro di una finestra.
fn center(window: &WindowInfo) -> (u32, u32) {
    (
        (window.x + window.width / 2) as u32,
        (window.y + window.height / 2) as u32,
    )
}

/// Rosso del pixel della finestra rossa (BGRA nel buffer di prova: il blu in memoria).
fn shows(shot: &[u8], at: (u32, u32), rgba: [u8; 4]) -> bool {
    let [r, g, b, _] = pixel(shot, at.0, at.1);
    r.abs_diff(rgba[0]) < 10 && g.abs_diff(rgba[1]) < 10 && b.abs_diff(rgba[2]) < 10
}

const RED: [u8; 4] = [0, 0, 255, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];
const RED_RGB: [u8; 4] = [255, 0, 0, 255];
const BLUE_RGB: [u8; 4] = [0, 0, 255, 255];

#[test]
fn windows_live_in_their_space() {
    let ossido = Headless::start("spaces");
    let _red = TestWindow::open_pixels(&ossido.wayland, "rossa", (100, 60), &solid((100, 60), RED));
    let _blue = TestWindow::open_pixels(&ossido.wayland, "blu", (100, 60), &solid((100, 60), BLUE));
    let mut c = ossido.client();

    assert_eq!(c.call(SPACE_ADD, Value::Null).unwrap(), json!({"index": 1}));
    let list = windows(&ossido);
    let blue = by_title(&list, "blu").clone();
    c.call(WINDOW_MOVE_TO_SPACE, json!({"id": blue.id, "index": 1}))
        .unwrap();

    let list = windows(&ossido);
    assert_eq!(by_title(&list, "rossa").space, 0);
    assert_eq!(by_title(&list, "blu").space, 1);
    let red = by_title(&list, "rossa").clone();
    let s = spaces(&ossido);
    assert_eq!(s.len(), 2);
    assert!(s[0].active);
    assert_eq!(s[0].windows, [red.id]);
    assert_eq!(s[1].windows, [blue.id]);

    // Solo lo spazio attivo è sullo schermo.
    let shot = screenshot(&ossido);
    assert!(shows(&shot, center(&red), RED_RGB));
    assert!(!shows(&shot, center(&blue), BLUE_RGB));

    // Spazio successivo (animazioni spente nei test): la blu, non la rossa.
    assert_eq!(
        c.call(SPACE_NEXT, Value::Null).unwrap(),
        json!({"switched": true})
    );
    let shot = screenshot(&ossido);
    assert!(shows(&shot, center(&blue), BLUE_RGB));
    assert!(!shows(&shot, center(&red), RED_RGB));
    assert!(spaces(&ossido)[1].active);
    // Oltre l'ultimo spazio non succede niente.
    assert_eq!(
        c.call(SPACE_NEXT, Value::Null).unwrap(),
        json!({"switched": false})
    );

    // window.focus su una finestra di un altro spazio porta in quello spazio.
    c.call(WINDOW_FOCUS, json!({"id": red.id})).unwrap();
    assert!(spaces(&ossido)[0].active);
    assert_eq!(
        c.call(SPACE_PREVIOUS, Value::Null).unwrap(),
        json!({"switched": false})
    );

    // Togliere lo spazio 1: la blu torna nello spazio 0, visibile.
    c.call(SPACE_REMOVE, json!({"index": 1})).unwrap();
    let shot = screenshot(&ossido);
    assert!(shows(&shot, center(&blue), BLUE_RGB));
    assert_eq!(spaces(&ossido).len(), 1);
    assert!(c.call(SPACE_REMOVE, json!({"index": 0})).is_err());
}

#[test]
fn switching_slides_with_a_black_gap() {
    // Animazioni 40 volte più lente: lo scorrimento dura ~30 s e si fotografa a metà.
    let ossido = Headless::start_with("spaces-slide", (WIDTH, HEIGHT), &[]);
    let mut c = ossido.client();
    c.call("ui.animation_scale", json!({"scale": 40})).unwrap();
    let _red = TestWindow::open_pixels(&ossido.wayland, "rossa", (100, 60), &solid((100, 60), RED));
    c.call(SPACE_ADD, Value::Null).unwrap();
    // Con ⌃→ dalla tastiera, come nel riferimento.
    c.call(INPUT_KEY, json!({"key": "Control_L", "pressed": true}))
        .unwrap();
    c.call(INPUT_KEY, json!({"key": "Right"})).unwrap();
    c.call(INPUT_KEY, json!({"key": "Control_L", "pressed": false}))
        .unwrap();
    std::thread::sleep(Duration::from_millis(4000));
    let shot = screenshot(&ossido);
    // Sulla riga centrale una fascia nera larga 64 (± il bordo antialias).
    let y = HEIGHT / 2;
    let black: Vec<u32> = (0..WIDTH)
        .filter(|&x| {
            let [r, g, b, _] = pixel(&shot, x, y);
            r < 4 && g < 4 && b < 4
        })
        .collect();
    assert!(!black.is_empty(), "nessuna fascia nera");
    let (first, last) = (black[0], *black.last().unwrap());
    assert!(
        black.len() as u32 == last - first + 1,
        "la fascia non è contigua: {black:?}"
    );
    assert!(
        (62..=65).contains(&(last - first + 1)),
        "larghezza {}",
        last - first + 1
    );
    assert!(spaces(&ossido)[1].active);
}
