//! Window effects compared with the measurements of macOS 27 (`plan/reference/windows.md`),
//! on headless Ossido at scale 1 like the recordings.

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

/// Missing pixels (255 − alpha, in whole pixels) for rows 0…23 of the top-left corner of a
/// 900 × 600 window (`windows.md`, "Windows · shape and corners").
const REFERENCE_MISSING: [f64; 24] = [
    16.00, 10.05, 7.90, 6.42, 5.26, 4.28, 3.47, 2.73, 2.14, 1.73, 1.37, 0.95, 0.85, 0.72, 0.59,
    0.47, 0.37, 0.30, 0.23, 0.18, 0.14, 0.10, 0.06, 0.03,
];

#[test]
fn window_corners_follow_the_reference() {
    let ossido = Headless::start("corners");
    let _window = TestWindow::open_sized(&ossido.wayland, "corners", (200, 120));
    let shot = screenshot(&ossido);
    // With OSSIDO_SHOTS=<folder> the screenshot is kept, to look at it.
    if let Some(dir) = std::env::var_os("OSSIDO_SHOTS") {
        std::fs::copy(
            ossido.dir.join("shot.png"),
            std::path::Path::new(&dir).join("corners.png"),
        )
        .unwrap();
    }
    let at = FIRST_WINDOW_AT;
    // The red channel goes from the background (read in the output's corner) to 255 in the window.
    let background = f64::from(pixel(&shot, 0, 0)[0]);
    let coverage =
        |x: u32, y: u32| (f64::from(pixel(&shot, x, y)[0]) - background) / (255.0 - background);

    let mut errors = Vec::new();
    for (row, reference) in REFERENCE_MISSING.iter().enumerate() {
        let y = at + row as u32;
        let missing: f64 = (0..40).map(|dx| 1.0 - coverage(at + dx, y)).sum();
        errors.push(missing - reference);
        eprintln!("row {row}: {missing:.2} (reference {reference:.2})");
    }
    let worst = errors.iter().fold(0.0_f64, |m, e| m.max(e.abs()));
    let rms = (errors.iter().map(|e| e * e).sum::<f64>() / errors.len() as f64).sqrt();
    eprintln!("maximum error {worst:.2} px, rms {rms:.2} px");
    assert!(
        worst <= 1.0,
        "corner far from the reference: {worst:.2} px on a row"
    );
    assert!(rms <= 0.5, "corner far from the reference: rms {rms:.2} px");

    // The four corners are the same: the apex is background, the center is window.
    let (w, h) = (200, 120);
    for (x, y) in [
        (at, at),
        (at + w - 1, at),
        (at, at + h - 1),
        (at + w - 1, at + h - 1),
    ] {
        assert!(coverage(x, y) < 0.05, "corner not cut at ({x}, {y})");
    }
    assert_eq!(pixel(&shot, at + w / 2, at + h / 2), [255, 0, 0, 255]);
}

/// Background of the reference recordings (`setup.md`).
const REFERENCE_GRAY: &str = "7a7b80";

/// Side profile of the shadow, alpha 0–255 for d = 2, 4, …, 20, 24, 28, 32, 40, 48 pt from the
/// outer edge of the ring (`windows.md`, "Windows · shadow", light appearance).
const PROFILE_D: [u32; 15] = [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 32, 40, 48];
const ACTIVE_PROFILE: [f64; 15] = [
    48., 44., 40., 36., 32., 30., 26., 22., 20., 16., 12., 7., 6., 2., 0.,
];
const INACTIVE_PROFILE: [f64; 15] = [
    24., 18., 14., 10., 6., 4., 2., 2., 1., 0., 0., 0., 0., 0., 0.,
];

/// Alpha (0–255) of a black shadow over the reference gray, from the green channel.
fn shadow_alpha(shot: &[u8], x: u32, y: u32) -> f64 {
    let gray = f64::from(0x7b_u8);
    255.0 * (1.0 - f64::from(pixel_in(shot, 640, x, y)[1]) / gray)
}

/// Compares the profile to the right of a window (right edge at `right`, row `y`).
fn compare_profile(shot: &[u8], right: u32, y: u32, reference: &[f64; 15], name: &str) {
    let mut worst = 0.0_f64;
    for (d, expected) in PROFILE_D.iter().zip(reference) {
        let measured = shadow_alpha(shot, right + d, y);
        eprintln!("{name} d={d}: {measured:.1} (reference {expected})");
        worst = worst.max((measured - expected).abs());
    }
    eprintln!("{name}: maximum error {worst:.1} levels");
    // The 8-bit green on 123 resolves the alpha to ~2 levels; the reference is ±1.
    assert!(
        worst <= 6.0,
        "shadow {name} far from the reference: {worst:.1} levels"
    );
}

#[test]
fn window_shadows_follow_the_reference() {
    let ossido = Headless::start_with("shadow", (640, 400), &["--background", REFERENCE_GRAY]);
    let _first = TestWindow::open_sized(&ossido.wayland, "first", (200, 120));
    let _second = TestWindow::open_sized(&ossido.wayland, "second", (200, 120));
    // The second (active) goes to the right; the first stays at (32, 32), inactive.
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

    // Active: right edge at x 530, half height y 180.
    compare_profile(&shot, 530, 180, &ACTIVE_PROFILE, "active");
    // Inactive: right edge at x 232, half height y 92 (the second is further right).
    compare_profile(&shot, 232, 92, &INACTIVE_PROFILE, "inactive");

    // Contact ring: active 81 on the sides, inactive 59 (±6).
    let ring_active = shadow_alpha(&shot, 530, 180);
    let ring_inactive = shadow_alpha(&shot, 232, 92);
    eprintln!("ring: active {ring_active:.1} (81), inactive {ring_inactive:.1} (59)");
    assert!(
        (ring_active - 81.0).abs() <= 6.0,
        "active ring {ring_active:.1}"
    );
    assert!(
        (ring_inactive - 59.0).abs() <= 6.0,
        "inactive ring {ring_inactive:.1}"
    );

    // Vertical shift: more shadow below than above (active, at half width).
    let above = shadow_alpha(&shot, 430, 120 - 3);
    let below = shadow_alpha(&shot, 430, 240 + 2);
    eprintln!("active: above {above:.1}, below {below:.1}");
    assert!(
        below > above + 20.0,
        "shadow not shifted down: above {above:.1}, below {below:.1}"
    );
}
