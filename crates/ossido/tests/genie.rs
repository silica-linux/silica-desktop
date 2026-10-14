//! Riduzione nel Dock a genio, ripristino e schermo intero (`plan/reference/windows.md`,
//! "Animazioni"), rallentati con `ui.animation_scale` per leggerli negli screenshot.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::time::Duration;

use common::client::TestWindow;
use common::{Headless, pixel_in};
use serde_json::{Value, json};
use silica_control::commands::{
    UI_ANIMATION_SCALE, WINDOW_FOCUS, WINDOW_FULLSCREEN, WINDOW_LIST, WINDOW_MINIMIZE, WindowInfo,
};

const SIZE: (u32, u32) = (640, 400);
/// Senza Dock la miniatura va in basso al centro: centro (320, 360), larga 57.
const TARGET: (i32, i32) = (320, 360);

fn start(name: &str) -> Headless {
    Headless::start_with(name, SIZE, &["--background", "7a7b80"])
}

fn windows(ossido: &Headless) -> Vec<WindowInfo> {
    serde_json::from_value(ossido.client().call(WINDOW_LIST, Value::Null).unwrap()).unwrap()
}

fn set_speed(ossido: &Headless, scale: f64) {
    ossido
        .client()
        .call(UI_ANIMATION_SCALE, json!({ "scale": scale }))
        .unwrap();
}

fn shot(ossido: &Headless, name: &str) -> Vec<u8> {
    let shot = common::screenshot(ossido);
    if let Some(dir) = std::env::var_os("OSSIDO_SHOTS") {
        std::fs::copy(
            ossido.dir.join("shot.png"),
            std::path::Path::new(&dir).join(format!("genie-{name}.png")),
        )
        .unwrap();
    }
    shot
}

fn is_red(shot: &[u8], x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= SIZE.0 as i32 || y >= SIZE.1 as i32 {
        return false;
    }
    let [r, g, b, _] = pixel_in(shot, SIZE.0, x as u32, y as u32);
    r > 180 && g < 90 && b < 90
}

/// I pixel rossi (il contenuto della finestra di prova) su una riga: primo e ultimo.
fn red_span(shot: &[u8], y: i32) -> Option<(i32, i32)> {
    let xs: Vec<i32> = (0..SIZE.0 as i32).filter(|&x| is_red(shot, x, y)).collect();
    Some((*xs.first()?, *xs.last()?))
}

fn red_count(shot: &[u8]) -> usize {
    (0..SIZE.1 as i32)
        .flat_map(|y| (0..SIZE.0 as i32).map(move |x| (x, y)))
        .filter(|&(x, y)| is_red(shot, x, y))
        .count()
}

#[test]
fn minimize_bends_the_window_into_the_dock_and_restore_brings_it_back() {
    let ossido = start("genie");
    let _window = TestWindow::open_decorated(&ossido.wayland, "genio", (240, 140));
    let info = windows(&ossido).remove(0);
    let (left, right) = (info.x, info.x + 239);
    let mid_row = info.y + 120;
    assert_eq!(
        red_span(&shot(&ossido, "prima"), mid_row),
        Some((left, right))
    );

    // ×20: la riduzione dura 9,6 s.
    set_speed(&ossido, 20.0);
    ossido
        .client()
        .call(WINDOW_MINIMIZE, json!({ "id": info.id }))
        .unwrap();
    // All'inizio l'istantanea è la finestra com'era: barra bianca sopra, rosso sotto (il
    // verso della texture è giusto).
    let at_start = shot(&ossido, "inizio");
    // (Cambiare velocità ricalcola anche l'apertura: la finestra può essere ancora al 98 %.)
    let (a, b) = red_span(&at_start, mid_row).expect("finestra all'inizio");
    assert!((a - left).abs() <= 4 && (b - right).abs() <= 4, "{a}–{b}");
    let [r, g, b, _] = pixel_in(&at_start, SIZE.0, (left + 210) as u32, (info.y - 26) as u32);
    assert!(r > 230 && g > 230 && b > 230, "barra in alto: {r} {g} {b}");

    // A metà (~215 ms su 480): il fondo della finestra è piegato verso la miniatura, che sta
    // a destra e in basso; la cima si è staccata dal posto di prima.
    std::thread::sleep(Duration::from_millis(4300));
    let middle = shot(&ossido, "meta");
    let bottom = info.y + 135;
    let (bent_left, bent_right) = red_span(&middle, bottom).expect("il fondo è ancora visibile");
    assert!(
        bent_left > left + 30,
        "il bordo sinistro in basso va verso la miniatura: {bent_left} (prima {left})"
    );
    assert!(bent_right - bent_left < 239 - 40, "l'imbuto si stringe");
    assert!(
        !is_red(&middle, left + 5, info.y + 2),
        "la cima della finestra è scesa"
    );
    // Il genio è rivolto alla miniatura: c'è rosso più in basso della finestra di prima.
    let lowest = (info.y + 140..SIZE.1 as i32)
        .rev()
        .find(|&y| red_span(&middle, y).is_some());
    assert!(lowest.is_some(), "niente imbuto sotto la finestra");

    // Alla fine la finestra non c'è più.
    set_speed(&ossido, 0.0);
    assert_eq!(red_count(&shot(&ossido, "fine")), 0);

    // Ripristino lento: la finestra è mappata ma si vede solo il genio, che parte dalla
    // miniatura (il posto della finestra è ancora vuoto).
    set_speed(&ossido, 20.0);
    ossido
        .client()
        .call(WINDOW_FOCUS, json!({ "id": info.id }))
        .unwrap();
    let restoring = shot(&ossido, "ripristino");
    assert!(
        !is_red(&restoring, left + 120, info.y + 40),
        "la finestra ripristinata resta nascosta finché il genio non finisce"
    );
    let near_target = (TARGET.1 - 30..SIZE.1 as i32).any(|y| {
        red_span(&restoring, y).is_some_and(|(a, b)| a <= TARGET.0 + 40 && b >= TARGET.0 - 40)
    });
    assert!(near_target, "il ripristino parte dalla miniatura");

    // Finito il genio, la finestra è al suo posto.
    set_speed(&ossido, 0.0);
    assert_eq!(
        red_span(&shot(&ossido, "ripristinata"), mid_row),
        Some((left, right))
    );
}

