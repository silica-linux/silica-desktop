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

/// Bordo del fondo del Dock su `#7a7b80`, chiaro (dock.md): riga superiore, sfumatura di
/// 7 pt verso il riempimento, bordo laterale.
const DOCK_RIM: [u8; 3] = [0xbd, 0xbe, 0xc8];
const DOCK_GLOW: [[u8; 3]; 7] = [
    [0x8e, 0x8f, 0x97],
    [0x89, 0x8a, 0x91],
    [0x86, 0x87, 0x8e],
    [0x84, 0x85, 0x8c],
    [0x83, 0x84, 0x8b],
    [0x82, 0x83, 0x8a],
    [0x81, 0x82, 0x89],
];
const DOCK_SIDE: [u8; 3] = [0x3c, 0x3c, 0x40];

#[test]
fn clear_glass_edge_matches_the_dock_border() {
    let ossido = Headless::start_with("glass-edge", SHOT, &["--background", "7a7b80"]);
    let size = (300, 80);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "bordo", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 300, 80, 22.0, Material::Clear)])
        .unwrap();
    let shot = shot(&ossido, "glass-edge.png");
    // Finestra a (32, 32): colonna a metà, riga a metà.
    let (x, top, bottom, mid) = (182, 32, 111, 72);
    let mut worst = 0;
    let mut check = |at: (u32, u32), expected: [u8; 3], what: &str| {
        let got = rgb(&shot, at.0, at.1);
        let diff = got
            .iter()
            .zip(expected)
            .map(|(g, e)| g.abs_diff(e))
            .max()
            .unwrap_or(0);
        eprintln!("{what}: {got:?} (riferimento {expected:?})");
        worst = worst.max(diff);
    };
    check((x, top), DOCK_RIM, "riga superiore");
    check((x, bottom), DOCK_RIM, "riga inferiore");
    for (row, expected) in DOCK_GLOW.iter().enumerate() {
        check((x, top + 1 + row as u32), *expected, "sfumatura sopra");
        check((x, bottom - 1 - row as u32), *expected, "sfumatura sotto");
    }
    check((32, mid), DOCK_SIDE, "bordo sinistro");
    check((331, mid), DOCK_SIDE, "bordo destro");
    assert!(worst <= 3, "bordo lontano dal Dock: {worst} livelli");
}

#[test]
fn clear_glass_refracts_near_the_side_edges_like_the_dock() {
    let ossido = Headless::start_with("glass-refraction", SHOT, &[]);
    // Strisce da 12 px dietro (nero per (x + 6) mod 24 < 12), finestra a (32, 32): bordi a
    // x 26 + 12k, uno a 10 pt dal bordo del vetro, dove lo spostamento è massimo.
    let back = (500, 200);
    let mut pixels = Vec::new();
    for _ in 0..back.1 {
        for x in 0..back.0 {
            let level = if (x + 6) % 24 < 12 { 0 } else { 255 };
            pixels.extend_from_slice(&[level, level, level, 255]);
        }
    }
    let _stripes = TestWindow::open_pixels(&ossido.wayland, "strisce", back, &pixels);
    let size = (300, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "vetro", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 300, 100, 0.0, Material::Clear)])
        .unwrap();
    let shot = shot(&ossido, "glass-refraction.png");
    // Vetro da x 64; bordi delle strisce a 26 + 12k. Attraversamenti del livello medio
    // sulla riga a metà del vetro (y 114), interpolati.
    let row: Vec<f64> = (64..364)
        .map(|x| f64::from(rgb(&shot, x, 114)[1]))
        .collect();
    let mid = (row.iter().cloned().fold(f64::MAX, f64::min)
        + row.iter().cloned().fold(0.0, f64::max))
        / 2.0;
    let mut near = 0.0_f64;
    let mut far = 0.0_f64;
    for i in 3..row.len() / 2 {
        let (a, b) = (row[i - 1], row[i]);
        if (a - mid) * (b - mid) > 0.0 || a == b {
            continue;
        }
        // Posizione del pixel (centro) dove si attraversa il livello medio.
        let crossing = 64.0 + (i - 1) as f64 + 0.5 + (mid - a) / (b - a);
        let distance = crossing - 64.0;
        // Lo sfondo campionato è più verso l'interno: il bordo vero è il primo dopo.
        let edge = 26.0 + 12.0 * ((crossing - 26.0) / 12.0 - 1e-6).ceil();
        let shift = edge - crossing;
        eprintln!("attraversamento a {distance:.1} pt dal bordo: spostamento {shift:.2} pt");
        if (3.0..=12.0).contains(&distance) {
            near = near.max(shift);
        }
        if distance >= 28.0 {
            far = far.max(shift.min(12.0 - shift));
        }
    }
    eprintln!("massimo vicino al bordo {near:.2} pt, lontano {far:.2} pt");
    // Dock: ~5 pt a 6 pt dal bordo sinistro, nulla oltre ~24 pt (rumore ±0,5–1).
    assert!(
        (3.5..=6.5).contains(&near),
        "rifrazione vicino al bordo: {near:.2} pt"
    );
    assert!(far <= 1.0, "rifrazione lontano dal bordo: {far:.2} pt");
}

