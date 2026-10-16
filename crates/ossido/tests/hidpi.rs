//! Scaled modes (#216): `output.configure` with a logical size, the output list, input and
//! captures in logical and native coordinates.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::client::{FIRST_WINDOW_AT, TestWindow, WINDOW};
use common::{Headless, decode, pixel_in};
use serde_json::{Value, json};
use silica_control::commands::{INPUT_POINTER_MOVE, SCREENSHOT_REGION, SCREENSHOT_SCREEN};

const NATIVE: (u32, u32) = (1280, 800);

fn configure(ossido: &Headless, params: Value) -> Result<Value, String> {
    ossido
        .client()
        .call("output.configure", params)
        .map_err(|e| e.to_string())
}

fn output(ossido: &Headless) -> Value {
    ossido
        .client()
        .call("output.list", Value::Null)
        .unwrap()
        .as_array()
        .unwrap()[0]
        .clone()
}

/// The screen as the control channel captures it: size and RGBA.
fn shot(ossido: &Headless) -> ((u32, u32), Vec<u8>) {
    let reply = ossido
        .client()
        .call(SCREENSHOT_SCREEN, Value::Null)
        .unwrap();
    let path = reply["path"].as_str().unwrap().to_owned();
    let (info, pixels) = decode(&std::fs::read(&path).unwrap());
    std::fs::remove_file(path).unwrap();
    ((info.width, info.height), pixels)
}

