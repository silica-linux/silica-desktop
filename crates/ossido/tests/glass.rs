//! `silica-glass-v1`: regions declared by the client, applied on commit, checked as hostile
//! input.

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
use silica_protocols::glass::client::silica_glass_v1::{Material, Profile, Shadow, Side};

fn glass_list(ossido: &Headless) -> Value {
    ossido.client().call(GLASS_LIST, Value::Null).unwrap()
}

#[test]
fn regions_apply_on_commit_and_are_listed() {
    let ossido = Headless::start("glass-list");
    let mut window = TestWindow::open(&ossido.wayland, "glass");
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

    // Without regions the surface leaves the list.
    window.set_glass(&[]).unwrap();
    assert_eq!(glass_list(&ossido), serde_json::json!([]));
}

#[test]
fn invalid_regions_disconnect_the_client_not_ossido() {
    let ossido = Headless::start("glass-bad");
    let mut bad = TestWindow::open(&ossido.wayland, "bad");
    let err = bad
        .set_glass(&[(0, 0, -5, 10, 0.0, Material::Clear)])
        .unwrap_err();
    assert!(!err.is_empty());

    let mut too_many = TestWindow::open(&ossido.wayland, "too-many");
    let regions = vec![(0, 0, 1, 1, 0.0, Material::Clear); 65];
    assert!(too_many.set_glass(&regions).is_err());

    // Ossido is alive and still serves the clients.
    let mut good = TestWindow::open(&ossido.wayland, "good");
    good.set_glass(&[(0, 0, 10, 10, 0.0, Material::Clear)])
        .unwrap();
    let windows = ossido.client().call(WINDOW_LIST, Value::Null).unwrap();
    assert!(windows.as_array().is_some_and(|list| !list.is_empty()));
    assert_eq!(glass_list(&ossido).as_array().map(Vec::len), Some(1));
}

/// Transparent pixels: where the client lets the glass show.
fn transparent(size: (u32, u32)) -> Vec<u8> {
    vec![0; (size.0 * size.1 * 4) as usize]
}

/// White and black vertical stripes 48 px wide, like the `stripes-v` background of the
/// recordings (black for x mod 96 < 48).
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
    let mut clear = TestWindow::open_pixels(&ossido.wayland, "clear", size, &transparent(size));
    clear
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Clear)])
        .unwrap();
    let mut thick = TestWindow::open_pixels(&ossido.wayland, "thick", size, &transparent(size));
    thick
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Thick)])
        .unwrap();
    // The two windows: the first at (32, 32), the second at (64, 64) above.
    ossido
        .client()
        .call(
            silica_control::commands::WINDOW_MOVE,
            serde_json::json!({"x": 330, "y": 200}),
        )
        .unwrap();
    let shot = shot(&ossido, "glass-gray.png");
    // Fill on #7a7b80 (glass.md): light Dock #818289, light menu #c7c8cd.
    let expect = |at: (u32, u32), expected: [u8; 3], name: &str| {
        let got = rgb(&shot, at.0, at.1);
        let worst = got
            .iter()
            .zip(expected)
            .map(|(g, e)| g.abs_diff(e))
            .max()
            .unwrap_or(0);
        eprintln!("{name}: {got:?} (reference {expected:?})");
        assert!(worst <= 3, "{name}: {got:?}, reference {expected:?}");
    };
    expect((132, 82), [0x81, 0x82, 0x89], "clear glass");
    expect((430, 250), [0xc7, 0xc8, 0xcd], "thick glass");
}

/// In dark (`--appearance dark`) Dock and menu take the dark materials; the Control Center
/// modules stay almost the same (glass.md, "Comparison between the materials").
#[test]
fn dark_materials_on_the_reference_gray() {
    let ossido = Headless::start_with(
        "glass-gray-dark",
        SHOT,
        &["--background", "7a7b80", "--appearance", "dark"],
    );
    let size = (200, 100);
    let mut clear = TestWindow::open_pixels(&ossido.wayland, "clear", size, &transparent(size));
    clear
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Clear)])
        .unwrap();
    let mut thick = TestWindow::open_pixels(&ossido.wayland, "thick", size, &transparent(size));
    thick
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Thick)])
        .unwrap();
    ossido
        .client()
        .call(
            silica_control::commands::WINDOW_MOVE,
            serde_json::json!({"x": 330, "y": 200}),
        )
        .unwrap();
    let shot = shot(&ossido, "glass-gray-dark.png");
    let expect = |at: (u32, u32), expected: [u8; 3], tolerance: u8, name: &str| {
        let got = rgb(&shot, at.0, at.1);
        let worst = got
            .iter()
            .zip(expected)
            .map(|(g, e)| g.abs_diff(e))
            .max()
            .unwrap_or(0);
        eprintln!("{name}: {got:?} (reference {expected:?})");
        assert!(
            worst <= tolerance,
            "{name}: {got:?}, reference {expected:?}"
        );
    };
    // Dark Dock `#68696e`: the measured matrix (rmse 4.7) on the gray gives ~7 levels less,
    // because the dark transfer is not linear.
    expect((132, 82), [0x68, 0x69, 0x6e], 8, "clear glass, dark");
    // Dark menu `#6f7075`: the offset is recomputed for this gray.
    expect((430, 250), [0x6f, 0x70, 0x75], 3, "thick glass, dark");
}

