//! The frame cost of a busy desktop, plain and in scaled modes (#216). A measurement, not a
//! check: `cargo test -p ossido --release --test hidpi_cost -- --ignored --nocapture`. The
//! times are Ossido's own debug lines, "screenshot drawn in N ms" (the draw, the reduction and
//! the read back of a full frame, without the PNG). Numbers for `docs/displays.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::Headless;
use common::client::TestWindow;
use serde_json::json;
use silica_control::commands::SCREENSHOT_SCREEN;
use silica_protocols::glass::client::silica_glass_v1::Material;

#[test]
#[ignore = "a measurement"]
fn frame_cost() {
    let plain = (1920, 1200);
    for (name, native, extra, logical) in [
        ("native 1920x1200", plain, &[][..], None),
        ("looks like 1680x1050", plain, &[][..], Some((1680, 1050))),
        ("looks like 1536x960", plain, &[][..], Some((1536, 960))),
        ("looks like 1280x800", plain, &[][..], Some((1280, 800))),
        // The scene alone at 2x, with no reduction: the same drawing as "looks like 1536x960".
        (
            "2x scene only, 3072x1920",
            (3072, 1920),
            &["--scale", "2"][..],
            None,
        ),
    ] {
        let ossido =
            Headless::start_with_env("hidpi-cost", native, extra, &[("RUST_LOG", "ossido=debug")]);
        // Three windows, two of them with glass over their top 100 points.
        let mut windows = Vec::new();
        for i in 0..3 {
            let mut window = TestWindow::open_decorated(&ossido.wayland, "cost", (600, 400));
            if i < 2 {
                window
                    .set_glass(&[(0, 0, 600, 100, 20.0, Material::Regular)])
                    .unwrap();
            }
            windows.push(window);
        }
        let mut client = ossido.client();
        if let Some((width, height)) = logical {
            client
                .call(
                    "output.configure",
                    json!({"logical": {"width": width, "height": height}}),
                )
                .unwrap();
        }
        eprintln!("== {name}");
        for _ in 0..8 {
            let reply = client
                .call(SCREENSHOT_SCREEN, serde_json::Value::Null)
                .unwrap();
            std::fs::remove_file(reply["path"].as_str().unwrap()).unwrap();
        }
    }
}
