//! Window opening and closing animations (`plan/reference/windows.md`), slowed down with
//! `ui.animation_scale` to read them in the screenshots.

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

/// Width of the red part (the window) on row `y`.
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
    // Durations ×100: the opening lasts 10 s, the first screenshot catches it at the start.
    let ossido = Headless::start_with(
        "anim-open",
        (common::WIDTH, common::HEIGHT),
        &["--animation-scale", "100"],
    );
    let _window = TestWindow::open_sized(&ossido.wayland, "opens", (200, 120));
    let y = FIRST_WINDOW_AT + 60;
    let opening = red_width(&screenshot(&ossido), y);
    eprintln!("opening: {opening} px of 200");
    // Scale 0.98 → 196 px, at most a few hundredths of progress more.
    assert!((195..=198).contains(&opening), "opening: {opening} px");

    set_speed(&ossido, 0.0);
    assert_eq!(red_width(&screenshot(&ossido), y), 200);
}

#[test]
fn closed_windows_fade_out() {
    let ossido = Headless::start("anim-close");
    let mut window = TestWindow::open_sized(&ossido.wayland, "closes", (200, 120));
    let (x, y) = (FIRST_WINDOW_AT + 100, FIRST_WINDOW_AT + 60);
    assert_eq!(pixel(&screenshot(&ossido), x, y), [255, 0, 0, 255]);

    // Slow closing: right after, the snapshot is still almost opaque.
    set_speed(&ossido, 100.0);
    window.destroy_toplevel();
    let [r, ..] = pixel(&screenshot(&ossido), x, y);
    eprintln!("closing: red {r}");
    assert!(
        r > 200,
        "the closed window does not fade out from its last image: red {r}"
    );

    // At the end of the animation the background stays.
    set_speed(&ossido, 0.0);
    common::assert_clear_color(&screenshot(&ossido), x, y);
}