#[test]
fn speed_zero_minimizes_and_restores_at_once() {
    let ossido = start("genie-zero");
    let _window = TestWindow::open_decorated(&ossido.wayland, "subito", (240, 140));
    let info = windows(&ossido).remove(0);
    set_speed(&ossido, 0.0);
    ossido
        .client()
        .call(WINDOW_MINIMIZE, json!({ "id": info.id }))
        .unwrap();
    assert_eq!(red_count(&shot(&ossido, "zero")), 0);
    ossido
        .client()
        .call(WINDOW_FOCUS, json!({ "id": info.id }))
        .unwrap();
    assert_eq!(
        red_span(&shot(&ossido, "zero-ripristino"), info.y + 120),
        Some((info.x, info.x + 239))
    );
}

#[test]
fn the_genie_lasts_as_measured() {
    let ossido = start("genie-durata");
    let _window = TestWindow::open_decorated(&ossido.wayland, "durata", (240, 140));
    let info = windows(&ossido).remove(0);
    // A velocità normale: 480 ms. A 300 ms c'è ancora qualcosa, a 600 ms niente.
    set_speed(&ossido, 1.0);
    ossido
        .client()
        .call(WINDOW_MINIMIZE, json!({ "id": info.id }))
        .unwrap();
    std::thread::sleep(Duration::from_millis(250));
    let during = red_count(&shot(&ossido, "durata-250"));
    assert!(during > 0, "a 250 ms il genio è ancora in corso");
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(red_count(&shot(&ossido, "durata-650")), 0);
}

#[test]
fn closing_a_window_while_it_comes_out_of_the_dock_does_not_crash() {
    let ossido = start("genie-chiusa");
    let mut window = TestWindow::open_decorated(&ossido.wayland, "chiusa", (240, 140));
    let info = windows(&ossido).remove(0);
    set_speed(&ossido, 0.0);
    ossido
        .client()
        .call(WINDOW_MINIMIZE, json!({ "id": info.id }))
        .unwrap();
    set_speed(&ossido, 20.0);
    ossido
        .client()
        .call(WINDOW_FOCUS, json!({ "id": info.id }))
        .unwrap();
    shot(&ossido, "chiusa-prima");
    window.destroy_toplevel();
    drop(window);
    std::thread::sleep(Duration::from_millis(100));
    // Ossido risponde ancora e il genio della finestra che non c'è più sparisce.
    let after = shot(&ossido, "chiusa-dopo");
    assert!(windows(&ossido).is_empty());
    set_speed(&ossido, 0.0);
    let _ = after;
    assert_eq!(red_count(&shot(&ossido, "chiusa-fine")), 0);
}

#[test]
fn fullscreen_grows_the_window_to_the_screen() {
    let ossido = start("genie-schermo-intero");
    let _window = TestWindow::open_decorated(&ossido.wayland, "intero", (240, 140));
    let info = windows(&ossido).remove(0);
    let width = 240;
    // ×20: l'entrata dura 10,4 s; a ~2,6 s (130 ms) il rettangolo è a ~0,55.
    set_speed(&ossido, 20.0);
    ossido
        .client()
        .call(WINDOW_FULLSCREEN, json!({ "id": info.id }))
        .unwrap();
    let start = shot(&ossido, "intero-inizio");
    let row = info.y + 70;
    // (Cambiare velocità ricalcola anche l'apertura: la finestra può essere ancora al 98 %.)
    let (a, b) = red_span(&start, row).expect("finestra all'inizio");
    assert!(
        (b - a + 1 - width).abs() <= 6,
        "all'inizio è com'era: {}",
        b - a + 1
    );
    std::thread::sleep(Duration::from_millis(2600));
    let middle = shot(&ossido, "intero-meta");
    let widest = (0..SIZE.1 as i32)
        .filter_map(|y| red_span(&middle, y))
        .map(|(a, b)| b - a + 1)
        .max()
        .expect("finestra a metà");
    assert!(
        widest > width + 60 && widest < SIZE.0 as i32,
        "a metà la finestra cresce verso lo schermo: {widest}"
    );

    // Finita l'entrata, l'uscita lenta (×20: 9 s): a ~2,4 s (120 ms, frazione ~0,56) la
    // cornice sta fra lo schermo e la cornice di prima.
    set_speed(&ossido, 0.0);
    shot(&ossido, "intero-fatto");
    set_speed(&ossido, 20.0);
    ossido
        .client()
        .call(WINDOW_FULLSCREEN, json!({ "id": info.id }))
        .unwrap();
    std::thread::sleep(Duration::from_millis(2400));
    let leaving = shot(&ossido, "intero-uscita");
    let widest = (0..SIZE.1 as i32)
        .filter_map(|y| red_span(&leaving, y))
        .map(|(a, b)| b - a + 1)
        .max()
        .expect("finestra in uscita");
    assert!(
        widest > width + 20 && widest < SIZE.0 as i32 - 20,
        "in uscita la finestra torna alla sua cornice: {widest}"
    );
    // Alla fine è di nuovo com'era.
    set_speed(&ossido, 0.0);
    let (a, b) = red_span(&shot(&ossido, "intero-tornata"), row).expect("finestra tornata");
    assert_eq!(b - a + 1, width);
}
