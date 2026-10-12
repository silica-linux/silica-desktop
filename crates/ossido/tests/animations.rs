//! Animazioni di apertura e chiusura delle finestre (`plan/reference/windows.md`), rese
//! lente con `ui.animation_scale` per leggerle negli screenshot.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::client::{FIRST_WINDOW_AT, TestWindow};
use common::{Headless, pixel, screenshot};
use serde_json::json;
use silica_control::commands::UI_ANIMATION_SCALE;

/// Larghezza della parte rossa (la finestra) sulla riga `y`.
fn red_width(shot: &[u8], y: u32) -> u32 {
    (0..common::WIDTH)
        .filter(|&x| {
            let [r, g, b, _] = pixel(shot, x, y);
            r > 200 && g < 60 && b < 60
        })
        .count() as u32
}

fn set_speed(ossido: &Headless, scale: f64) {
    ossido
        .client()
        .call(UI_ANIMATION_SCALE, json!({ "scale": scale }))
        .unwrap();
}

#[test]
fn windows_open_from_98_percent() {
    // Durate ×100: l'apertura dura 10 s, il primo screenshot la coglie all'inizio.
    let ossido = Headless::start_with(
        "anim-open",
        (common::WIDTH, common::HEIGHT),
        &["--animation-scale", "100"],
    );
    let _window = TestWindow::open_sized(&ossido.wayland, "apre", (200, 120));
    let y = FIRST_WINDOW_AT + 60;
    let opening = red_width(&screenshot(&ossido), y);
    eprintln!("in apertura: {opening} px su 200");
    // Scala 0,98 → 196 px, al più qualche centesimo di avanzamento in più.
    assert!((195..=198).contains(&opening), "in apertura: {opening} px");

    set_speed(&ossido, 0.0);
    assert_eq!(red_width(&screenshot(&ossido), y), 200);
}

#[test]
fn closed_windows_fade_out() {
    let ossido = Headless::start("anim-close");
    let mut window = TestWindow::open_sized(&ossido.wayland, "chiude", (200, 120));
    let (x, y) = (FIRST_WINDOW_AT + 100, FIRST_WINDOW_AT + 60);
    assert_eq!(pixel(&screenshot(&ossido), x, y), [255, 0, 0, 255]);

    // Chiusura lenta: subito dopo, l'istantanea è ancora quasi opaca.
    set_speed(&ossido, 100.0);
    window.destroy_toplevel();
    let [r, ..] = pixel(&screenshot(&ossido), x, y);
    eprintln!("in chiusura: rosso {r}");
    assert!(
        r > 200,
        "la finestra chiusa non svanisce dall'ultima immagine: rosso {r}"
    );

    // A fine animazione resta il fondo.
    set_speed(&ossido, 0.0);
    common::assert_clear_color(&screenshot(&ossido), x, y);
}