#[test]
fn glass_quality_levels_choose_what_the_glass_sees() {
    let ossido = Headless::start_with("glass-quality", SHOT, &[]);
    let back = (400, 200);
    let _stripes = TestWindow::open_pixels(&ossido.wayland, "strisce", back, &stripes(back));
    let size = (200, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "vetro", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Clear)])
        .unwrap();
    let mut client = ossido.client();
    // Escursione del verde sulla riga a metà del vetro (x 74–254, lontano dai bordi).
    let mut spread = |quality: &str| {
        let resolved = client
            .call(
                silica_control::commands::UI_GLASS_QUALITY,
                serde_json::json!({ "quality": quality }),
            )
            .unwrap();
        let shot = shot(&ossido, &format!("glass-{quality}.png"));
        let row: Vec<u8> = (74..254).map(|x| rgb(&shot, x, 114)[1]).collect();
        let spread = row.iter().max().unwrap() - row.iter().min().unwrap();
        eprintln!("{quality} ({resolved}): escursione {spread}");
        (
            resolved["quality"].as_str().unwrap_or_default().to_owned(),
            spread,
        )
    };
    // Pieno: le strisce sfocate restano (nero → 39, bianco → 228).
    assert!(spread("full").1 > 150);
    // Smerigliato: vede solo il desktop, non la finestra a strisce; opaco: niente sfondo.
    assert!(spread("frosted").1 <= 2);
    assert!(spread("opaque").1 <= 2);
    // Su llvmpipe (headless) l'automatico è smerigliato.
    assert_eq!(spread("auto").0, "frosted");
}

#[test]
fn regular_glass_is_the_liquid_lens_of_the_control_center() {
    // Riempimento sul grigio di riferimento: i moduli del Centro di controllo, #6a6b70.
    let ossido = Headless::start_with("glass-liquid-fill", SHOT, &["--background", "7a7b80"]);
    let size = (200, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "liquido", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Regular)])
        .unwrap();
    let fill = rgb(&shot(&ossido, "glass-liquid-fill.png"), 132, 82);
    eprintln!("riempimento: {fill:?} (riferimento [106, 107, 112])");
    for (got, want) in fill.iter().zip([0x6a_u8, 0x6b, 0x70]) {
        assert!(got.abs_diff(want) <= 3, "riempimento {fill:?}");
    }
    drop(glass);
    drop(ossido);

    // Lente: strisce da 12 px dietro (bordi a x 32 + 12k), vetro da x 64 a 363.
    let ossido = Headless::start_with("glass-liquid-lens", SHOT, &[]);
    let back = (500, 200);
    let mut pixels = Vec::new();
    for _ in 0..back.1 {
        for x in 0..back.0 {
            let level = if x % 24 < 12 { 0 } else { 255 };
            pixels.extend_from_slice(&[level, level, level, 255]);
        }
    }
    let _stripes = TestWindow::open_pixels(&ossido.wayland, "strisce", back, &pixels);
    let size = (300, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "liquido", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 300, 100, 0.0, Material::Regular)])
        .unwrap();
    let shot = shot(&ossido, "glass-liquid-lens.png");
    let row: Vec<f64> = (64..364)
        .map(|x| f64::from(rgb(&shot, x, 114)[1]))
        .collect();
    let low = row.iter().cloned().fold(f64::MAX, f64::min);
    let high = row.iter().cloned().fold(0.0, f64::max);
    let mid = (low + high) / 2.0;
    // Attraversamenti del livello medio nella metà sinistra, a distanza t dal bordo.
    let mut crossings = Vec::new();
    for i in 1..row.len() / 2 {
        let (a, b) = (row[i - 1], row[i]);
        if (a - mid) * (b - mid) <= 0.0 && a != b {
            crossings.push((i - 1) as f64 + 0.5 + (mid - a) / (b - a));
        }
    }
    // Oltre ~30 pt la lente non agisce: l'ultimo attraversamento cade su un bordo vero, e
    // la curva è monotona, quindi i precedenti cadono sui bordi prima, uno per volta.
    let last = *crossings.last().unwrap();
    let last_edge = (32.0 + 12.0 * ((64.0 + last - 32.0) / 12.0).round()) - 64.0;
    let count = crossings.len();
    // Curva misurata ai lati dei moduli (glass.md): (distanza dal bordo, sorgente) in pt.
    let measured = [
        (0.0, -25.0),
        (6.0, -4.0),
        (10.0, 14.2),
        (24.0, 25.5),
        (30.0, 30.0),
    ];
    let reference = |t: f64| match measured.windows(2).find(|w| t <= w[1].0) {
        Some(w) => w[0].1 + (w[1].1 - w[0].1) * (t - w[0].0) / (w[1].0 - w[0].0),
        None => t,
    };
    let mut worst: f64 = 0.0;
    let mut outside = 0;
    for (k, t) in crossings.iter().enumerate() {
        let source = last_edge - 12.0 * (count - 1 - k) as f64;
        let expected = reference(*t);
        eprintln!("t {t:.1} pt → sorgente {source:.1} pt (curva misurata {expected:.1})");
        if *t <= 30.0 {
            worst = worst.max((source - expected).abs());
        }
        if *t < 6.0 && source < -2.0 {
            outside += 1;
        }
    }
    // Incertezza della misura nella fascia esterna: ±3 pt.
    assert!(
        worst <= 3.0,
        "lente lontana dalla curva misurata: {worst:.1} pt"
    );
    assert!(
        outside >= 1,
        "la lente non preleva lo sfondo fuori dal bordo"
    );
}