/// A window that overrides the appearance with dark in a light session gets the dark glass
/// tones for the regions it declares (silica-window-v1 `set_appearance`).
#[test]
fn glass_of_a_dark_window_in_a_light_session_has_the_dark_tone() {
    let ossido = Headless::start_with("glass-override-dark", SHOT, &["--background", "7a7b80"]);
    let size = (200, 100);
    let mut thick = TestWindow::open_pixels(&ossido.wayland, "thick", size, &transparent(size));
    thick
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Thick)])
        .unwrap();
    thick.set_appearance(silica_protocols::window::client::silica_window_v1::Appearance::Dark);
    ossido
        .client()
        .call(
            silica_control::commands::WINDOW_MOVE,
            serde_json::json!({"x": 330, "y": 200}),
        )
        .unwrap();
    let shot = shot(&ossido, "glass-override-dark.png");
    let got = rgb(&shot, 430, 250);
    // The dark menu tone of `dark_materials_on_the_reference_gray`, not the light one.
    let expected = [0x6f, 0x70, 0x75];
    assert!(
        got.iter().zip(expected).all(|(g, e)| g.abs_diff(e) <= 3),
        "thick glass of a dark window: {got:?}, expected {expected:?}"
    );
}

/// Profile of a stripe edge (black → white) inside the Dock background, light, row 1366
/// (glass.md, "Glass · Dock · Blur"), 1 pt per value.
const DOCK_EDGE: [f64; 20] = [
    39., 40., 41., 42., 45., 50., 55., 59., 66., 102., 166., 202., 208., 213., 218., 223., 226.,
    227., 228., 229.,
];

#[test]
fn clear_glass_blurs_a_stripe_edge_like_the_dock() {
    let ossido = Headless::start_with("glass-blur", SHOT, &[]);
    let back = (400, 200);
    let _stripes = TestWindow::open_pixels(&ossido.wayland, "stripes", back, &stripes(back));
    let size = (200, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "glass", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Clear)])
        .unwrap();
    let shot = shot(&ossido, "glass-blur.png");
    // Stripes at x 32 + 96k (black) and 80 + 96k (white); glass on x 64–263, y 64–163.
    // Black → white edge at x 176; the measured profile aligns to the sharpest step.
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
    eprintln!("profile: {:?}", &row[start..start + DOCK_EDGE.len()]);
    eprintln!("reference: {DOCK_EDGE:?}, rms {rms:.2}");
    // The model fit on the recordings has rmse 3.4: twice that is accepted.
    assert!(rms <= 7.0, "blurred edge far from the Dock: rms {rms:.2}");
}

/// Edge of the Dock background on `#7a7b80`, light (dock.md): top row, 7 pt gradient
/// towards the fill, side edge.
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
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "edge", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 300, 80, 22.0, Material::Clear)])
        .unwrap();
    let shot = shot(&ossido, "glass-edge.png");
    // Window at (32, 32): column halfway, row halfway.
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
        eprintln!("{what}: {got:?} (reference {expected:?})");
        worst = worst.max(diff);
    };
    check((x, top), DOCK_RIM, "top row");
    check((x, bottom), DOCK_RIM, "bottom row");
    for (row, expected) in DOCK_GLOW.iter().enumerate() {
        check((x, top + 1 + row as u32), *expected, "gradient on top");
        check(
            (x, bottom - 1 - row as u32),
            *expected,
            "gradient at the bottom",
        );
    }
    check((32, mid), DOCK_SIDE, "left edge");
    check((331, mid), DOCK_SIDE, "right edge");
    // The measured edge offsets are neutral: the top row's slight blue tint (B +4 over R and
    // G more than the fill's) is not modelled. The profile lifts the rim row by 3 levels
    // (+64 over the grey, GLASS-PROFILES, against the capture's +61).
    assert!(worst <= 5, "edge far from the Dock: {worst} levels");
}

#[test]
fn clear_glass_refracts_near_the_side_edges_like_the_dock() {
    let ossido = Headless::start_with("glass-refraction", SHOT, &[]);
    // 12 px stripes behind (black for (x + 6) mod 24 < 12), window at (32, 32): edges at
    // x 26 + 12k, one 10 pt from the glass edge, where the displacement is largest.
    let back = (500, 200);
    let mut pixels = Vec::new();
    for _ in 0..back.1 {
        for x in 0..back.0 {
            let level = if (x + 6) % 24 < 12 { 0 } else { 255 };
            pixels.extend_from_slice(&[level, level, level, 255]);
        }
    }
    let _stripes = TestWindow::open_pixels(&ossido.wayland, "stripes", back, &pixels);
    let size = (300, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "glass", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 300, 100, 0.0, Material::Clear)])
        .unwrap();
    let shot = shot(&ossido, "glass-refraction.png");
    // Glass from x 64; stripe edges at 26 + 12k. Crossings of the mid level on the row
    // halfway through the glass (y 114), interpolated.
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
        // Position of the pixel (center) where the mid level is crossed.
        let crossing = 64.0 + (i - 1) as f64 + 0.5 + (mid - a) / (b - a);
        let distance = crossing - 64.0;
        // The sampled background is further inside: the true edge is the first one after.
        let edge = 26.0 + 12.0 * ((crossing - 26.0) / 12.0 - 1e-6).ceil();
        let shift = edge - crossing;
        eprintln!("crossing {distance:.1} pt from the edge: displacement {shift:.2} pt");
        if (3.0..=12.0).contains(&distance) {
            near = near.max(shift);
        }
        if distance >= 28.0 {
            far = far.max(shift.min(12.0 - shift));
        }
    }
    eprintln!("maximum near the edge {near:.2} pt, far {far:.2} pt");
    // Dock (GLASS-PROFILES): 2-3 pt, nothing beyond ~28 pt (noise ±0.5–1).
    assert!(
        (1.5..=3.5).contains(&near),
        "refraction near the edge: {near:.2} pt"
    );
    assert!(far <= 1.0, "refraction far from the edge: {far:.2} pt");
}

