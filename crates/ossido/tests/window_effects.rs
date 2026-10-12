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
use common::{Headless, pixel, screenshot};

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
