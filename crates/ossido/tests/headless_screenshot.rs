//! Avvia Ossido in headless e lo pilota dal canale di controllo.

// Codice di test: un errore deve far fallire il test (AGENTS.md vale per il codice non di test).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use base64::Engine;
use common::{HEIGHT, Headless, WIDTH, assert_clear_color, decode};
use silica_control::{Error, ScreenshotParams};

#[test]
fn screenshot_to_file_has_output_size_and_background() {
    let ossido = Headless::start("file");
    let path = ossido.dir.join("shot.png");
    let shot = ossido
        .client()
        .screenshot(&ScreenshotParams {
            path: Some(path.to_string_lossy().into_owned()),
        })
        .unwrap();
    assert_eq!((shot.width, shot.height), (WIDTH, HEIGHT));

    let (info, pixels) = decode(&std::fs::read(&path).unwrap());
    assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    for (x, y) in [(0, 0), (WIDTH / 2, HEIGHT / 2), (WIDTH - 1, HEIGHT - 1)] {
        assert_clear_color(&pixels, x, y);
    }
}

#[test]
fn screenshot_inline_returns_base64_png() {
    let ossido = Headless::start("inline");
    let mut client = ossido.client();
    let first = client.screenshot(&ScreenshotParams::default()).unwrap();
    // La connessione resta aperta per più comandi.
    let second = client.screenshot(&ScreenshotParams::default()).unwrap();

    for shot in [first, second] {
        assert!(shot.path.is_none());
        let png_bytes = base64::engine::general_purpose::STANDARD
            .decode(shot.png_base64.unwrap())
            .unwrap();
        let (info, _) = decode(&png_bytes);
        assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    }
}

#[test]
fn unknown_command_is_an_error_not_a_crash() {
    let ossido = Headless::start("unknown");
    let mut client = ossido.client();
    match client.call("ui.nope", serde_json::Value::Null) {
        Err(Error::Command(message)) => assert!(message.contains("ui.nope")),
        other => panic!("atteso un errore di comando, ottenuto {other:?}"),
    }
    // Ossido è ancora vivo e risponde.
    client.screenshot(&ScreenshotParams::default()).unwrap();
}