#[test]
fn nearby_regions_of_the_same_glass_merge_like_drops() {
    let ossido = Headless::start_with("glass-merge", SHOT, &["--background", "7a7b80"]);
    let size = (400, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "gocce", size, &transparent(size));
    let background = [0x7a_u8, 0x7b, 0x80];
    let differs = |shot: &[u8], x: u32, y: u32| {
        rgb(shot, x, y)
            .iter()
            .zip(background)
            .any(|(got, bg)| got.abs_diff(bg) > 6)
    };
    // Due capsule a 4 pt l'una dall'altra: il punto in mezzo diventa vetro (finestra a
    // (32, 32): capsule a x 32–131 e 136–235, spazio centrato a x 134, y 82).
    glass
        .set_glass(&[
            (0, 20, 100, 60, 30.0, Material::Regular),
            (104, 20, 100, 60, 30.0, Material::Regular),
        ])
        .unwrap();
    let joined = shot(&ossido, "glass-merge.png");
    assert!(
        differs(&joined, 134, 82),
        "le capsule vicine non si fondono"
    );

    // A 10 pt (i pulsanti di Spotlight dal pannello) restano separate.
    glass
        .set_glass(&[
            (0, 20, 100, 60, 30.0, Material::Regular),
            (110, 20, 100, 60, 30.0, Material::Regular),
        ])
        .unwrap();
    let near = shot(&ossido, "glass-near.png");
    assert!(!differs(&near, 136, 82), "a 10 pt le capsule si fondono");

    // Lontane (60 pt): resta il fondo.
    glass
        .set_glass(&[
            (0, 20, 100, 60, 30.0, Material::Regular),
            (160, 20, 100, 60, 30.0, Material::Regular),
        ])
        .unwrap();
    let apart = shot(&ossido, "glass-apart.png");
    assert!(!differs(&apart, 162, 82), "le capsule lontane si fondono");

    // Materiali diversi non si fondono, anche vicini.
    glass
        .set_glass(&[
            (0, 20, 100, 60, 30.0, Material::Regular),
            (104, 20, 100, 60, 30.0, Material::Clear),
        ])
        .unwrap();
    let mixed = shot(&ossido, "glass-mixed.png");
    assert!(!differs(&mixed, 134, 82), "materiali diversi si fondono");
}

#[test]
fn notification_glass_matches_the_isolated_banner() {
    // Il banner isolato sul grigio di riferimento: #bfc0c5 in alto, #c2c3c8 in basso
    // (notifications.md, "Banner · materiale su grigio").
    let ossido = Headless::start_with("glass-notification", SHOT, &["--background", "7a7b80"]);
    let size = (344, 91);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "banner", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 344, 91, 20.0, Material::Notification)])
        .unwrap();
    let fill = rgb(&shot(&ossido, "glass-notification.png"), 32 + 172, 32 + 45);
    eprintln!("riempimento: {fill:?} (riferimento [192, 193, 198])");
    for (got, want) in fill.iter().zip([0xc0_u8, 0xc1, 0xc6]) {
        assert!(got.abs_diff(want) <= 3, "riempimento {fill:?}");
    }

    // Due carte a 6 pt (le carte del Centro, con il vetro più largo di 1 pt per lato)
    // restano separate: nello spazio fra loro c'è lo sfondo.
    glass
        .set_glass(&[
            (0, 0, 344, 40, 20.0, Material::Notification),
            (0, 46, 344, 40, 20.0, Material::Notification),
        ])
        .unwrap();
    let gap = rgb(
        &shot(&ossido, "glass-notification-gap.png"),
        32 + 172,
        32 + 43,
    );
    assert_eq!(gap, [0x7a, 0x7b, 0x80], "fra le carte: {gap:?}");
    drop(glass);

    // È della versione 2: un client legato alla 1 che lo chiede viene disconnesso.
    common::client::GLASS_VERSION.with(|v| v.set(1));
    let mut old = TestWindow::open_pixels(&ossido.wayland, "vecchio", size, &transparent(size));
    assert!(
        old.set_glass(&[(0, 0, 344, 91, 20.0, Material::Notification)])
            .is_err()
    );
    common::client::GLASS_VERSION.with(|v| v.set(2));
}

