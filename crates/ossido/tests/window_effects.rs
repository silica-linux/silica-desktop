//! Effetti delle finestre confrontati con le misure di macOS 27
//! (`plan/reference/windows.md`), su Ossido headless a scala 1 come le riprese.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::client::{FIRST_WINDOW_AT, TestWindow};
use common::{Headless, pixel, pixel_in, screenshot};
use serde_json::json;
use silica_control::commands::{WINDOW_LIST, WINDOW_MOVE};

/// Pixel mancanti (255 − alfa, in pixel interi) per le righe 0…23 dell'angolo in alto a
/// sinistra di una finestra 900 × 600 (`windows.md`, "Finestre · forma e angoli").
const REFERENCE_MISSING: [f64; 24] = [
    16.00, 10.05, 7.90, 6.42, 5.26, 4.28, 3.47, 2.73, 2.14, 1.73, 1.37, 0.95, 0.85, 0.72, 0.59,
    0.47, 0.37, 0.30, 0.23, 0.18, 0.14, 0.10, 0.06, 0.03,
];

#[test]
fn window_corners_follow_the_reference() {
    let ossido = Headless::start("corners");
    let _window = TestWindow::open_sized(&ossido.wayland, "angoli", (200, 120));
    let shot = screenshot(&ossido);
    // Con OSSIDO_SHOTS=<cartella> lo screenshot resta, per guardarlo.
    if let Some(dir) = std::env::var_os("OSSIDO_SHOTS") {
        std::fs::copy(
            ossido.dir.join("shot.png"),
            std::path::Path::new(&dir).join("corners.png"),
        )
        .unwrap();
    }
    let at = FIRST_WINDOW_AT;
    // Il canale rosso va dal fondo (letto nell'angolo dell'output) a 255 nella finestra.
    let background = f64::from(pixel(&shot, 0, 0)[0]);
    let coverage =
        |x: u32, y: u32| (f64::from(pixel(&shot, x, y)[0]) - background) / (255.0 - background);

    let mut errors = Vec::new();
    for (row, reference) in REFERENCE_MISSING.iter().enumerate() {
        let y = at + row as u32;
        let missing: f64 = (0..40).map(|dx| 1.0 - coverage(at + dx, y)).sum();
        errors.push(missing - reference);
        eprintln!("riga {row}: {missing:.2} (riferimento {reference:.2})");
    }
    let worst = errors.iter().fold(0.0_f64, |m, e| m.max(e.abs()));
    let rms = (errors.iter().map(|e| e * e).sum::<f64>() / errors.len() as f64).sqrt();
    eprintln!("errore massimo {worst:.2} px, rms {rms:.2} px");
    assert!(
        worst <= 1.0,
        "angolo lontano dal riferimento: {worst:.2} px su una riga"
    );
    assert!(
        rms <= 0.5,
        "angolo lontano dal riferimento: rms {rms:.2} px"
    );

    // I quattro angoli sono uguali: il vertice è fondo, il centro è finestra.
    let (w, h) = (200, 120);
    for (x, y) in [
        (at, at),
        (at + w - 1, at),
        (at, at + h - 1),
        (at + w - 1, at + h - 1),
    ] {
        assert!(coverage(x, y) < 0.05, "angolo non tagliato in ({x}, {y})");
    }
    assert_eq!(pixel(&shot, at + w / 2, at + h / 2), [255, 0, 0, 255]);
}

/// Sfondo delle riprese di riferimento (`setup.md`).
const REFERENCE_GRAY: &str = "7a7b80";

/// Profilo laterale dell'ombra, alfa 0–255 per d = 2, 4, …, 20, 24, 28, 32, 40, 48 pt dal
/// bordo esterno dell'anello (`windows.md`, "Finestre · ombra", aspetto chiaro).
const PROFILE_D: [u32; 15] = [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 32, 40, 48];
const ACTIVE_PROFILE: [f64; 15] = [
    48., 44., 40., 36., 32., 30., 26., 22., 20., 16., 12., 7., 6., 2., 0.,
];
const INACTIVE_PROFILE: [f64; 15] = [
    24., 18., 14., 10., 6., 4., 2., 2., 1., 0., 0., 0., 0., 0., 0.,
];

