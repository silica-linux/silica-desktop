//! `cargo xtask bench glass`: il costo del vetro per Ossido, per livello di qualità.
//!
//! Caso peggiore: il Dock Flutter (vetro chiaro, Impeller) con una finestra che scorre
//! dietro di lui a ogni frame, così lo sfondo del vetro cambia sempre, e il puntatore che
//! passa sulle icone. Per ogni livello si misurano il tempo di rendering di Ossido
//! (`perf.render`, backend winit) e il ritmo del Dock (`perf.frames`). Va lanciato in una
//! sessione grafica con la GPU (Ossido annidato).

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
        let mut value = || args.next().ok_or(format!("{arg} richiede un valore"));
        match arg.as_str() {
            "--runs" => {
                options.runs = value()?
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or("--runs vuole un intero positivo")?;
            }
            "--seconds" => {
                options.seconds = value()?
                    .parse()
                    .ok()
                    .filter(|s: &f64| *s > 0.0)
                    .ok_or("--seconds vuole un numero positivo")?;
            }
            "--scale" => {
                options.scale = value()?
                    .parse()
                    .ok()
                    .filter(|s| (1..=4).contains(s))
                    .ok_or("--scale vuole un intero da 1 a 4")?;
            }
            other => return Err(format!("argomento sconosciuto: {other}")),
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
    let dock = root.join("apps/dock/build/release/flutter_assets");
    let demo = root.join("apps/demo/build/flutter_assets");
    for (path, hint) in [
        (
            dock.with_file_name("libapp.so"),
            "cargo xtask bundle dock --release",
        ),
        (demo.join("kernel_blob.bin"), "cargo xtask bundle demo"),
    ] {
        if !path.exists() {
            return Err(format!("{} assente: {hint}", path.display()));
        }
    }
    let embedder = root.join("target/release/silica-embedder");
    let dock_command = format!(
        "{} --engine {} --renderer impeller --layer top --anchor left,right,bottom \
         --size 1280x160 --exclusive-zone 72 --namespace dock {}",
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
    eprintln!("xtask: risultati in {}", results.display());
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

    // Si aspettano il Dock e la finestra della demo.
    let deadline = Instant::now() + Duration::from_secs(30);
    let window = loop {
        let list: Vec<WindowInfo> =
            serde_json::from_value(call(&mut client, WINDOW_LIST, Value::Null)?)
                .map_err(|e| e.to_string())?;
        if let (Some(window), Some(_)) = (list.into_iter().next(), frames(&mut client, false)?) {
            break window;
        }
        if Instant::now() > deadline {
            return Err("Dock o finestra della demo non comparsi in 30 s".to_owned());
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
        // La finestra scorre dietro il Dock (bordo inferiore oltre la riga delle icone).
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
    let dock = frames(&mut client, true)?.ok_or("misure del Dock assenti")?;
    if render.frames < 2 {
        return Err("Ossido non ha misurato i rendering: serve il backend winit".to_owned());
    }
    drop(client);
    drop(session);
    Ok(Run { render, dock })
}

fn table(report: &[(&str, Vec<Run>)]) -> String {
    let mut lines = vec![
        "| Vetro | Render Ossido: frame | media (ms) | p95 | p99 | max | Dock: frame | p99 | lenti |".to_owned(),
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