#[test]
fn glass_quality_levels_choose_what_the_glass_sees() {
    let ossido = Headless::start_with("glass-quality", SHOT, &[]);
    let back = (400, 200);
    let _stripes = TestWindow::open_pixels(&ossido.wayland, "stripes", back, &stripes(back));
    let size = (200, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "glass", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Clear)])
        .unwrap();
    let mut client = ossido.client();
    // Swing of the green on the row halfway through the glass (x 74–254, far from the edges).
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
    // Full: the blurred stripes stay (black → 39, white → 228).
    assert!(spread("full").1 > 150);
    // Frosted: sees only the desktop, not the striped window; opaque: no background.
    assert!(spread("frosted").1 <= 2);
    assert!(spread("opaque").1 <= 2);
    // On llvmpipe (headless) the automatic level is frosted.
    assert_eq!(spread("auto").0, "frosted");
}

#[test]
fn regular_glass_is_the_liquid_lens_of_the_control_center() {
    // Fill on the reference gray: the Control Center modules, #6a6b70.
    let ossido = Headless::start_with("glass-liquid-fill", SHOT, &["--background", "7a7b80"]);
    let size = (200, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "liquid", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 200, 100, 0.0, Material::Regular)])
        .unwrap();
    let fill = rgb(&shot(&ossido, "glass-liquid-fill.png"), 132, 82);
    eprintln!("fill: {fill:?} (reference [106, 107, 112])");
    for (got, want) in fill.iter().zip([0x6a_u8, 0x6b, 0x70]) {
        assert!(got.abs_diff(want) <= 3, "riempimento {fill:?}");
    }
    drop(glass);
    drop(ossido);

    // Lens: a horizontal ramp behind (level = x up to 255: the blur does not change a linear
    // ramp), glass from x 64 to 363; the same glass over gray gives the edge's own light,
    // taken out before reading the source position from the level.
    let row_over = |name: &str, level: fn(u32) -> u8| {
        let ossido = Headless::start_with(name, SHOT, &[]);
        let back = (500, 200);
        let mut pixels = Vec::new();
        for _ in 0..back.1 {
            for x in 0..back.0 {
                let l = level(x);
                pixels.extend_from_slice(&[l, l, l, 255]);
            }
        }
        let _back = TestWindow::open_pixels(&ossido.wayland, "back", back, &pixels);
        let size = (300, 100);
        let mut glass =
            TestWindow::open_pixels(&ossido.wayland, "liquid", size, &transparent(size));
        glass
            .set_glass(&[(0, 0, 300, 100, 0.0, Material::Regular)])
            .unwrap();
        let shot = shot(&ossido, &format!("{name}.png"));
        (64..164)
            .map(|x| f64::from(rgb(&shot, x, 114)[1]))
            .collect::<Vec<f64>>()
    };
    let ramp = row_over("glass-liquid-ramp", |x| x.min(255) as u8);
    let gray = row_over("glass-liquid-gray", |_| 0x7b);
    // Past ~24 pt the glass shows what is behind it: there the level gives the scale.
    let (t0, t1) = (40, 90);
    let gain = (ramp[t1] - ramp[t0]) / (t1 - t0) as f64;
    let source = |t: usize| {
        let edge = gray[t] - gray[t1];
        t1 as f64 + (ramp[t] - edge - ramp[t1]) / gain
    };
    // The lens of the Control Center modules (GLASS-PROFILES): up to ~3.5 pt further inside,
    // around 15 pt from the edge, nothing past 30 pt. (distance from the edge, source) in pt.
    let measured = [
        (4, 5.6),
        (6, 8.2),
        (8, 10.8),
        (12, 15.3),
        (15, 18.5),
        (16, 19.3),
        (30, 30.0),
    ];
    let mut worst: f64 = 0.0;
    for (t, expected) in measured {
        let got = source(t);
        eprintln!("t {t} pt → source {got:.1} pt (measured {expected:.1})");
        worst = worst.max((got - expected).abs());
    }
    assert!(
        worst <= 3.0,
        "lens far from the measured curve: {worst:.1} pt"
    );
    // It peaks at ~3.5 pt around 15 pt from the edge.
    assert!(
        (2.0..=5.0).contains(&(source(15) - 15.0)),
        "the lens does not peak at 15 pt: {:.1}",
        source(15)
    );
}

#[test]
fn nearby_regions_of_the_same_glass_merge_like_drops() {
    let ossido = Headless::start_with("glass-merge", SHOT, &["--background", "7a7b80"]);
    let size = (400, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "drops", size, &transparent(size));
    let background = [0x7a_u8, 0x7b, 0x80];
    let differs = |shot: &[u8], x: u32, y: u32| {
        rgb(shot, x, y)
            .iter()
            .zip(background)
            .any(|(got, bg)| got.abs_diff(bg) > 6)
    };
    // Two capsules 4 pt from each other: the point in between becomes glass (window at
    // (32, 32): capsules at x 32–131 and 136–235, gap centered at x 134, y 82).
    glass
        .set_glass(&[
            (0, 20, 100, 60, 30.0, Material::Regular),
            (104, 20, 100, 60, 30.0, Material::Regular),
        ])
        .unwrap();
    let joined = shot(&ossido, "glass-merge.png");
    assert!(
        differs(&joined, 134, 82),
        "the nearby capsules do not merge"
    );

    // At 10 pt (the Spotlight buttons from the panel) they stay separate.
    glass
        .set_glass(&[
            (0, 20, 100, 60, 30.0, Material::Regular),
            (110, 20, 100, 60, 30.0, Material::Regular),
        ])
        .unwrap();
    let near = shot(&ossido, "glass-near.png");
    assert!(!differs(&near, 136, 82), "at 10 pt the capsules merge");

    // Far apart (60 pt): the background stays.
    glass
        .set_glass(&[
            (0, 20, 100, 60, 30.0, Material::Regular),
            (160, 20, 100, 60, 30.0, Material::Regular),
        ])
        .unwrap();
    let apart = shot(&ossido, "glass-apart.png");
    assert!(!differs(&apart, 162, 82), "the distant capsules merge");

    // Different materials do not merge, even when close.
    glass
        .set_glass(&[
            (0, 20, 100, 60, 30.0, Material::Regular),
            (104, 20, 100, 60, 30.0, Material::Clear),
        ])
        .unwrap();
    let mixed = shot(&ossido, "glass-mixed.png");
    assert!(!differs(&mixed, 134, 82), "different materials merge");
}

