//! `cargo xtask bench glass`: the cost of the glass for Ossido, per quality level.
//!
//! Worst case: the shell's Dock (Liquid Glass, Impeller) with a window scrolling behind it
//! on every frame, so the glass background always changes, and the pointer passing over
//! the icons. For each level it measures Ossido's rendering time (`perf.render`, winit
//! backend) and the Dock's pace (`perf.frames`). Run it in a graphical session with the GPU
//! (nested Ossido).

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use silica_control::Client;
use silica_control::commands::{
    APP_LAUNCH, PERF_RENDER, RenderTimes, UI_GLASS_QUALITY, WINDOW_LIST, WINDOW_MOVE, WindowInfo,
};

use crate::bench::{
    EVENTS_PER_SECOND, ROW, ROW_Y, SWEEP_PERIOD, frames, median, pointer, start_ossido,
};
use crate::{Result, cargo_command, flutter, root, status};

const QUALITIES: [&str; 3] = ["full", "frosted", "opaque"];

struct Options {
    runs: usize,
    seconds: f64,
    scale: u32,
}

fn options(args: &[String]) -> Result<Options> {
    let mut options = Options {
        runs: 3,
        seconds: 5.0,
        scale: 1,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--runs" => {
                options.runs = value()?
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or("--runs wants a positive integer")?;
            }
            "--seconds" => {
                options.seconds = value()?
                    .parse()
                    .ok()
                    .filter(|s: &f64| *s > 0.0)
                    .ok_or("--seconds wants a positive number")?;
            }
            "--scale" => {
                options.scale = value()?
                    .parse()
                    .ok()
                    .filter(|s| (1..=4).contains(s))
                    .ok_or("--scale wants an integer from 1 to 4")?;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(options)
}

struct Run {
    render: RenderTimes,
    dock: silica_control::commands::SurfaceFrames,
}

pub fn glass(args: &[String]) -> Result<()> {
    let options = options(args)?;
    let root = root()?;
    status(cargo_command(&[
        "build",
        "--release",
        "--package",
        "ossido",
        "--package",
        "silica-embedder",
    ]))?;
    let release_engine = flutter::release_engine_dir()?;
    let debug_engine = flutter::engine_dir()?;
    let dock = root.join("shell/build/release/flutter_assets");
    let demo = root.join("testapps/demo/build/flutter_assets");
    for (path, hint) in [
        (
            dock.with_file_name("libapp.so"),
            "cargo xtask bundle shell --release",
        ),
        (demo.join("kernel_blob.bin"), "cargo xtask bundle demo"),
    ] {
        if !path.exists() {
            return Err(format!("{} missing: {hint}", path.display()));
        }
    }
    let embedder = root.join("target/release/silica-embedder");
    let dock_command = format!(
        "{} --engine {} --renderer impeller --layer top --anchor left,right,bottom \
         --size 1280x200 --exclusive-zone 84 --namespace dock --dart-arg dock-demo {}",
        embedder.display(),
        release_engine.display(),
        dock.display()
    );
    let demo_command = format!(
        "{} --engine {} --renderer impeller --size 600x300 {}",
        embedder.display(),
        debug_engine.display(),
        demo.display()
    );

    let mut report: Vec<(&str, Vec<Run>)> = Vec::new();
    for quality in QUALITIES {
        let mut runs = Vec::new();
        for run in 0..options.runs {
            eprintln!("xtask: bench glass {quality} {}/{}", run + 1, options.runs);
            runs.push(measure(
                &root,
                quality,
                &options,
                &dock_command,
                &demo_command,
            )?);
        }
        report.push((quality, runs));
    }

    let out = root.join("target/bench/glass");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let json = Value::Array(
        report
            .iter()
            .map(|(quality, runs)| {
                json!({
                    "quality": quality,
                    "runs": runs.iter().map(|run| json!({
                        "render": run.render,
                        "dock": run.dock,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    );
    let results = out.join("results.json");
    std::fs::write(
        &results,
        serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("{}: {e}", results.display()))?;
    println!("{}", table(&report));
    eprintln!("xtask: results in {}", results.display());
    Ok(())
}

fn call(client: &mut Client, command: &str, params: Value) -> Result<Value> {
    client.call(command, params).map_err(|e| e.to_string())
}

fn measure(
    root: &std::path::Path,
    quality: &str,
    options: &Options,
    dock_command: &str,
    demo_command: &str,
) -> Result<Run> {
    let session = start_ossido(root, options.scale, &["--background", "7a7b80"])?;
    let mut client = Client::connect(&session.control).map_err(|e| e.to_string())?;
    call(&mut client, APP_LAUNCH, json!({ "command": dock_command }))?;
    call(&mut client, APP_LAUNCH, json!({ "command": demo_command }))?;
    call(&mut client, UI_GLASS_QUALITY, json!({ "quality": quality }))?;

    // Wait for the Dock and the demo window.
    let deadline = Instant::now() + Duration::from_secs(30);
    let window = loop {
        let list: Vec<WindowInfo> =
            serde_json::from_value(call(&mut client, WINDOW_LIST, Value::Null)?)
                .map_err(|e| e.to_string())?;
        if let (Some(window), Some(_)) = (list.into_iter().next(), frames(&mut client, false)?) {
            break window;
        }
        if Instant::now() > deadline {
            return Err("Dock or demo window did not appear within 30 s".to_owned());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    std::thread::sleep(Duration::from_secs(2));

    call(&mut client, PERF_RENDER, json!({ "reset": true }))?;
    frames(&mut client, true)?;
    let start = Instant::now();
    let tick = Duration::from_secs_f64(1.0 / EVENTS_PER_SECOND);
    let mut next = start;
    while start.elapsed().as_secs_f64() < options.seconds {
        let phase = (start.elapsed().as_secs_f64() / SWEEP_PERIOD).fract();
        let along = if phase < 0.5 {
            phase * 2.0
        } else {
            2.0 - phase * 2.0
        };
        // The window scrolls behind the Dock (bottom edge past the icon row).
        let x = (along * 600.0).round() as i64;
        call(
            &mut client,
            WINDOW_MOVE,
            json!({ "id": window.id, "x": x, "y": 520 }),
        )?;
        pointer(&mut client, ROW.0 + along * (ROW.1 - ROW.0), ROW_Y)?;
        next += tick;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    let render: RenderTimes =
        serde_json::from_value(call(&mut client, PERF_RENDER, json!({ "reset": true }))?)
            .map_err(|e| e.to_string())?;
    let dock = frames(&mut client, true)?.ok_or("Dock measurements missing")?;
    if render.frames < 2 {
        return Err("Ossido did not measure the renders: the winit backend is needed".to_owned());
    }
    drop(client);
    drop(session);
    Ok(Run { render, dock })
}

fn table(report: &[(&str, Vec<Run>)]) -> String {
    let mut lines = vec![
        "| Glass | Ossido render: frames | mean (ms) | p95 | p99 | max | Dock: frames | p99 | slow |".to_owned(),
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |".to_owned(),
    ];
    for (quality, runs) in report {
        let m = |f: &dyn Fn(&Run) -> f64| median(runs.iter().map(f).collect());
        lines.push(format!(
            "| {quality} | {:.0} | {:.2} | {:.2} | {:.2} | {:.1} | {:.0} | {:.2} | {:.0} |",
            m(&|r| r.render.frames as f64),
            m(&|r| r.render.mean_ms),
            m(&|r| r.render.p95_ms),
            m(&|r| r.render.p99_ms),
            m(&|r| r.render.max_ms),
            m(&|r| r.dock.frames as f64),
            m(&|r| r.dock.p99_ms),
            m(&|r| r.dock.slow as f64),
        ));
    }
    lines.join("\n")
}