/// Alfa (0–255) di un'ombra nera sopra il grigio di riferimento, dal canale verde.
fn shadow_alpha(shot: &[u8], x: u32, y: u32) -> f64 {
    let gray = f64::from(0x7b_u8);
    255.0 * (1.0 - f64::from(pixel_in(shot, 640, x, y)[1]) / gray)
}

/// Confronta il profilo a destra di una finestra (bordo destro a `right`, riga `y`).
fn compare_profile(shot: &[u8], right: u32, y: u32, reference: &[f64; 15], name: &str) {
    let mut worst = 0.0_f64;
    for (d, expected) in PROFILE_D.iter().zip(reference) {
        let measured = shadow_alpha(shot, right + d, y);
        eprintln!("{name} d={d}: {measured:.1} (riferimento {expected})");
        worst = worst.max((measured - expected).abs());
    }
    eprintln!("{name}: errore massimo {worst:.1} livelli");
    // Il verde a 8 bit su 123 risolve l'alfa a ~2 livelli; il riferimento è ±1.
    assert!(
        worst <= 6.0,
        "ombra {name} lontana dal riferimento: {worst:.1} livelli"
    );
}

#[test]
fn window_shadows_follow_the_reference() {
    let ossido = Headless::start_with("shadow", (640, 400), &["--background", REFERENCE_GRAY]);
    let _first = TestWindow::open_sized(&ossido.wayland, "prima", (200, 120));
    let _second = TestWindow::open_sized(&ossido.wayland, "seconda", (200, 120));
    // La seconda (attiva) va a destra; la prima resta in (32, 32), inattiva.
    let mut client = ossido.client();
    client
        .call(WINDOW_MOVE, json!({"x": 330, "y": 120}))
        .unwrap();
    let list = client.call(WINDOW_LIST, json!({})).unwrap();
    assert_eq!(list.as_array().map(Vec::len), Some(2), "{list}");
    let shot = screenshot(&ossido);
    if let Some(dir) = std::env::var_os("OSSIDO_SHOTS") {
        std::fs::copy(
            ossido.dir.join("shot.png"),
            std::path::Path::new(&dir).join("shadow.png"),
        )
        .unwrap();
    }

    // Attiva: bordo destro a x 530, metà altezza y 180.
    compare_profile(&shot, 530, 180, &ACTIVE_PROFILE, "attiva");
    // Inattiva: bordo destro a x 232, metà altezza y 92 (la seconda è più a destra).
    compare_profile(&shot, 232, 92, &INACTIVE_PROFILE, "inattiva");

    // Anello a contatto: attiva 81 ai lati, inattiva 59 (±6).
    let ring_active = shadow_alpha(&shot, 530, 180);
    let ring_inactive = shadow_alpha(&shot, 232, 92);
    eprintln!("anello: attiva {ring_active:.1} (81), inattiva {ring_inactive:.1} (59)");
    assert!(
        (ring_active - 81.0).abs() <= 6.0,
        "anello attivo {ring_active:.1}"
    );
    assert!(
        (ring_inactive - 59.0).abs() <= 6.0,
        "anello inattivo {ring_inactive:.1}"
    );

    // Spostamento verticale: sotto più ombra che sopra (attiva, a metà larghezza).
    let above = shadow_alpha(&shot, 430, 120 - 3);
    let below = shadow_alpha(&shot, 430, 240 + 2);
    eprintln!("attiva: sopra {above:.1}, sotto {below:.1}");
    assert!(
        below > above + 20.0,
        "ombra non spostata in basso: sopra {above:.1}, sotto {below:.1}"
    );
}