#[test]
fn a_logical_size_is_configured_listed_saved_and_left_again() {
    let ossido = Headless::start_with("hidpi-config", NATIVE, &[]);
    let before = output(&ossido);
    assert_eq!(before["hidpi"], false);
    assert_eq!(before["logical"], json!({"width": 1280, "height": 800}));
    let looks: Vec<(i64, i64, bool)> = before["looksLike"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["width"].as_i64().unwrap(),
                l["height"].as_i64().unwrap(),
                l["scaled"].as_bool().unwrap(),
            )
        })
        .collect();
    // The display's own size first, not scaled; then the "looks like" sizes.
    assert_eq!(looks[0], (1280, 800, false));
    for wanted in [(1120, 700), (1024, 640), (960, 600)] {
        assert!(looks.contains(&(wanted.0, wanted.1, true)), "{looks:?}");
    }
    let connector = before["connector"].as_str().unwrap().to_owned();

    let scaled = configure(&ossido, json!({"logical": {"width": 1024, "height": 640}})).unwrap();
    assert_eq!(scaled["hidpi"], true);
    assert_eq!(scaled["logical"], json!({"width": 1024, "height": 640}));
    // The display keeps its mode, and the scale the user chose is untouched.
    assert_eq!(scaled["current"]["width"], 1280);
    assert_eq!(scaled["current"]["height"], 800);
    assert_eq!(scaled["scale"], 1.0);
    let saved: Value = serde_json::from_slice(
        &std::fs::read(ossido.dir.join("config/silica/displays.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        saved["outputs"][&connector]["logical"],
        json!({"width": 1024, "height": 640})
    );
    assert_eq!(output(&ossido), scaled);

    // Not possible: another shape, below half, above the display, with a scale.
    for bad in [
        json!({"logical": {"width": 1024, "height": 700}}),
        json!({"logical": {"width": 500, "height": 312}}),
        json!({"logical": {"width": 1600, "height": 1000}}),
        json!({"logical": {"width": 1024, "height": 640}, "scale": 2}),
        json!({"logical": {"width": 1024}}),
    ] {
        assert!(configure(&ossido, bad.clone()).is_err(), "{bad}");
    }
    assert_eq!(output(&ossido), scaled);

    // Another size replaces it; the display's own size or null leaves the scaled mode; a
    // scale replaces it too.
    let other = configure(&ossido, json!({"logical": {"width": 960, "height": 600}})).unwrap();
    assert_eq!(other["logical"], json!({"width": 960, "height": 600}));
    let plain = configure(&ossido, json!({"logical": {"width": 1280, "height": 800}})).unwrap();
    assert_eq!(plain["hidpi"], false);
    assert_eq!(plain["logical"], json!({"width": 1280, "height": 800}));
    configure(&ossido, json!({"logical": {"width": 1024, "height": 640}})).unwrap();
    let cleared = configure(&ossido, json!({"logical": null})).unwrap();
    assert_eq!(cleared["hidpi"], false);
    configure(&ossido, json!({"logical": {"width": 1024, "height": 640}})).unwrap();
    let fractional = configure(&ossido, json!({"scale": 1.25})).unwrap();
    assert_eq!(fractional["hidpi"], false);
    assert_eq!(fractional["scale"], 1.25);
    assert_eq!(fractional["logical"], json!({"width": 1024, "height": 640}));
    let saved: Value = serde_json::from_slice(
        &std::fs::read(ossido.dir.join("config/silica/displays.json")).unwrap(),
    )
    .unwrap();
    assert!(saved["outputs"][&connector]["logical"].is_null());
}

#[test]
fn a_saved_logical_size_applies_when_the_file_changes() {
    let ossido = Headless::start_with("hidpi-file", NATIVE, &[]);
    let connector = output(&ossido)["connector"].as_str().unwrap().to_owned();
    let dir = ossido.dir.join("config/silica");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("displays.json"),
        json!({"outputs": {connector: {"logical": {"width": 1120, "height": 700}}}}).to_string(),
    )
    .unwrap();
    let start = std::time::Instant::now();
    loop {
        if output(&ossido)["hidpi"] == true {
            break;
        }
        assert!(start.elapsed().as_secs() < 10, "the file was not applied");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(
        output(&ossido)["logical"],
        json!({"width": 1120, "height": 700})
    );
}

#[test]
fn input_and_captures_follow_the_logical_and_native_sizes() {
    let ossido = Headless::start_with("hidpi-map", NATIVE, &[]);
    let mut window = TestWindow::open(&ossido.wayland, "scaled");
    configure(&ossido, json!({"logical": {"width": 1024, "height": 640}})).unwrap();

    // The screenshot is what the display shows: its own size, the window 1.25x larger than
    // its 100 x 60 points, at the same logical place (32, 32) -> (40, 40).
    let ((width, height), pixels) = shot(&ossido);
    assert_eq!((width, height), NATIVE);
    let red = |x: u32, y: u32| pixel_in(&pixels, width, x, y) == [255, 0, 0, 255];
    let at = FIRST_WINDOW_AT as f64 * 1.25;
    let (w, h) = (WINDOW.0 as f64 * 1.25, WINDOW.1 as f64 * 1.25);
    let (x0, y0) = (at as u32, at as u32);
    for (x, y) in [
        (x0 + (w / 2.0) as u32, y0 + (h / 2.0) as u32),
        (x0 + 3, y0 + (h / 2.0) as u32),
        (x0 + w as u32 - 4, y0 + (h / 2.0) as u32),
        (x0 + (w / 2.0) as u32, y0 + 3),
        (x0 + (w / 2.0) as u32, y0 + h as u32 - 4),
    ] {
        assert!(red(x, y), "window missing at native ({x}, {y})");
    }
    // Past the window's right edge at 1.25x, and where the unscaled size would still be red.
    assert!(!red(x0 + w as u32 + 4, y0 + 30));
    assert!(!red(x0 + 30, y0 + h as u32 + 4));

    // The pointer is in logical points: inside at (120, 60), outside at (140, 60), where the
    // same number would still be inside if it were read as native pixels.
    let call = |x: f64, y: f64| {
        ossido
            .client()
            .call(INPUT_POINTER_MOVE, json!({"x": x, "y": y}))
            .unwrap();
    };
    call(120.0, 60.0);
    assert!(window.pointer_log().inside);
    call(140.0, 60.0);
    assert!(!window.pointer_log().inside);
    call(60.0, 40.0);
    assert!(window.pointer_log().inside);

    // A region is in logical points and comes out in native pixels.
    let reply = ossido
        .client()
        .call(
            SCREENSHOT_REGION,
            json!({"x": 32, "y": 32, "width": 100, "height": 60}),
        )
        .unwrap();
    assert_eq!(
        (reply["width"].as_u64(), reply["height"].as_u64()),
        (Some(125), Some(75))
    );
    let (_, region) = decode(&std::fs::read(reply["path"].as_str().unwrap()).unwrap());
    std::fs::remove_file(reply["path"].as_str().unwrap()).unwrap();
    assert_eq!(pixel_in(&region, 125, 62, 37), [255, 0, 0, 255]);

    // (the cost of all this is measured by `frame_cost` below)
    // Back to the plain mode: the capture is the display's size and the window 1:1 again.
    configure(&ossido, json!({"logical": null})).unwrap();
    let ((width, height), pixels) = shot(&ossido);
    assert_eq!((width, height), NATIVE);
    assert_eq!(
        pixel_in(&pixels, width, FIRST_WINDOW_AT + 50, FIRST_WINDOW_AT + 30),
        [255, 0, 0, 255]
    );
    assert_ne!(
        pixel_in(&pixels, width, FIRST_WINDOW_AT + 110, FIRST_WINDOW_AT + 30),
        [255, 0, 0, 255]
    );
}