#[test]
fn every_region_of_a_group_has_its_rim_on_its_own_edge() {
    // Come i moduli del Centro di controllo: regioni dello stesso materiale lontane fra
    // loro, capsule e rettangoli arrotondati. La riga luminosa sta sulla prima riga di
    // ciascuna regione, sopra e sotto, anche lontano dal centro del gruppo.
    let ossido = Headless::start_with("glass-group-rim", SHOT, &["--background", "7a7b80"]);
    let size = (292, 330);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "pila", size, &transparent(size));
    // La griglia del Centro di controllo: moduli a 12 pt l'uno dall'altro, con il vetro
    // largo 1 pt in più per lato (10 pt fra i vetri in orizzontale).
    let regions = [
        (0, 0, 142, 64, 32.0, Material::Regular),
        (152, 0, 142, 140, 35.0, Material::Regular),
        (0, 76, 142, 64, 32.0, Material::Regular),
        (0, 152, 142, 64, 32.0, Material::Regular),
        (0, 228, 294, 64, 28.0, Material::Regular),
    ];
    glass.set_glass(&regions).unwrap();
    let shot = shot(&ossido, "glass-group-rim.png");
    let brightest =
        |x: u32, y0: u32, y1: u32| (y0..y1).max_by_key(|&y| rgb(&shot, x, y)[0]).unwrap();
    for (x, y, w, h, _, _) in regions {
        let column = 32 + x as u32 + w as u32 / 2;
        let top = 32 + y as u32;
        let bottom = top + h as u32 - 1;
        assert_eq!(
            brightest(column, top - 3, top + 3),
            top,
            "regione a y {y}: riga superiore"
        );
        assert_eq!(
            brightest(column, bottom - 3, bottom + 4),
            bottom,
            "regione a y {y}: riga inferiore"
        );
        // A 12 pt i moduli non si toccano: la riga subito fuori è sfondo (nel
        // riferimento `#6e6f73` sotto il Wi-Fi, nessun ponte verso il Bluetooth).
        let outside = rgb(&shot, column, bottom + 1);
        assert!(
            outside
                .iter()
                .zip([0x7a_u8, 0x7b, 0x80])
                .all(|(a, b)| a.abs_diff(b) <= 4),
            "regione a y {y}: sotto il bordo {outside:?}"
        );
    }
    // Fra il primo modulo e quello multimediale: sfondo subito a destra della riga scura.
    let beside = rgb(&shot, 32 + 142, 32 + 32);
    assert!(
        beside
            .iter()
            .zip([0x7a_u8, 0x7b, 0x80])
            .all(|(a, b)| a.abs_diff(b) <= 4),
        "accanto al bordo laterale {beside:?}"
    );
}

#[test]
fn translucent_glass_fades_with_its_panel() {
    // A metà opacità il vetro delle notifiche sul grigio sta a metà fra lo sfondo (#7a7b80)
    // e il riempimento pieno (#c0c1c6): un pannello in dissolvenza porta con sé il vetro.
    let ossido = Headless::start_with("glass-translucent", SHOT, &["--background", "7a7b80"]);
    let size = (344, 91);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "banner", size, &transparent(size));
    glass
        .set_translucent_glass(&[((0, 0, 344, 91, 20.0, Material::Notification), 0.5)])
        .unwrap();
    let fill = rgb(&shot(&ossido, "glass-translucent.png"), 32 + 172, 32 + 45);
    for (got, want) in fill.iter().zip([157_u8, 158, 163]) {
        assert!(got.abs_diff(want) <= 4, "metà opacità: {fill:?}");
    }
    // Trasparente del tutto: resta lo sfondo.
    glass
        .set_translucent_glass(&[((0, 0, 344, 91, 20.0, Material::Notification), 0.0)])
        .unwrap();
    let fill = rgb(&shot(&ossido, "glass-translucent-0.png"), 32 + 172, 32 + 45);
    assert_eq!(fill, [0x7a, 0x7b, 0x80]);
    // Fuori da 0–1: il client viene disconnesso, Ossido continua.
    assert!(
        glass
            .set_translucent_glass(&[((0, 0, 344, 91, 20.0, Material::Notification), 1.5)])
            .is_err()
    );
    let _alive = TestWindow::open(&ossido.wayland, "dopo");
}
