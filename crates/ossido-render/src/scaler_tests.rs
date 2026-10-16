use super::*;

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Physical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

#[test]
fn weights_are_normalised_and_stay_inside() {
    for filter in [
        Filter::Box,
        Filter::Bilinear,
        Filter::CatmullRom,
        Filter::Lanczos(2),
        Filter::Lanczos(3),
    ] {
        for (src, dst) in [(3072, 1920), (3072, 1536), (3840, 1920), (10, 10), (5, 3)] {
            for o in [0, 1, dst / 2, dst - 2, dst - 1] {
                let taps = taps(filter, o, src, dst);
                let total: f64 = taps.iter().map(|t| t.1).sum();
                assert!((total - 1.0).abs() < 1e-9, "{filter:?} {src}->{dst} @{o}");
                assert!(taps.iter().all(|t| t.0 < src));
                assert!(taps.len() <= 14, "{} taps", taps.len());
            }
        }
    }
    // A ratio of 1 is the identity.
    let one = taps(Filter::Lanczos(2), 7, 20, 20);
    assert_eq!(one.iter().filter(|t| t.1.abs() > 1e-9).count(), 1);
    assert!(taps(Filter::Box, 0, 0, 5).is_empty());
}

#[test]
fn a_flat_image_stays_flat_and_alpha_is_respected() {
    let src = [200u8, 100, 50, 255].repeat(32 * 24);
    let out = resample(Filter::Lanczos(3), &src, (32, 24), (20, 15));
    assert!(out.chunks(4).all(|p| p == [200, 100, 50, 255]));
    // A premultiplied edge never leaves colour above alpha.
    let mut row = Vec::new();
    for x in 0..32 {
        row.extend_from_slice(if x < 16 {
            &[255u8, 255, 255, 255]
        } else {
            &[0u8, 0, 0, 0]
        });
    }
    let edge = row.repeat(8);
    let out = resample(Filter::Lanczos(3), &edge, (32, 8), (20, 8));
    assert!(out.chunks(4).all(|p| p[0] <= p[3] && p[1] <= p[3]));
}

#[test]
fn damage_grows_by_the_reach_and_the_rows_follow() {
    let (src, dst) = ((3072, 1920).into(), (1920, 1200).into());
    let plan = plan(&[rect(1000, 1000, 100, 50)], src, dst, 2);
    assert_eq!(plan.dst.len(), 1);
    let d = plan.dst[0];
    // 1000 / 1.6 = 625, minus 3 pixels of margin; 1100 / 1.6 = 687.5 -> 688 + 3.
    assert_eq!((d.loc.x, right(&d)), (622, 691));
    assert_eq!((d.loc.y, bottom(&d)), (622, 660));
    let m = plan.mid[0];
    assert_eq!((m.loc.x, m.size.w), (d.loc.x, d.size.w));
    // The rows the vertical pass reads: 2 output pixels of reach = 3.2 source pixels.
    assert!(m.loc.y <= (f64::from(d.loc.y) * 1.6 - 3.2) as i32);
    assert!(bottom(&m) >= (f64::from(bottom(&d)) * 1.6 + 3.2) as i32);
    // Clamped to the output; nothing for damage outside the source.
    let edge = super::plan(&[rect(0, 0, 3072, 1920)], src, dst, 2);
    assert_eq!(edge.dst, vec![rect(0, 0, 1920, 1200)]);
    assert_eq!(edge.mid, vec![rect(0, 0, 1920, 1920)]);
    assert_eq!(
        super::plan(&[rect(5000, 0, 10, 10)], src, dst, 2),
        Plan::default()
    );
    assert_eq!(super::plan(&[], src, dst, 2), Plan::default());
}

#[test]
fn every_output_pixel_a_damage_changes_is_redrawn() {
    // A source pixel changes the output pixels whose window contains it.
    let (src, dst): (Size<i32, Physical>, Size<i32, Physical>) =
        ((128, 80).into(), (80, 50).into());
    for (x, y) in [(0, 0), (1, 1), (63, 39), (127, 79), (64, 40)] {
        let plan = plan(&[rect(x, y, 1, 1)], src, dst, 2);
        for ox in 0..80usize {
            let reads_x = taps(Filter::Lanczos(2), ox, 128, 80)
                .iter()
                .any(|t| t.0 as i32 == x);
            for oy in 0..50usize {
                let reads_y = taps(Filter::Lanczos(2), oy, 80, 50)
                    .iter()
                    .any(|t| t.0 as i32 == y);
                if reads_x && reads_y {
                    let covered = plan.dst.iter().any(|r| r.contains((ox as i32, oy as i32)));
                    assert!(covered, "source ({x},{y}) changes output ({ox},{oy})");
                }
            }
        }
    }
}

#[test]
fn overlapping_rectangles_merge_and_many_become_one() {
    let merged = merge(vec![
        rect(0, 0, 10, 10),
        rect(5, 5, 10, 10),
        rect(100, 0, 4, 4),
    ]);
    assert_eq!(merged, vec![rect(0, 0, 15, 15), rect(100, 0, 4, 4)]);
    // Touching rectangles do not overlap and stay apart.
    assert_eq!(merge(vec![rect(0, 0, 4, 4), rect(4, 0, 4, 4)]).len(), 2);
    let many: Vec<_> = (0..40).map(|i| rect(i * 10, 0, 4, 4)).collect();
    assert_eq!(merge(many), vec![rect(0, 0, 394, 4)]);
    assert!(merge(vec![rect(0, 0, 0, 5)]).is_empty());
}