#[test]
fn separate_groups_overlap_without_merging() {
    let ossido = Headless::start_with("glass-groups", SHOT, &["--background", "7a7b80"]);
    let size = (400, 100);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "groups", size, &transparent(size));
    let background = [0x7a_u8, 0x7b, 0x80];
    let differs = |shot: &[u8], x: u32, y: u32| {
        rgb(shot, x, y)
            .iter()
            .zip(background)
            .any(|(got, bg)| got.abs_diff(bg) > 6)
    };
    // The capsules 4 pt apart that merge (above), but in two groups: the gap stays background,
    // like a submenu next to its menu.
    glass
        .set_glass_with(|g| {
            g.add_region(0, 20, 100, 60, 30.0, Material::Regular);
            g.separate();
            g.add_region(104, 20, 100, 60, 30.0, Material::Regular);
        })
        .unwrap();
    let split = shot(&ossido, "glass-groups.png");
    assert!(!differs(&split, 134, 82), "the separate groups merge");
    let list = glass_list(&ossido);
    let groups: Vec<u64> = list[0]["regions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["group"].as_u64().unwrap())
        .collect();
    assert_eq!(groups, [0, 1]);
}

/// Widths of the reference Dock menu's tip per row of 1 pt below the body edge (TIP-01, light:
/// 24.2 19.4 17.0 15.3 13.7 12.1 10.4 8.8 6.9 2.7), rounded.
const TIP_WIDTHS: [u32; 10] = [24, 19, 17, 15, 14, 12, 10, 9, 7, 3];

/// The glass of a thick body with the Dock menu's tip (31 × 10: TIP-01 measures 30.6 × 10.19
/// from where the outline leaves the body edge) on one of its sides, over the reference gray;
/// per row (or column) of the tip from its base outwards, how many pixels are glass.
fn tip_widths(name: &str, side: &str) -> Vec<u32> {
    // Upward and sideways tips: silica-glass-v1 versions 5 and 6.
    common::client::GLASS_VERSION.with(|v| v.set(6));
    let ossido = Headless::start_with(name, SHOT, &["--background", "7a7b80"]);
    let size = (300, 300);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "tip", size, &transparent(size));
    // Body from (60, 60) to (240, 240); the tip centered on the chosen side at 150.
    let side_owned = side.to_owned();
    glass
        .set_glass_with(move |g| {
            g.add_region(60, 60, 180, 180, 12.0, Material::Thick);
            match side_owned.as_str() {
                "down" => g.add_tip(135, 240, 31, 10, Material::Thick, 1.0),
                "up" => g.add_upward_tip(135, 50, 31, 10, Material::Thick, 1.0),
                "left" => g.add_side_tip(50, 135, 10, 31, Side::Left, Material::Thick, 1.0),
                _ => g.add_side_tip(240, 135, 10, 31, Side::Right, Material::Thick, 1.0),
            }
        })
        .unwrap();
    let image = shot(&ossido, &format!("{name}.png"));
    let background = [0x7a_u8, 0x7b, 0x80];
    // Glass: halfway or more from the background to the fill (the outline outside is lighter).
    let fill = rgb(&image, 32 + 150, 32 + 150);
    let is_glass = |x: u32, y: u32| {
        let got = rgb(&image, 32 + x, 32 + y);
        let d = |c: [u8; 3]| {
            c.iter()
                .zip(background)
                .map(|(a, b)| f64::from(*a) - f64::from(b))
                .sum::<f64>()
        };
        d(got) / d(fill) >= 0.5
    };
    (0..12)
        .map(|k| {
            (128..174)
                .filter(|&a| match side {
                    "down" => is_glass(a, 240 + k),
                    "up" => is_glass(a, 59 - k),
                    "left" => is_glass(59 - k, a),
                    _ => is_glass(240 + k, a),
                })
                .count() as u32
        })
        .collect()
}

#[test]
fn a_tip_has_the_shape_of_the_dock_menus_tip() {
    let widths = tip_widths("glass-tip", "down");
    eprintln!("widths {widths:?} (reference {TIP_WIDTHS:?})");
    for (k, (got, want)) in widths.iter().zip(TIP_WIDTHS).enumerate() {
        assert!(
            got.abs_diff(want) <= 2,
            "row {k} below the base: {got} px, reference {want}"
        );
    }
    // Nothing past the apex.
    assert!(
        widths[10] <= 1 && widths[11] == 0,
        "the tip goes too far: {widths:?}"
    );
}

#[test]
fn tips_point_every_way_with_the_same_shape() {
    let down = tip_widths("glass-tip-down", "down");
    for side in ["up", "left", "right"] {
        let widths = tip_widths(&format!("glass-tip-{side}"), side);
        eprintln!("{side}: {widths:?} (down {down:?})");
        for (k, (a, b)) in widths.iter().zip(&down).enumerate() {
            // At the base the fillets' antialiased pixels also carry the edge light, which
            // differs by orientation (the menus' top and bottom rows are lighter than the
            // sides): a pixel more or less per fillet there.
            let tolerance = if k == 0 { 2 } else { 1 };
            assert!(
                a.abs_diff(*b) <= tolerance,
                "{side}, step {k}: {a} px against {b} down"
            );
        }
    }
}

