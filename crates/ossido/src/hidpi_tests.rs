//! The scaled mode's reduction on a real GLES context (llvmpipe in CI): the shader against the
//! CPU reference, the damage plan against a full redraw, and the cost (`--ignored`).

use ossido_render::scaler::{self, Downscaler, Filter};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, ExportMem, ImportMem, Offscreen};
use smithay::utils::{Buffer, Physical, Rectangle, Size, Transform};

type Rect = Rectangle<i32, Physical>;

fn buffer(size: Size<i32, Physical>) -> Size<i32, Buffer> {
    size.to_logical(1).to_buffer(1, Transform::Normal)
}

/// A scene that tells the axes apart and has what text has: a red/green gradient in the
/// corner, 1 px lines, a checker, diagonals and a bar that stops short of the edge.
fn scene(w: usize, h: usize, seed: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let mut px = [250u8, 250, 250, 255];
            if x < w / 4 && y < h / 4 {
                px = [
                    (x * 255 / (w / 4)) as u8,
                    (y * 255 / (h / 4)) as u8,
                    seed,
                    255,
                ];
            } else if x % 2 == 0 && y > h / 2 && x > w / 2 {
                px = [10, 10, 10, 255];
            } else if (x / 2 + y / 2) % 2 == 0 && x > w / 3 && x < w / 2 && y > h / 3 {
                px = [20, 20, 120, 255];
            } else if (x + 2 * y) % 7 < 2 && y < h / 3 && x > w / 3 {
                px = [0, 0, 0, 255];
            } else if y == h - 3 && x < w * 3 / 4 {
                px = [0, 120, 0, 255];
            }
            out.extend_from_slice(&px);
        }
    }
    out
}

fn upload(renderer: &mut GlesRenderer, rgba: &[u8], w: i32, h: i32) -> GlesTexture {
    renderer
        .import_memory(rgba, Fourcc::Abgr8888, (w, h).into(), false)
        .unwrap()
}

fn read(
    renderer: &mut GlesRenderer,
    texture: &mut GlesTexture,
    size: Size<i32, Physical>,
) -> Vec<u8> {
    let target = renderer.bind(texture).unwrap();
    let mapping = renderer
        .copy_framebuffer(
            &target,
            Rectangle::from_size(buffer(size)),
            Fourcc::Abgr8888,
        )
        .unwrap();
    renderer.map_texture(&mapping).unwrap().to_vec()
}

fn max_diff(a: &[u8], b: &[u8]) -> (u8, f64) {
    let worst = a
        .iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0);
    let mean = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(x.abs_diff(*y)))
        .sum::<f64>()
        / a.len() as f64;
    (worst, mean)
}

fn target(renderer: &mut GlesRenderer, size: Size<i32, Physical>) -> GlesTexture {
    Offscreen::<GlesTexture>::create_buffer(renderer, Fourcc::Abgr8888, buffer(size)).unwrap()
}

#[test]
fn the_shader_matches_the_cpu_reference() {
    let mut renderer = crate::egl::headless_renderer(false).unwrap();
    for lobes in [2, 3] {
        for ((sw, sh), (dw, dh)) in [
            ((384, 240), (240, 150)),
            ((448, 280), (256, 160)),
            ((256, 160), (256, 160)),
        ] {
            let source = scene(sw, sh, 40);
            let texture = upload(&mut renderer, &source, sw as i32, sh as i32);
            let (src, dst): (Size<i32, Physical>, Size<i32, Physical>) =
                ((sw as i32, sh as i32).into(), (dw as i32, dh as i32).into());
            let mut out = target(&mut renderer, dst);
            let mut downscaler = Downscaler::new(&mut renderer, lobes).unwrap();
            {
                let mut bound = renderer.bind(&mut out).unwrap();
                let (drawn, sync) = downscaler
                    .run(
                        &mut renderer,
                        &texture,
                        src,
                        &mut bound,
                        dst,
                        &[Rectangle::from_size(src)],
                    )
                    .unwrap();
                assert_eq!(drawn, vec![Rectangle::from_size(dst)]);
                let _ = sync.wait();
            }
            let gpu = read(&mut renderer, &mut out, dst);
            let cpu = scaler::resample(Filter::Lanczos(lobes), &source, (sw, sh), (dw, dh));
            let (worst, mean) = max_diff(&gpu, &cpu);
            println!("lanczos-{lobes} {sw}x{sh} -> {dw}x{dh}: worst {worst}, mean {mean:.3}");
            // Two 8-bit passes against one float computation.
            assert!(worst <= 4 && mean < 0.5, "worst {worst}, mean {mean}");
        }
    }
}

