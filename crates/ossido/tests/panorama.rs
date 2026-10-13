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
use common::{Headless, pixel, pixel_in, screenshot};
use serde_json::{Value, json};
use silica_control::commands::{
    INPUT_CLICK, INPUT_KEY, INPUT_POINTER_BUTTON, INPUT_POINTER_MOVE, PANORAMA_SPACES_BAR,
    PANORAMA_STATE, PANORAMA_TOGGLE, PanoramaState, SPACE_ADD, SPACE_LIST, SpaceInfo, WINDOW_LIST,
    WindowInfo,
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

    // Esc chiude; il "+" aggiunge uno spazio. Andarci espande la barra e il "+" scende
    // con lei (centro a y 101,5).
    c.call(PANORAMA_TOGGLE, Value::Null).unwrap();
    c.call(INPUT_POINTER_MOVE, json!({"x": 320 - 26, "y": 20}))
        .unwrap();
    c.call(INPUT_CLICK, json!({"x": 320 - 26, "y": 101}))
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

fn spaces(ossido: &Headless) -> Vec<SpaceInfo> {
    serde_json::from_value(ossido.client().call(SPACE_LIST, Value::Null).unwrap()).unwrap()
}

#[test]
fn a_window_dragged_onto_a_thumbnail_changes_space() {
    // Uno schermo grande abbastanza da avere le finestre sotto la barra espansa (170 pt).
    let ossido = Headless::start_with("panorama-drag", (1280, 800), &[]);
    let size = (300, 300);
    let _red = TestWindow::open_pixels(&ossido.wayland, "rossa", size, &solid(size, RED));
    let _blue = TestWindow::open_pixels(&ossido.wayland, "blu", size, &solid(size, BLUE));
    let mut c = ossido.client();
    c.call(SPACE_ADD, Value::Null).unwrap();
    c.call(PANORAMA_TOGGLE, Value::Null).unwrap();
    // Il puntatore in alto espande la barra: compaiono le miniature.
    c.call(INPUT_POINTER_MOVE, json!({"x": 100, "y": 5}))
        .unwrap();
    let s = state(&ossido);
    assert_eq!(s.expansion, 1.0);
    assert_eq!(s.thumbnails.len(), 2);
    // Sotto la barra espansa si richiude; il comando la riapre.
    c.call(INPUT_POINTER_MOVE, json!({"x": 100, "y": 799}))
        .unwrap();
    assert_eq!(state(&ossido).expansion, 0.0);
    c.call(PANORAMA_SPACES_BAR, json!({"enabled": true}))
        .unwrap();
    let s = state(&ossido);
    assert_eq!(s.expansion, 1.0);
    let shot = screenshot(&ossido);
    // Anello blu attorno alla miniatura dello spazio corrente (1 pt di barra, poi 3 pt).
    let t = s.thumbnails[0];
    let ring = pixel_in(
        &shot,
        1280,
        (t.x + t.width / 2.0) as u32,
        (t.y - 2.5) as u32,
    );
    assert!(close(ring, [0x3b, 0x99, 0xfd, 255]), "{ring:?}");

    let blue_id = windows(&ossido)
        .iter()
        .find(|w| w.title.as_deref() == Some("blu"))
        .unwrap()
        .id;
    let blue = *s.windows.iter().find(|w| w.id == blue_id).unwrap();
    let target = s.thumbnails[1];
    c.call(
        INPUT_POINTER_MOVE,
        json!({"x": blue.x + blue.width / 2.0, "y": blue.y + blue.height / 2.0}),
    )
    .unwrap();
    c.call(INPUT_POINTER_BUTTON, json!({"pressed": true}))
        .unwrap();
    // Verso l'alto: arrivata alla barra compatta la barra si espande.
    c.call(INPUT_POINTER_MOVE, json!({"x": target.x, "y": 10}))
        .unwrap();
    c.call(
        INPUT_POINTER_MOVE,
        json!({"x": target.x + target.width / 2.0, "y": target.y + target.height / 2.0}),
    )
    .unwrap();
    c.call(INPUT_POINTER_BUTTON, json!({"pressed": false}))
        .unwrap();
    // La blu è passata allo spazio 2; Panorama resta aperto con la sola rossa.
    assert_eq!(spaces(&ossido)[1].windows, [blue_id]);
    let s = state(&ossido);
    assert!(s.open);
    assert_eq!(s.windows.len(), 1);

    // Un clic sulla miniatura dello spazio 2 ci passa e chiude Panorama.
    c.call(
        INPUT_CLICK,
        json!({"x": target.x + target.width / 2.0, "y": target.y + target.height / 2.0}),
    )
    .unwrap();
    assert!(!state(&ossido).open);
    assert!(spaces(&ossido)[1].active);
}

#[test]
fn app_windows_and_the_desktop() {
    let ossido = Headless::start_with("panorama-app", (1280, 800), &[]);
    let size = (300, 200);
    let _red = TestWindow::open_pixels(&ossido.wayland, "rossa", size, &solid(size, RED));
    let _blue = TestWindow::open_pixels(&ossido.wayland, "blu", size, &solid(size, BLUE));
    let mut c = ossido.client();
    let list = windows(&ossido);
    let focused = list.iter().find(|w| w.focused).unwrap().clone();
    let other = list.iter().find(|w| !w.focused).unwrap().clone();

    // App Exposé (⌃↓): solo la finestra dell'app in primo piano (le finestre di prova non
    // hanno app_id), centrata, in scala al più 0,947; l'altra non si vede.
    c.call(INPUT_KEY, json!({"key": "Control_L", "pressed": true}))
        .unwrap();
    c.call(INPUT_KEY, json!({"key": "Down"})).unwrap();
    c.call(INPUT_KEY, json!({"key": "Control_L", "pressed": false}))
        .unwrap();
    let s = state(&ossido);
    assert_eq!(s.mode.as_deref(), Some("app"));
    assert_eq!(s.windows.len(), 1);
    let w = s.windows[0];
    assert_eq!(w.id, focused.id);
    assert!((w.x + w.width / 2.0 - 640.0).abs() < 1.0, "{w:?}");
    let shot = screenshot(&ossido);
    let at = |w: &WindowInfo| ((w.x + w.width / 2) as u32, (w.y + w.height / 2) as u32);
    let hidden = pixel_in(&shot, 1280, at(&other).0, at(&other).1);
    assert!(!close(hidden, [255, 0, 0, 255]) && !close(hidden, [0, 0, 255, 255]));
    // Esc chiude.
    c.call(INPUT_KEY, json!({"key": "Escape"})).unwrap();
    assert!(state(&ossido).mode.is_none());

    // Mostra scrivania: le finestre escono quasi del tutto; il centro dello schermo è libero.
    assert_eq!(
        c.call(silica_control::commands::DESKTOP_SHOW, Value::Null)
            .unwrap(),
        json!({"open": true})
    );
    let s = state(&ossido);
    assert_eq!(s.mode.as_deref(), Some("desktop"));
    for w in &s.windows {
        let inside_x = (w.x + w.width).min(1280.0) - w.x.max(0.0);
        let inside_y = (w.y + w.height).min(800.0) - w.y.max(0.0);
        assert!(inside_x <= 24.0 + 1e-6 || inside_y <= 24.0 + 1e-6, "{w:?}");
    }
    // Un clic le rimette a posto.
    c.call(INPUT_CLICK, json!({"x": 640, "y": 400})).unwrap();
    assert!(state(&ossido).mode.is_none());
}
