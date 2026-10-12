//! `silica-glass-v1`: regioni dichiarate dal client, applicate al commit, controllate
//! come input ostile.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::Headless;
use common::client::TestWindow;
use serde_json::Value;
use silica_control::commands::{GLASS_LIST, WINDOW_LIST};
use silica_protocols::glass::client::silica_glass_v1::Material;

fn glass_list(ossido: &Headless) -> Value {
    ossido.client().call(GLASS_LIST, Value::Null).unwrap()
}

#[test]
fn regions_apply_on_commit_and_are_listed() {
    let ossido = Headless::start("glass-list");
    let mut window = TestWindow::open(&ossido.wayland, "vetro");
    assert_eq!(glass_list(&ossido), serde_json::json!([]));

    window
        .set_glass(&[
            (10, 10, 40, 20, 8.0, Material::Clear),
            (0, 30, 100, 30, 0.0, Material::Thick),
        ])
        .unwrap();
    let list = glass_list(&ossido);
    let regions = &list[0]["regions"];
    assert_eq!(list[0]["surface"], "window 1", "{list}");
    assert_eq!(regions[0]["width"], 40);
    assert_eq!(regions[0]["radius"], 8.0);
    assert_eq!(regions[0]["material"], "clear");
    assert_eq!(regions[1]["material"], "thick");

    // Senza regioni la superficie esce dall'elenco.
    window.set_glass(&[]).unwrap();
    assert_eq!(glass_list(&ossido), serde_json::json!([]));
}

#[test]
fn invalid_regions_disconnect_the_client_not_ossido() {
    let ossido = Headless::start("glass-bad");
    let mut bad = TestWindow::open(&ossido.wayland, "cattivo");
    let err = bad
        .set_glass(&[(0, 0, -5, 10, 0.0, Material::Clear)])
        .unwrap_err();
    assert!(!err.is_empty());

    let mut too_many = TestWindow::open(&ossido.wayland, "troppe");
    let regions = vec![(0, 0, 1, 1, 0.0, Material::Clear); 65];
    assert!(too_many.set_glass(&regions).is_err());

    // Ossido è vivo e serve ancora i client.
    let mut good = TestWindow::open(&ossido.wayland, "buono");
    good.set_glass(&[(0, 0, 10, 10, 0.0, Material::Clear)])
        .unwrap();
    let windows = ossido.client().call(WINDOW_LIST, Value::Null).unwrap();
    assert!(windows.as_array().is_some_and(|list| !list.is_empty()));
    assert_eq!(glass_list(&ossido).as_array().map(Vec::len), Some(1));
}