#[test]
fn redrawing_only_the_damage_gives_the_full_result() {
    let mut renderer = crate::egl::headless_renderer(false).unwrap();
    let (sw, sh, dw, dh) = (384usize, 240usize, 240usize, 150usize);
    let (src, dst): (Size<i32, Physical>, Size<i32, Physical>) =
        ((sw as i32, sh as i32).into(), (dw as i32, dh as i32).into());
    let first = scene(sw, sh, 40);
    // The same scene with a block changed: what an app's repaint does.
    let mut second = first.clone();
    let block: Rect = Rectangle::new((150, 90).into(), (37, 21).into());
    for y in block.loc.y..block.loc.y + block.size.h {
        for x in block.loc.x..block.loc.x + block.size.w {
            let at = (y as usize * sw + x as usize) * 4;
            second[at..at + 4].copy_from_slice(&[255, 0, 255, 255]);
        }
    }
    let mut out = target(&mut renderer, dst);
    let mut downscaler = Downscaler::new(&mut renderer, 2).unwrap();
    let texture = upload(&mut renderer, &first, sw as i32, sh as i32);
    {
        let mut bound = renderer.bind(&mut out).unwrap();
        let _ = downscaler
            .run(
                &mut renderer,
                &texture,
                src,
                &mut bound,
                dst,
                &[Rectangle::from_size(src)],
            )
            .unwrap();
    }
    let texture = upload(&mut renderer, &second, sw as i32, sh as i32);
    let drawn = {
        let mut bound = renderer.bind(&mut out).unwrap();
        let (drawn, sync) = downscaler
            .run(&mut renderer, &texture, src, &mut bound, dst, &[block])
            .unwrap();
        let _ = sync.wait();
        drawn
    };
    // Much less than the whole output was drawn, and the result is the full reduction.
    let area: i32 = drawn.iter().map(|r| r.size.w * r.size.h).sum();
    assert!(area < dw as i32 * dh as i32 / 8, "{area} px redrawn");
    assert!(area > 0);
    let gpu = read(&mut renderer, &mut out, dst);
    let cpu = scaler::resample(Filter::Lanczos(2), &second, (sw, sh), (dw, dh));
    let (worst, mean) = max_diff(&gpu, &cpu);
    println!("partial redraw: {area} px, worst {worst}, mean {mean:.3}");
    assert!(worst <= 4 && mean < 0.5, "worst {worst}, mean {mean}");
}

#[test]
fn an_older_buffer_is_brought_up_to_date_with_the_missed_frames() {
    // A swapchain buffer of age 2 misses the previous frame's damage as well.
    let (src, dst): (Size<i32, Physical>, Size<i32, Physical>) =
        ((384, 240).into(), (240, 150).into());
    let a: Rect = Rectangle::new((10, 10).into(), (20, 20).into());
    let b: Rect = Rectangle::new((300, 200).into(), (20, 20).into());
    let mut history = scaler::DamageHistory::default();
    let first = scaler::plan(&[a], src, dst, 2).dst;
    history.push(first.clone());
    let second = scaler::plan(&[b], src, dst, 2).dst;
    let redraw = history.for_age(2, &second).unwrap();
    // Both areas are in the redraw, and the redraw's rows are all read by the plan.
    let plan = scaler::plan_from_dst(redraw, src, dst, 2);
    for r in first.iter().chain(&second) {
        assert!(plan.dst.iter().any(|d| d.contains_rect(*r)), "{r:?}");
    }
    for d in &plan.dst {
        assert!(plan.mid.iter().any(|m| m.loc.x <= d.loc.x
            && m.loc.x + m.size.w >= d.loc.x + d.size.w
            && m.loc.y <= (d.loc.y as f64 * 1.6) as i32 - 3
            && m.loc.y + m.size.h >= ((d.loc.y + d.size.h) as f64 * 1.6) as i32 + 3));
    }
}