#[test]
fn a_tip_joins_its_body_in_one_shape() {
    let ossido = Headless::start_with("glass-tip-join", SHOT, &["--background", "7a7b80"]);
    let size = (300, 120);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "tip", size, &transparent(size));
    // The Dock menu: thick body and its 31 × 10 tip below, centered at x 100.5.
    glass
        .set_glass_with(|g| {
            g.add_region(40, 10, 200, 60, 12.0, Material::Thick);
            g.add_tip(85, 70, 31, 10, Material::Thick, 1.0);
        })
        .unwrap();
    let image = shot(&ossido, "glass-tip-join.png");
    // No body edge across the tip: on the base, the same color as the inside.
    let base = rgb(&image, 32 + 100, 32 + 70);
    let inside = rgb(&image, 32 + 100, 32 + 60);
    assert!(
        base.iter().zip(inside).all(|(a, b)| a.abs_diff(b) <= 8),
        "the body edge crosses the tip: {base:?} against {inside:?}"
    );
    // Along the body's bottom edge the outline turns into the tip without a notch: right
    // next to the tip's base, the body edge is still glass up to its last row.
    let corner = rgb(&image, 32 + 86, 32 + 69);
    assert!(
        corner.iter().zip(inside).all(|(a, b)| a.abs_diff(b) <= 40),
        "a notch where the tip meets the body: {corner:?}"
    );
    assert_eq!(glass_list(&ossido)[0]["regions"][1]["shape"], "tip");
}

/// The Dock menu with its tip near the left corner (the body 122 × 69, the apex 27 pt from the
/// left edge, as measured): the tip's triangle must not show outside the body above its base. It used to
/// go on into the body, widening, and stuck out of the left side as a wedge.
#[test]
fn a_tip_near_a_corner_does_not_leak_out_of_the_body() {
    let ossido = Headless::start_with("glass-tip-corner", SHOT, &["--background", "7a7b80"]);
    let size = (240, 120);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "tip", size, &transparent(size));
    glass
        .set_glass_with(|g| {
            g.add_region(50, 10, 122, 69, 12.0, Material::Thick);
            g.add_tip(62, 79, 31, 10, Material::Thick, 1.0);
        })
        .unwrap();
    let image = shot(&ossido, "glass-tip-corner.png");
    let background = [0x7a_u8, 0x7b, 0x80];
    let lift = |c: [u8; 3]| {
        c.iter()
            .zip(background)
            .map(|(a, b)| f64::from(*a) - f64::from(b))
            .sum::<f64>()
    };
    let fill = lift(rgb(&image, 32 + 110, 32 + 40));
    // Left of the body, along its height except the rounded corners: the background (the
    // dark outline just outside the edge is allowed, glass is not).
    for y in 24..66 {
        for x in 38..49 {
            let got = rgb(&image, 32 + x, 32 + y);
            assert!(
                lift(got) / fill < 0.3,
                "glass left of the body at ({x}, {y}): {got:?}"
            );
        }
    }
    // The tip itself is still there.
    let tip = rgb(&image, 32 + 77, 32 + 82);
    assert!(lift(tip) / fill > 0.5, "the tip is gone: {tip:?}");
}

#[test]
fn notification_glass_matches_the_isolated_banner() {
    // The isolated banner on the reference gray: #bfc0c5 on top, #c2c3c8 at the bottom
    // (notifications.md, "Banner · material on gray").
    let ossido = Headless::start_with("glass-notification", SHOT, &["--background", "7a7b80"]);
    let size = (344, 91);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "banner", size, &transparent(size));
    glass
        .set_glass(&[(0, 0, 344, 91, 20.0, Material::Notification)])
        .unwrap();
    let fill = rgb(&shot(&ossido, "glass-notification.png"), 32 + 172, 32 + 45);
    eprintln!("fill: {fill:?} (reference [192, 193, 198])");
    for (got, want) in fill.iter().zip([0xc0_u8, 0xc1, 0xc6]) {
        assert!(got.abs_diff(want) <= 3, "riempimento {fill:?}");
    }

    // Two cards 6 pt apart (the Center's cards, with the glass 1 pt wider per side) stay
    // separate: in the gap between them there is the background.
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
    assert_eq!(gap, [0x7a, 0x7b, 0x80], "between the cards: {gap:?}");
    drop(glass);

    // It belongs to version 2: a client bound to 1 that asks for it is disconnected.
    common::client::GLASS_VERSION.with(|v| v.set(1));
    let mut old = TestWindow::open_pixels(&ossido.wayland, "old", size, &transparent(size));
    assert!(
        old.set_glass(&[(0, 0, 344, 91, 20.0, Material::Notification)])
            .is_err()
    );
    common::client::GLASS_VERSION.with(|v| v.set(2));
}

#[test]
fn every_region_of_a_group_has_its_rim_on_its_own_edge() {
    // Like the Control Center modules: regions of the same material far from each other,
    // capsules and rounded rectangles. The bright line sits on the first row of each
    // region, top and bottom, even far from the center of the group.
    let ossido = Headless::start_with("glass-group-rim", SHOT, &["--background", "7a7b80"]);
    let size = (292, 330);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "stack", size, &transparent(size));
    // The Control Center grid: modules 12 pt from each other, with the glass 1 pt wider
    // per side (10 pt between the glasses horizontally).
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
            "region at y {y}: top row"
        );
        assert_eq!(
            brightest(column, bottom - 3, bottom + 4),
            bottom,
            "region at y {y}: bottom row"
        );
        // At 12 pt the modules do not touch: the row right outside is background (in the
        // reference `#6e6f73` below the Wi-Fi, no bridge towards the Bluetooth).
        let outside = rgb(&shot, column, bottom + 1);
        assert!(
            outside
                .iter()
                .zip([0x7a_u8, 0x7b, 0x80])
                .all(|(a, b)| a.abs_diff(b) <= 4),
            "region at y {y}: below the edge {outside:?}"
        );
    }
    // Between the first module and the media one: the dark line just outside the side edge
    // (controlcenter.md, "1 pt border sticking out"), then background.
    let line = rgb(&shot, 32 + 142, 32 + 32);
    assert!(
        line[0] < 0x50,
        "no dark line outside the side edge {line:?}"
    );
    let beside = rgb(&shot, 32 + 144, 32 + 32);
    assert!(
        beside
            .iter()
            .zip([0x7a_u8, 0x7b, 0x80])
            .all(|(a, b)| a.abs_diff(b) <= 4),
        "next to the side edge {beside:?}"
    );
}

