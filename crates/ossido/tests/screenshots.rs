//! Le istantanee: lo schermo, una regione, una finestra da sola (⇧⌘3, ⇧⌘4); i file PNG
//! temporanei che la shell mostra in miniatura e poi sposta sulla Scrivania.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::Headless;
use common::client::{FIRST_WINDOW_AT, TestWindow};
use serde_json::{Value, json};
use silica_control::commands::{SCREENSHOT_REGION, SCREENSHOT_SCREEN, SCREENSHOT_WINDOW_AT};

fn png_size(path: &str) -> (u32, u32) {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let reader = decoder.read_info().unwrap();
    let info = reader.info();
    (info.width, info.height)
}

#[test]
fn screen_region_and_window_captures_write_png_files() {
    let ossido = Headless::start_with("screenshots", (640, 400), &[]);
    let _window = TestWindow::open_decorated(&ossido.wayland, "da catturare", (200, 120));
    let mut client = ossido.client();
    let call = |client: &mut silica_control::Client, id: &str, params: Value| {
        let reply = client.call(id, params).unwrap();
        let path = reply["path"].as_str().unwrap().to_owned();
        let size = png_size(&path);
        assert_eq!(
            size,
            (
                reply["width"].as_u64().unwrap() as u32,
                reply["height"].as_u64().unwrap() as u32
            )
        );
        std::fs::remove_file(&path).unwrap();
        size
    };
    assert_eq!(
        call(&mut client, SCREENSHOT_SCREEN, Value::Null),
        (640, 400)
    );
    assert_eq!(
        call(
            &mut client,
            SCREENSHOT_REGION,
            json!({"x": 10, "y": 20, "width": 100, "height": 50})
        ),
        (100, 50)
    );
    // La finestra con la sua barra di 52: 200 × (120 + 52).
    let at = f64::from(FIRST_WINDOW_AT) + 10.0;
    assert_eq!(
        call(&mut client, SCREENSHOT_WINDOW_AT, json!({"x": at, "y": at})),
        (200, 172)
    );
    // Fuori da ogni finestra: un errore, nessun file.
    assert!(
        client
            .call(SCREENSHOT_WINDOW_AT, json!({"x": 630, "y": 390}))
            .is_err()
    );
}
