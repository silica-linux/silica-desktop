//! Un vero client Wayland apre una finestra xdg-shell su Ossido headless,
//! e la finestra compare nello screenshot del canale di controllo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::client::{FIRST_WINDOW_AT, TestWindow, WINDOW};
use common::{Headless, assert_outside_window, pixel, screenshot};

#[test]
fn xdg_toplevel_appears_in_screenshot() {
    let ossido = Headless::start("client");
    let _window = TestWindow::open(&ossido.wayland, "test");
    let shot = screenshot(&ossido);

    let (w, h) = WINDOW;
    let at = FIRST_WINDOW_AT;
    // Centro e metà dei lati: gli angoli sono tagliati (window_corners_follow_the_reference).
    for (x, y) in [
        (at + w / 2, at + h / 2),
        (at, at + h / 2),
        (at + w - 1, at + h / 2),
        (at + w / 2, at),
        (at + w / 2, at + h - 1),
    ] {
        assert_eq!(
            pixel(&shot, x, y),
            [255, 0, 0, 255],
            "finestra assente in ({x}, {y})"
        );
    }
    assert_outside_window(&shot, at - 1, at + h / 2);
    assert_outside_window(&shot, at + w, at + h / 2);
}