#[test]
fn translucent_glass_fades_with_its_panel() {
    // At half opacity the notification glass on the gray is halfway between the background
    // (#7a7b80) and the full fill (#c0c1c6): a fading panel carries the glass with it.
    let ossido = Headless::start_with("glass-translucent", SHOT, &["--background", "7a7b80"]);
    let size = (344, 91);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "banner", size, &transparent(size));
    glass
        .set_translucent_glass(&[((0, 0, 344, 91, 20.0, Material::Notification), 0.5)])
        .unwrap();
    let fill = rgb(&shot(&ossido, "glass-translucent.png"), 32 + 172, 32 + 45);
    for (got, want) in fill.iter().zip([157_u8, 158, 163]) {
        assert!(got.abs_diff(want) <= 4, "half opacity: {fill:?}");
    }
    // Fully transparent: the background stays.
    glass
        .set_translucent_glass(&[((0, 0, 344, 91, 20.0, Material::Notification), 0.0)])
        .unwrap();
    let fill = rgb(&shot(&ossido, "glass-translucent-0.png"), 32 + 172, 32 + 45);
    assert_eq!(fill, [0x7a, 0x7b, 0x80]);
    // Outside 0–1: the client is disconnected, Ossido goes on.
    assert!(
        glass
            .set_translucent_glass(&[((0, 0, 344, 91, 20.0, Material::Notification), 1.5)])
            .is_err()
    );
    let _alive = TestWindow::open(&ossido.wayland, "after");
}

/// A wallpaper window and a glass window both covering the screen (and 40 px beyond, so their
/// corners and shadows stay off screen): screen pixel (x, y) is window pixel (x + 40, y + 40).
fn full_screen_glass(
    ossido: &Headless,
    level: impl Fn(u32, u32) -> [u8; 3],
) -> (TestWindow, TestWindow) {
    const BLEED: u32 = 40;
    let size = (SHOT.0 + 2 * BLEED, SHOT.1 + 2 * BLEED);
    let mut pixels = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for y in 0..size.1 {
        for x in 0..size.0 {
            let [r, g, b] = level(x.saturating_sub(BLEED), y.saturating_sub(BLEED));
            pixels.extend_from_slice(&[b, g, r, 255]);
        }
    }
    let place = || {
        ossido
            .client()
            .call(
                silica_control::commands::WINDOW_MOVE,
                serde_json::json!({"x": -(BLEED as i32), "y": -(BLEED as i32)}),
            )
            .unwrap();
    };
    let back = TestWindow::open_pixels(&ossido.wayland, "back", size, &pixels);
    place();
    let glass = TestWindow::open_pixels(&ossido.wayland, "glass", size, &transparent(size));
    place();
    (back, glass)
}

#[test]
fn glass_cut_by_the_screen_edge_does_not_blur_in_the_far_side() {
    // A menu 2 px from the left edge of the screen: its capture is cut there. White behind it,
    // black from x 175 on (inside the capture's right margin). The offscreen textures repeat
    // by default: without clamping, the blur at the left edge read the black from the far
    // right of the capture (#168).
    let ossido = Headless::start_with("glass-edge-clamp", SHOT, &[]);
    let (_back, mut glass) =
        full_screen_glass(&ossido, |x, _| if x < 175 { [255; 3] } else { [0; 3] });
    glass
        .set_glass(&[(40 + 2, 40 + 100, 150, 120, 12.0, Material::Thick)])
        .unwrap();
    let shot = shot(&ossido, "glass-edge-clamp.png");
    let middle = rgb(&shot, 60, 160);
    for x in [4, 6, 10, 20] {
        let edge = rgb(&shot, x, 160);
        // Inside the edge rows: the side profile darkens by a few levels only.
        let worst = edge
            .iter()
            .zip(middle)
            .map(|(e, m)| i32::from(*e) - i32::from(m))
            .min()
            .unwrap_or(0);
        assert!(
            worst >= -8,
            "x {x}: {edge:?} darker than the middle {middle:?}"
        );
    }
}

#[test]
fn the_dark_side_line_stays_on_the_side_of_a_capsule() {
    // A Control Center module (a capsule, 142 × 64, radius 32) over the reference gray. The
    // dark line just outside the edge is on the side and fades within ~45° around the end,
    // as measured on the modules; it used to wrap the whole end (a black crescent, #168).
    let ossido = Headless::start_with("glass-capsule-end", SHOT, &[]);
    let (_back, mut glass) = full_screen_glass(&ossido, |_, _| [0x7a, 0x7b, 0x80]);
    glass
        .set_glass(&[(40 + 100, 40 + 100, 142, 64, 32.0, Material::Regular)])
        .unwrap();
    let shot = shot(&ossido, "glass-capsule-end.png");
    let background = f64::from(rgb(&shot, 40, 40)[1]);
    // The darkest pixel just outside the edge at an angle from the left side (0°) towards the
    // top, relative to the background.
    let (cx, cy, r) = (100.0 + 32.0, 100.0 + 32.0, 32.0);
    let darkness = |deg: f64| {
        let a = deg.to_radians();
        (0..3)
            .map(|step| {
                let d = r + 0.25 + f64::from(step) * 0.5;
                let x = cx - a.cos() * d;
                let y = cy - a.sin() * d;
                1.0 - f64::from(rgb(&shot, x.floor() as u32, y.floor() as u32)[1]) / background
            })
            .fold(0.0, f64::max)
    };
    let side = darkness(0.0);
    eprintln!(
        "dark line: 0° {side:.2}, 30° {:.2}, 45° {:.2}, 60° {:.2}",
        darkness(30.0),
        darkness(45.0),
        darkness(60.0)
    );
    assert!(side > 0.45, "no dark line on the side: {side:.2}");
    assert!(darkness(45.0) < 0.3 * side, "45°: {:.2}", darkness(45.0));
    assert!(darkness(60.0) < 0.15 * side, "60°: {:.2}", darkness(60.0));
}