/// Cost of the reduction on this machine: `cargo test -p ossido --release hidpi_cost --
/// --ignored --nocapture`. Numbers for `docs/displays.md`.
#[test]
#[ignore = "a measurement"]
fn hidpi_cost() {
    use std::time::Instant;
    let mut renderer =
        crate::egl::headless_renderer(std::env::var("OSSIDO_BENCH_GPU").is_ok()).unwrap();
    for (name, (nw, nh), (lw, lh)) in [
        ("1920x1200 -> 1536x960", (1920, 1200), (1536, 960)),
        ("1920x1200 -> 1680x1050", (1920, 1200), (1680, 1050)),
        ("3840x2160 -> 2560x1440", (3840, 2160), (2560, 1440)),
    ] {
        let (src, dst): (Size<i32, Physical>, Size<i32, Physical>) =
            ((lw * 2, lh * 2).into(), (nw, nh).into());
        let pixels = vec![200u8; (src.w * src.h * 4) as usize];
        let texture = upload(&mut renderer, &pixels, src.w, src.h);
        let mut out = target(&mut renderer, dst);
        let mut downscaler = Downscaler::new(&mut renderer, 2).unwrap();
        let small: Rect = Rectangle::new((600, 400).into(), (300, 200).into());
        for (what, damage) in [
            ("full", Rectangle::from_size(src)),
            ("300x200 damage", small),
        ] {
            let mut times = Vec::new();
            for round in 0..8 {
                let start = Instant::now();
                let mut bound = renderer.bind(&mut out).unwrap();
                let (_, sync) = downscaler
                    .run(&mut renderer, &texture, src, &mut bound, dst, &[damage])
                    .unwrap();
                let _ = sync.wait();
                // Force the work to finish: a one-pixel readback.
                let mapping = renderer
                    .copy_framebuffer(
                        &bound,
                        Rectangle::new((0, 0).into(), (1, 1).into()),
                        Fourcc::Abgr8888,
                    )
                    .unwrap();
                let _ = renderer.map_texture(&mapping).unwrap();
                if round >= 2 {
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                }
            }
            times.sort_by(f64::total_cmp);
            println!(
                "{name} {what}: {:.2} ms (median of {})",
                times[times.len() / 2],
                times.len()
            );
        }
    }
}

/// The pointer in a scaled mode: drawn at 2x (the 48 px Adwaita frame), its hotspot on the
/// pointer's place in the 2x frame, and from there on the pointer's place on the display.
#[cfg(feature = "drm")]
#[test]
fn the_cursor_hotspot_lands_on_the_pointer() {
    use smithay::backend::renderer::element::Element;
    use smithay::input::pointer::{CursorIcon, CursorImageStatus};
    use smithay::utils::Scale;
    let mut renderer = crate::egl::headless_renderer(false).unwrap();
    let frame = vec![255u8; 48 * 48 * 4];
    let texture = upload(&mut renderer, &frame, 48, 48);
    let hidpi = crate::displays::HiDpi {
        logical: (1536, 960),
        native: (1920, 1200),
    };
    let pointer = (100.0, 50.0);
    let hotspot = (6, 4);
    let elements = crate::cursor::elements(
        &mut renderer,
        &CursorImageStatus::Named(CursorIcon::Default),
        Some((&texture, hotspot, None)),
        pointer.into(),
        f64::from(crate::displays::HIDPI_SCALE),
    );
    assert_eq!(elements.len(), 1);
    let geometry = elements[0].geometry(Scale::from(1.0));
    // In the 2x frame: pointer 200, 100 minus the hotspot.
    assert_eq!((geometry.loc.x, geometry.loc.y), (200 - 6, 100 - 4));
    assert_eq!((geometry.size.w, geometry.size.h), (48, 48));
    // The hotspot's pixel, reduced to the display, is the pointer in native pixels.
    let (fx, fy) = hidpi.native_per_logical();
    let hot = (
        f64::from(geometry.loc.x + hotspot.0) / 2.0 * fx,
        f64::from(geometry.loc.y + hotspot.1) / 2.0 * fy,
    );
    assert_eq!(hot, (pointer.0 * 1.25, pointer.1 * 1.25));
}