#[test]
fn older_buffers_get_the_damage_of_the_frames_they_missed() {
    let mut history = DamageHistory::default();
    let a = rect(0, 0, 10, 10);
    let b = rect(100, 100, 10, 10);
    let c = rect(200, 200, 10, 10);
    // No memory yet: only an age of one is known.
    assert_eq!(history.for_age(1, &[a]), Some(vec![a]));
    assert_eq!(history.for_age(0, &[a]), None);
    assert_eq!(history.for_age(2, &[a]), None);
    history.push(vec![a]);
    history.push(vec![b]);
    assert_eq!(history.for_age(1, &[c]), Some(vec![c]));
    assert_eq!(history.for_age(2, &[c]), Some(vec![b, c]));
    assert_eq!(history.for_age(3, &[c]).map(|v| v.len()), Some(3));
    assert_eq!(history.for_age(4, &[c]), None);
    history.reset();
    assert_eq!(history.for_age(2, &[c]), None);
    for _ in 0..10 {
        history.push(vec![a]);
    }
    assert!(history.for_age(5, &[]).is_some());
    assert_eq!(history.for_age(6, &[]), None);
}

/// A scene in "logical" units (a point on a 1 pt grid), as a coverage of black on white:
/// 1 pt lines every 2 and 3 pt, a checker of 1 pt cells, diagonal bars, a disc and a
/// rectangle with edges between pixels: the things text and controls are made of.
fn ink(x: f64, y: f64) -> f64 {
    let inside = |x0: f64, y0: f64, x1: f64, y1: f64| x >= x0 && x < x1 && y >= y0 && y < y1;
    let mut v = 0.0;
    if inside(2.0, 2.0, 22.0, 14.0) && (x.floor() as i64) % 2 == 0 {
        v = 1.0;
    }
    if inside(26.0, 2.0, 46.0, 14.0) && (y.floor() as i64) % 3 == 0 {
        v = 1.0;
    }
    if inside(2.0, 18.0, 18.0, 34.0) && ((x.floor() + y.floor()) as i64) % 2 == 0 {
        v = 1.0;
    }
    if inside(22.0, 18.0, 46.0, 34.0) && ((x - y).rem_euclid(5.0)) < 1.5 {
        v = 1.0;
    }
    if (x - 54.0).powi(2) + (y - 26.0).powi(2) < 49.0 {
        v = 1.0;
    }
    if inside(50.3, 3.4, 61.2, 12.8) {
        v = 1.0;
    }
    1.0 - v
}

/// The scene rasterised on a `w` x `h` grid covering 64 x 40 logical points, `n` x `n`
/// samples per pixel.
fn raster(w: usize, h: usize, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0;
            for j in 0..n {
                for i in 0..n {
                    let px = (x as f64 + (i as f64 + 0.5) / n as f64) * 64.0 / w as f64;
                    let py = (y as f64 + (j as f64 + 0.5) / n as f64) * 40.0 / h as f64;
                    sum += ink(px, py);
                }
            }
            let g = (sum / (n * n) as f64 * 255.0).round() as u8;
            out.extend_from_slice(&[g, g, g, 255]);
        }
    }
    out
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let mut sq = 0.0;
    let mut n = 0.0;
    for (p, q) in a.chunks(4).zip(b.chunks(4)) {
        let d = f64::from(p[0]) - f64::from(q[0]);
        sq += d * d;
        n += 1.0;
    }
    10.0 * (255.0 * 255.0 / (sq / n).max(1e-9)).log10()
}

/// The measurement behind the choice of the filter (`docs/displays.md`): the 2x scene
/// reduced by each filter against the scene rasterised straight at the native size with
/// 8x8 samples per pixel. Run with `--nocapture` to see the table.
#[test]
fn lanczos_2_is_the_best_compromise_on_text_like_content() {
    let (sw, sh) = (128, 80);
    let source = raster(sw, sh, 4);
    let mut means = std::collections::BTreeMap::<&str, f64>::new();
    for (dw, dh) in [(73, 46), (80, 50), (85, 53), (96, 60)] {
        let truth = raster(dw, dh, 8);
        let mut table = Vec::new();
        for (name, filter) in [
            ("box", Filter::Box),
            ("bilinear", Filter::Bilinear),
            ("catmull-rom", Filter::CatmullRom),
            ("lanczos-2", Filter::Lanczos(2)),
            ("lanczos-3", Filter::Lanczos(3)),
        ] {
            let out = resample(filter, &source, (sw, sh), (dw, dh));
            let clipped = out
                .chunks(4)
                .map(|p| p[0])
                .filter(|&g| g == 0 || g == 255)
                .count();
            table.push((name, psnr(&out, &truth), clipped));
        }
        for (name, db, clipped) in &table {
            println!("{sw}x{sh} -> {dw}x{dh} {name:12} {db:6.2} dB  clipped {clipped}");
        }
        for (name, db, _) in &table {
            *means.entry(name).or_default() += db / 4.0;
        }
    }
    println!("mean dB: {means:?}");
    let get = |n: &str| means.get(n).copied().unwrap_or(0.0);
    assert!(get("lanczos-2") > get("bilinear") + 3.0, "{means:?}");
    assert!(get("lanczos-2") > get("box"), "{means:?}");
}