#[test]
fn a_profile_applies_to_the_following_regions_until_clear() {
    common::client::GLASS_VERSION.with(|v| v.set(7));
    let ossido = Headless::start("glass-profile-list");
    let mut window = TestWindow::open(&ossido.wayland, "glass");
    window
        .set_glass_with(|g| {
            g.add_region(0, 0, 40, 20, 8.0, Material::Thick);
            g.set_profile(Profile::Tooltip, Shadow::Compositor);
            g.add_region(0, 30, 40, 20, 8.0, Material::Thick);
            g.set_profile(Profile::Default, Shadow::Compositor);
            g.add_region(0, 60, 40, 20, 8.0, Material::Thick);
        })
        .unwrap();
    let list = glass_list(&ossido);
    let regions = &list[0]["regions"];
    assert_eq!(regions[0]["profile"], Value::Null, "{list}");
    assert_eq!(regions[1]["profile"], "tooltip");
    assert_eq!(regions[1]["shadow"], true);
    // Without a profile there is no shadow of its own to draw.
    assert_eq!(regions[2]["shadow"], false);
    // `clear` goes back to the default.
    window
        .set_glass(&[(0, 0, 40, 20, 8.0, Material::Thick)])
        .unwrap();
    assert_eq!(glass_list(&ossido)[0]["regions"][0]["profile"], Value::Null);

    let windows = ossido.client().call(WINDOW_LIST, Value::Null).unwrap();
    assert!(windows.as_array().is_some_and(|list| !list.is_empty()));
}

/// The source of the lens at `t` pt from the left end of a 120 × 24 pill over a ramp
/// (level = x), with a profile or the material's.
fn pill_source(name: &str, profile: Option<Profile>) -> Vec<f64> {
    common::client::GLASS_VERSION.with(|v| v.set(7));
    let ossido = Headless::start_with(name, SHOT, &[]);
    let (_back, mut glass) = full_screen_glass(&ossido, |x, _| {
        let l = x.min(255) as u8;
        [l, l, l]
    });
    glass
        .set_glass_with(|g| {
            if let Some(profile) = profile {
                g.set_profile(profile, Shadow::Client);
            }
            g.add_region(40 + 60, 40 + 100, 120, 24, 12.0, Material::Regular);
        })
        .unwrap();
    let image = shot(&ossido, &format!("{name}.png"));
    (60..120)
        .map(|x| f64::from(rgb(&image, x, 112)[1]))
        .collect()
}

#[test]
fn small_controls_refract_less_than_modules() {
    // A 24 pt pill. With the module lens, scaled to the pill (it never samples past the far
    // edge), the left end shows the ramp a few pt from inside; with the control profile the
    // displacement is about half (glass_profile.rs). The control's tone has its own gain, so
    // each row is read with its own scale.
    let module = pill_source("glass-pill-module", None);
    let control = pill_source("glass-pill-control", Some(Profile::Control));
    // At 40 pt the lens no longer acts: the ramp's slope gives the scale.
    let shift = |row: &[f64], t: usize| {
        let gain = (row[50] - row[40]) / 10.0;
        assert!(gain > 0.2, "no ramp under the pill: {row:?}");
        (row[t] - row[40]) / gain + 40.0 - t as f64
    };
    // The most the lens moves the background within the first 12 pt (the edge's own light
    // adds to the level very near the edge, so it is not read closer than 5 pt).
    let strongest = |row: &[f64]| (5..12).map(|t| shift(row, t)).fold(f64::MIN, f64::max);
    let (m, c) = (strongest(&module), strongest(&control));
    eprintln!("strongest displacement: module lens {m:.1} pt, control {c:.1} pt");
    assert!(m > 1.0, "the module lens does not act on the pill: {m:.1}");
    // Never past the pill's far end.
    assert!(m + 5.0 <= 24.0, "the lens samples past the pill: {m:.1}");
    assert!(
        c < 0.9 * m,
        "the control lens is not gentler: {c:.1} against {m:.1}"
    );
}

#[test]
fn the_compositor_draws_the_profile_shadow_only_when_asked() {
    common::client::GLASS_VERSION.with(|v| v.set(7));
    let below = |name: &str, shadow: Shadow| {
        let ossido = Headless::start_with(name, SHOT, &[]);
        let (_back, mut glass) = full_screen_glass(&ossido, |_, _| [0xc0; 3]);
        glass
            .set_glass_with(|g| {
                g.set_profile(Profile::Menu, shadow);
                g.add_region(40 + 100, 40 + 100, 200, 120, 12.0, Material::Thick);
            })
            .unwrap();
        let image = shot(&ossido, &format!("{name}.png"));
        // 4 pt and 16 pt below the bottom edge, mid-width; and well above the top edge.
        (
            rgb(&image, 200, 224)[1],
            rgb(&image, 200, 236)[1],
            rgb(&image, 200, 80)[1],
        )
    };
    let (near, far, above) = below("glass-shadow-compositor", Shadow::Compositor);
    eprintln!("compositor shadow: {near} at 4 pt, {far} at 16 pt, {above} above");
    assert!(near < 0xc0 - 10, "no shadow under the menu: {near}");
    assert!(far > near, "the shadow does not fade: {near} then {far}");
    assert!(above >= 0xc0 - 4, "the shadow sits above the menu: {above}");
    let (near, _, _) = below("glass-shadow-client", Shadow::Client);
    assert!(
        near >= 0xc0 - 2,
        "a shadow the client did not ask for: {near}"
    );
}

