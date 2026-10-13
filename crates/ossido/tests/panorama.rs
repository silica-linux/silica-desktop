//! Panorama (M5.2a) su Ossido headless: finestre disposte senza sovrapporsi, barra degli
//! spazi in alto, clic su una finestra, Esc (`plan/reference/missioncontrol.md`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::client::TestWindow;
use common::{Headless, pixel, screenshot};
use serde_json::{Value, json};
use silica_control::commands::{
    INPUT_CLICK, INPUT_KEY, PANORAMA_STATE, PANORAMA_TOGGLE, PanoramaState, SPACE_LIST, SpaceInfo,
    WINDOW_LIST, WindowInfo,
};

fn solid(size: (u32, u32), rgba: [u8; 4]) -> Vec<u8> {
    rgba.repeat((size.0 * size.1) as usize)
}

fn state(ossido: &Headless) -> PanoramaState {
    serde_json::from_value(ossido.client().call(PANORAMA_STATE, Value::Null).unwrap()).unwrap()
}

fn windows(ossido: &Headless) -> Vec<WindowInfo> {
    serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap()
}

fn close(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).take(3).all(|(x, y)| x.abs_diff(y) < 12)
}

// Nel buffer di prova i canali sono BGRA.
const RED: [u8; 4] = [0, 0, 255, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];

#[test]
fn windows_spread_out_and_a_click_picks_one() {
    let ossido = Headless::start("panorama");
    let _red = TestWindow::open_pixels(&ossido.wayland, "rossa", (100, 60), &solid((100, 60), RED));
    let _blue = TestWindow::open_pixels(&ossido.wayland, "blu", (100, 60), &solid((100, 60), BLUE));
    let mut c = ossido.client();
    assert!(!state(&ossido).open);

    assert_eq!(
        c.call(PANORAMA_TOGGLE, Value::Null).unwrap(),
        json!({"open": true})
    );
    let s = state(&ossido);
    assert!(s.open);
    assert_eq!(s.progress, 1.0);
    assert_eq!(s.windows.len(), 2);
    // Entrano tutte e due a grandezza piena, 16 pt fra loro, sotto la barra.
    let (a, b) = (&s.windows[0], &s.windows[1]);
    let (left, right) = if a.x < b.x { (a, b) } else { (b, a) };
    assert_eq!(right.x - (left.x + left.width), 16.0);
    assert!(left.y >= 40.0 && right.y >= 40.0);

    let shot = screenshot(&ossido);
    // La barra degli spazi: grigio chiaro in alto a sinistra, nessuna finestra.
    assert!(
        close(pixel(&shot, 4, 4), [0x9e, 0x9f, 0xa5, 255]),
        "{:?}",
        pixel(&shot, 4, 4)
    );
    let list = windows(&ossido);
    let red_id = list
        .iter()
        .find(|w| w.title.as_deref() == Some("rossa"))
        .unwrap()
        .id;
    let blue_id = list
        .iter()
        .find(|w| w.title.as_deref() == Some("blu"))
        .unwrap()
        .id;
    for (id, rgb) in [(red_id, [255, 0, 0, 255]), (blue_id, [0, 0, 255, 255])] {
        let w = s.windows.iter().find(|w| w.id == id).unwrap();
        let (x, y) = (w.x + w.width / 2.0, w.y + w.height / 2.0);
        assert!(
            close(pixel(&shot, x as u32, y as u32), rgb),
            "finestra {id} a {x},{y}: {:?}",
            pixel(&shot, x as u32, y as u32)
        );
    }

    // Un clic sulla rossa la porta in primo piano e chiude Panorama.
    let red = s.windows.iter().find(|w| w.id == red_id).unwrap();
    c.call(
        INPUT_CLICK,
        json!({"x": red.x + red.width / 2.0, "y": red.y + red.height / 2.0}),
    )
    .unwrap();
    assert!(!state(&ossido).open);
    let list = windows(&ossido);
    assert!(list.iter().find(|w| w.id == red_id).unwrap().focused);

    // Esc chiude; il "+" aggiunge uno spazio.
    c.call(PANORAMA_TOGGLE, Value::Null).unwrap();
    c.call(INPUT_CLICK, json!({"x": 320 - 26, "y": 20}))
        .unwrap();
    let spaces: Vec<SpaceInfo> =
        serde_json::from_value(c.call(SPACE_LIST, Value::Null).unwrap()).unwrap();
    assert_eq!(spaces.len(), 2);
    assert!(state(&ossido).open);
    c.call(INPUT_KEY, json!({"key": "Escape"})).unwrap();
    assert!(!state(&ossido).open);
    // Chiuso, la barra dei menu e le finestre tornano al loro posto.
    let shot = screenshot(&ossido);
    assert!(!close(pixel(&shot, 4, 4), [0x9e, 0x9f, 0xa5, 255]));
}
