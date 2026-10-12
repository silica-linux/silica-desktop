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

/// Pixel trasparenti: dove il client lascia vedere il vetro.
fn transparent(size: (u32, u32)) -> Vec<u8> {
    vec![0; (size.0 * size.1 * 4) as usize]
}

/// Strisce verticali bianche e nere da 48 px, come lo sfondo `stripes-v` delle riprese
/// (nero per x mod 96 < 48).
fn stripes(size: (u32, u32)) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for _ in 0..size.1 {
        for x in 0..size.0 {
            let level = if x % 96 < 48 { 0 } else { 255 };
            pixels.extend_from_slice(&[level, level, level, 255]);
        }
    }
    pixels
}

const SHOT: (u32, u32) = (640, 400);

fn shot(ossido: &Headless, name: &str) -> Vec<u8> {
    let shot = common::screenshot(ossido);
    if let Some(dir) = std::env::var_os("OSSIDO_SHOTS") {
        std::fs::copy(
            ossido.dir.join("shot.png"),
            std::path::Path::new(&dir).join(name),
        )
        .unwrap();
    }
    shot
}

fn rgb(shot: &[u8], x: u32, y: u32) -> [u8; 3] {
    let [r, g, b, _] = common::pixel_in(shot, SHOT.0, x, y);
    [r, g, b]
}

#[test]
fn materials_on_the_reference_gray_match_the_measured_fill() {
    let ossido = Headless::start_with("glass-gray", SHOT, &["--background", "7a7b80"]);
    let size = (200, 100);
    let mut clear = TestWindow::open_pixels(&ossido.wayland, "chiaro", size, &transparent(size));
    clear
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Clear)])
        .unwrap();
    let mut thick = TestWindow::open_pixels(&ossido.wayland, "spesso", size, &transparent(size));
    thick
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Thick)])
        .unwrap();
    // Le due finestre: la prima a (32, 32), la seconda a (64, 64) sopra.
    ossido
        .client()
        .call(
            silica_control::commands::WINDOW_MOVE,
            serde_json::json!({"x": 330, "y": 200}),
        )
        .unwrap();
    let shot = shot(&ossido, "glass-gray.png");
    // Riempimento su #7a7b80 (glass.md): Dock chiaro #818289, menu chiaro #c7c8cd.
    let expect = |at: (u32, u32), expected: [u8; 3], name: &str| {
        let got = rgb(&shot, at.0, at.1);
        let worst = got
            .iter()
            .zip(expected)
            .map(|(g, e)| g.abs_diff(e))
            .max()
            .unwrap_or(0);
        eprintln!("{name}: {got:?} (riferimento {expected:?})");
        assert!(worst <= 3, "{name}: {got:?}, riferimento {expected:?}");
    };
    expect((132, 82), [0x81, 0x82, 0x89], "vetro chiaro");
    expect((430, 250), [0xc7, 0xc8, 0xcd], "vetro spesso");
}

/// Profilo di un bordo di striscia (nero → bianco) dentro il fondo del Dock, chiaro, riga
/// 1366 (glass.md, "Vetro · Dock · Sfocatura"), 1 pt per valore.
const DOCK_EDGE: [f64; 20] = [
    39., 40., 41., 42., 45., 50., 55., 59., 66., 102., 166., 202., 208., 213., 218., 223., 226.,
    227., 228., 229.,
];

#[test]
fn clear_glass_blurs_a_stripe_edge_like_the_dock() {
    let ossido = Headless::start_with("glass-blur", SHOT, &[]);
    let back = (400, 200);
    let _stripes = TestWindow::open_pixels(&ossido.wayland, "strisce", back, &stripes(back));
    let size = (200, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "vetro", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Clear)])
        .unwrap();
    let shot = shot(&ossido, "glass-blur.png");
    // Strisce a x 32 + 96k (nero) e 80 + 96k (bianco); vetro su x 64–263, y 64–163.
    // Bordo nero → bianco a x 176; il profilo misurato si allinea al passo più netto.
    let row: Vec<f64> = (150..202)
        .map(|x| f64::from(rgb(&shot, x, 114)[1]))
        .collect();
    let mut best = (f64::MAX, 0);
    for start in 0..row.len() - DOCK_EDGE.len() {
        let rms = (row[start..start + DOCK_EDGE.len()]
            .iter()
            .zip(DOCK_EDGE)
            .map(|(got, want)| (got - want).powi(2))
            .sum::<f64>()
            / DOCK_EDGE.len() as f64)
            .sqrt();
        if rms < best.0 {
            best = (rms, start);
        }
    }
    let (rms, start) = best;
    eprintln!("profilo: {:?}", &row[start..start + DOCK_EDGE.len()]);
    eprintln!("riferimento: {DOCK_EDGE:?}, rms {rms:.2}");
    // L'adattamento del modello sulle riprese ha rmse 3,4: si accetta il doppio.
    assert!(rms <= 7.0, "bordo sfocato lontano dal Dock: rms {rms:.2}");
}