/// A point at `(x, y)` from the centre `c` of a region, in its own upright frame, on screen
/// once the region is turned by `degrees` clockwise about that centre (to its pixel).
fn turned(c: (f64, f64), degrees: f64, (x, y): (f64, f64)) -> (u32, u32) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let sx = c.0 + cos * x - sin * y;
    let sy = c.1 + sin * x + cos * y;
    (sx.floor() as u32, sy.floor() as u32)
}

#[test]
fn a_rotated_region_turns_its_whole_shape() {
    common::client::GLASS_VERSION.with(|v| v.set(8));
    let ossido = Headless::start_with("glass-rotated", SHOT, &["--background", "7a7b80"]);
    let size = (320, 320);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "turned", size, &transparent(size));
    // A 200 × 60 menu glass turned 20° clockwise about its centre (window (160, 160)).
    glass
        .set_glass_with(|g| {
            g.set_rotation(20.0);
            g.add_region(60, 130, 200, 60, 10.0, Material::Thick);
        })
        .unwrap();
    let list = glass_list(&ossido);
    assert_eq!(list[0]["regions"][0]["rotation"], 20.0, "{list}");
    assert_eq!(list[0]["regions"][0]["width"], 200);
    let image = shot(&ossido, "glass-rotated.png");
    let background = [0x7a_u8, 0x7b, 0x80];
    let lift = |(x, y): (u32, u32)| {
        rgb(&image, 32 + x, 32 + y)
            .iter()
            .zip(background)
            .map(|(a, b)| f64::from(*a) - f64::from(b))
            .sum::<f64>()
            / 3.0
    };
    let centre = (160.0, 160.0);
    let fill = lift(turned(centre, 20.0, (0.0, 0.0)));
    assert!(fill > 40.0, "no glass at the centre: {fill:.1}");
    // Glass just inside the four turned corners (past the 10 px rounding).
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let at = turned(centre, 20.0, (sx * 93.0, sy * 23.0));
        assert!(
            lift(at) > 0.6 * fill,
            "no glass inside the turned corner at {at:?}"
        );
        // Past the turned side, the background.
        let out = turned(centre, 20.0, (sx * 60.0, sy * 36.0));
        assert!(
            lift(out).abs() < 0.2 * fill,
            "glass outside the turned side at {out:?}"
        );
    }
    // The upright rectangle's top-right and bottom-left corners are not glass any more.
    for at in [(255, 135), (65, 185)] {
        assert!(
            lift(at).abs() < 0.2 * fill,
            "the upright corner {at:?} is still glass"
        );
    }
    // Without a rotation (an earlier client, or after `clear`) the upright rectangle.
    glass
        .set_glass(&[(60, 130, 200, 60, 10.0, Material::Thick)])
        .unwrap();
    assert_eq!(glass_list(&ossido)[0]["regions"][0]["rotation"], 0.0);
    let image = shot(&ossido, "glass-rotated-cleared.png");
    let corner = rgb(&image, 32 + 255, 32 + 135);
    assert!(
        corner[0] > 0x7a + 40,
        "the upright region lost its corner: {corner:?}"
    );
}

#[test]
fn a_rotated_region_keeps_its_edge_light_on_its_own_top_edge() {
    common::client::GLASS_VERSION.with(|v| v.set(8));
    let ossido = Headless::start_with("glass-rotated-edge", SHOT, &["--background", "7a7b80"]);
    let size = (320, 320);
    let mut glass = TestWindow::open_pixels(&ossido.wayland, "turned", size, &transparent(size));
    // A Control Center module (a light row on top and at the bottom, none on the sides),
    // turned 60°: read upright, its top edge would be a side (no light) and its side a bottom.
    glass
        .set_glass_with(|g| {
            g.set_rotation(60.0);
            g.add_region(60, 120, 200, 80, 12.0, Material::Regular);
        })
        .unwrap();
    let image = shot(&ossido, "glass-rotated-edge.png");
    let level = |(x, y): (u32, u32)| {
        let [r, g, b] = rgb(&image, 32 + x, 32 + y);
        (f64::from(r) + f64::from(g) + f64::from(b)) / 3.0
    };
    let centre = (160.0, 160.0);
    let fill = level(turned(centre, 60.0, (0.0, 0.0)));
    // The brightest pixel inside an edge's middle, from 0.5 to 4 px in (the background
    // outside is lighter than the module).
    let brightest = |along: (f64, f64), half: f64| {
        (5..=40)
            .map(|step| {
                let d = half - f64::from(step) / 10.0;
                level(turned(centre, 60.0, (along.0 * d, along.1 * d)))
            })
            .fold(f64::MIN, f64::max)
    };
    let top = brightest((0.0, -1.0), 40.0);
    let side = brightest((1.0, 0.0), 100.0);
    eprintln!("fill {fill:.1}, top edge {top:.1}, side edge {side:.1}");
    assert!(
        top > fill + 30.0,
        "no light on the turned top edge: {top:.1} over {fill:.1}"
    );
    assert!(
        side < fill + 12.0,
        "light on the turned side edge: {side:.1} over {fill:.1}"
    );
}
