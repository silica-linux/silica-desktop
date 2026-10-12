//! `cargo xtask bench dock`: il banco di prova di M1.5 (`docs/m1.5-dock.md`).
//!
//! Per ogni variante del Dock e ogni ripetizione: Ossido annidato a 2560×1600 (scala 2),
//! lancio del Dock con `app.launch`, puntatore che scorre avanti e indietro sul Dock a 120
//! eventi/s, poi uscita del puntatore per la molla. Le misure vengono da Ossido
//! (`perf.frames`, `perf.process`), uguali per ogni toolkit. Risultati e screenshot in
//! `target/bench/dock/`.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use silica_control::Client;
use silica_control::commands::{
    APP_LAUNCH, INPUT_POINTER_MOVE, PERF_FRAMES, PERF_PROCESS, ProcessInfo, SESSION_QUIT,
    SurfaceFrames,
};

use crate::{Result, cargo_command, flutter, root, status, wslg_socket};

/// Output logico: 1280×800, a scala 2 salvo `--scale`.
const OUTPUT: (f64, f64) = (1280.0, 800.0);
/// Riga delle icone a riposo: da (1280 − 664) / 2 a + 664, centro verticale a 760.
const ROW: (f64, f64) = (308.0, 972.0);
const ROW_Y: f64 = 760.0;
/// Centro a riposo della sesta icona, per lo screenshot ingrandito.
const ICON_6: f64 = 308.0 + 5.0 * 56.0 + 24.0;
const EVENTS_PER_SECOND: f64 = 120.0;
/// Andata e ritorno sulla riga.
const SWEEP_PERIOD: f64 = 2.0;
const SURFACE: &str = "layer dock";

struct Options {
    runs: usize,
    seconds: f64,
    only: Option<String>,
    /// Scala dell'output: la finestra di Ossido è OUTPUT × scala pixel, e deve entrare
    /// nello schermo del compositor ospite (2560×1600 non entra in uno schermo 1920×1200).
    scale: u32,
}

fn options(args: &[String]) -> Result<Options> {
    let mut options = Options {
        runs: 3,
        seconds: 10.0,
        only: None,
        scale: 2,
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
            "--only" => options.only = Some(value()?.clone()),
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

struct Variant {
    name: &'static str,
    description: &'static str,
    command: String,
}

fn variants(root: &Path) -> Result<Vec<Variant>> {
    let release = root.join("target/release");
    let engine = flutter::release_engine_dir()?;
    let bundle = root.join("apps/dock/build/release/flutter_assets");
    for (path, hint) in [
        (
            engine.join("libflutter_engine.so"),
            "cargo xtask engine --release (docs/engine-release.md)",
        ),
        (
            bundle.with_file_name("libapp.so"),
            "cargo xtask bundle dock --release",
        ),
    ] {
        if !path.exists() {
            return Err(format!("{} assente: {hint}", path.display()));
        }
    }
    let embedder = |renderer: &str| {
        format!(
            "env RUST_LOG={} {} --engine {} --renderer {renderer} --layer bottom --anchor left,right,bottom \
             --size 1280x160 --exclusive-zone 72 --namespace dock {}",
            std::env::var("BENCH_CLIENT_LOG").unwrap_or_else(|_| "warn".to_owned()),
            release.join("silica-embedder").display(),
            engine.display(),
            bundle.display()
        )
    };
    let iced = release.join("dock-iced");
    Ok(vec![
        Variant {
            name: "flutter-gl",
            description: "Flutter release AOT, embedder custom, OpenGL ES",
            command: embedder("gl"),
        },
        Variant {
            name: "flutter-gl-impeller",
            description: "Flutter release AOT, embedder custom, Impeller su OpenGL ES",
            command: format!("{} --engine-arg --enable-impeller", embedder("gl")),
        },
        Variant {
            name: "iced-gl",
            description: "iced 0.14, wgpu su OpenGL ES",
            command: format!("env WGPU_BACKEND=gl {}", iced.display()),
        },
        Variant {
            name: "iced-vulkan",
            description: "iced 0.14, wgpu su Vulkan (in WSL è lavapipe, cioè CPU)",
            command: format!("env WGPU_BACKEND=vulkan {}", iced.display()),
        },
        Variant {
            name: "flutter-software",
            description: "Flutter release AOT, renderer software (CPU)",
            command: embedder("software"),
        },
        Variant {
            name: "iced-software",
            description: "iced 0.14, tiny-skia (CPU)",
            command: format!("env ICED_BACKEND=tiny-skia {}", iced.display()),
        },
    ])
}

/// Le misure di una ripetizione.
#[derive(Debug, Clone)]
struct Run {
    startup_ms: f64,
    sweep: SurfaceFrames,
    spring: SurfaceFrames,
    memory: ProcessInfo,
}

pub fn dock(args: &[String]) -> Result<()> {
    let options = options(args)?;
    let root = root()?;
    status(cargo_command(&[
        "build",
        "--release",
        "--package",
        "ossido",
        "--package",
        "silica-embedder",
        "--package",
        "silica-dock-iced",
    ]))?;
    let out = root.join("target/bench/dock");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;

    let mut report = Vec::new();
    for variant in variants(&root)? {
        if options
            .only
            .as_deref()
            .is_some_and(|only| only != variant.name)
        {
            continue;
        }
        let mut runs = Vec::new();
        for run in 0..options.runs {
            eprintln!("xtask: bench {} {}/{}", variant.name, run + 1, options.runs);
            let shots = (run == 0).then_some(out.as_path());
            runs.push(measure(&root, &variant, &options, shots)?);
        }
        report.push((variant, runs));
    }

    let json = Value::Array(
        report
            .iter()
            .map(|(variant, runs)| {
                json!({
                    "variant": variant.name,
                    "description": variant.description,
                    "runs": runs.iter().map(|run| json!({
                        "startup_ms": run.startup_ms,
                        "sweep": run.sweep,
                        "spring": run.spring,
                        "memory": run.memory,
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

/// Ossido annidato per una ripetizione; si chiude quando esce di scena.
struct Session {
    child: Child,
    control: String,
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(mut client) = Client::connect(&self.control) {
            let _ = client.call(SESSION_QUIT, Value::Null);
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_ossido(root: &Path, scale: u32) -> Result<Session> {
    let size = format!("{}x{}", 1280 * scale, 800 * scale);
    let scale = scale.to_string();
    let mut command = Command::new(root.join("target/release/ossido"));
    command
        .args([
            "--backend",
            "winit",
            "--size",
            &size,
            "--scale",
            &scale,
            "--control",
            "0",
        ])
        .env(
            "RUST_LOG",
            std::env::var("BENCH_LOG").unwrap_or_else(|_| "warn".to_owned()),
        )
        .stdout(Stdio::piped());
    if let Some(socket) = wslg_socket() {
        command.env("WAYLAND_DISPLAY", socket);
    }
    if Path::new("/dev/dxg").exists() {
        command.env("GALLIUM_DRIVER", "d3d12");
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Ossido non avviabile: {e}"))?;
    let stdout = child.stdout.take().ok_or("stdout di Ossido assente")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut control = None;
    for line in lines.by_ref() {
        let line = line.map_err(|e| e.to_string())?;
        if let Some(addr) = line.strip_prefix("OSSIDO_CONTROL=") {
            control = Some(addr.trim().to_owned());
            break;
        }
    }
    // Il resto dello stdout si scarta, ma la pipe resta aperta finché Ossido vive.
    std::thread::spawn(move || lines.for_each(drop));
    let control = control.ok_or("Ossido non ha aperto il canale di controllo")?;
    Ok(Session { child, control })
}

fn frames(client: &mut Client, reset: bool) -> Result<Option<SurfaceFrames>> {
    let value = client
        .call(PERF_FRAMES, json!({ "reset": reset }))
        .map_err(|e| e.to_string())?;
    let list: Vec<SurfaceFrames> = serde_json::from_value(value).map_err(|e| e.to_string())?;
    Ok(list.into_iter().find(|s| s.surface == SURFACE))
}

fn pointer(client: &mut Client, x: f64, y: f64) -> Result<()> {
    client
        .call(INPUT_POINTER_MOVE, json!({ "x": x, "y": y }))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn screenshot(client: &mut Client, path: &Path) -> Result<()> {
    client
        .call(
            silica_control::commands::UI_SCREENSHOT,
            json!({ "path": path.to_string_lossy() }),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn measure(root: &Path, variant: &Variant, options: &Options, shots: Option<&Path>) -> Result<Run> {
    let session = start_ossido(root, options.scale)?;
    let mut client = Client::connect(&session.control).map_err(|e| e.to_string())?;
    pointer(&mut client, OUTPUT.0 / 2.0, OUTPUT.1 / 2.0)?;
    client
        .call(APP_LAUNCH, json!({ "command": variant.command }))
        .map_err(|e| e.to_string())?;

    // Avvio: dal lancio al primo buffer del Dock, misurato da Ossido.
    let deadline = Instant::now() + Duration::from_secs(30);
    let first = loop {
        if let Some(found) = frames(&mut client, false)? {
            break found;
        }
        if Instant::now() > deadline {
            return Err(format!("{}: il Dock non è comparso in 30 s", variant.name));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let startup_ms = first
        .startup_ms
        .ok_or("tempo di avvio assente: il Dock non discende dal processo lanciato")?;
    let pid = first.pid.ok_or("pid del Dock assente")?;
    std::thread::sleep(Duration::from_secs(2));
    if let Some(dir) = shots {
        screenshot(&mut client, &dir.join(format!("{}-rest.png", variant.name)))?;
    }

    // Scorrimento: onda triangolare sulla riga, 120 eventi al secondo.
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
        pointer(&mut client, ROW.0 + along * (ROW.1 - ROW.0), ROW_Y)?;
        next += tick;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    let sweep = frames(&mut client, true)?.ok_or("misure dello scorrimento assenti")?;
    if sweep.frames < 2 {
        return Err(format!(
            "{}: {} frame durante lo scorrimento, il Dock non risponde al puntatore",
            variant.name, sweep.frames
        ));
    }
    let memory: ProcessInfo = serde_json::from_value(
        client
            .call(PERF_PROCESS, json!({ "pid": pid }))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    // Molla: il puntatore esce, il Dock torna a riposo da solo.
    pointer(&mut client, OUTPUT.0 / 2.0, OUTPUT.1 / 2.0)?;
    std::thread::sleep(Duration::from_millis(1500));
    let spring = frames(&mut client, true)?.ok_or("misure della molla assenti")?;

    if let Some(dir) = shots {
        pointer(&mut client, ICON_6, ROW_Y)?;
        std::thread::sleep(Duration::from_millis(1500));
        screenshot(
            &mut client,
            &dir.join(format!("{}-hover.png", variant.name)),
        )?;
    }
    let _ = Command::new("kill").arg(pid.to_string()).status();
    drop(client);
    drop(session);
    Ok(Run {
        startup_ms,
        sweep,
        spring,
        memory,
    })
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    match (values.len() % 2, values.get(mid), mid.checked_sub(1)) {
        (1, Some(v), _) => *v,
        (_, Some(v), Some(before)) => values.get(before).map_or(*v, |b| (b + v) / 2.0),
        _ => 0.0,
    }
}

/// Tabella Markdown con la mediana delle ripetizioni.
fn table(report: &[(Variant, Vec<Run>)]) -> String {
    let mut lines = vec![
        "| Variante | Avvio (ms) | Frame | Media (ms) | p50 | p95 | p99 | Max | Lenti | Molla: frame | Molla: max | PSS (MiB) | RSS (MiB) |".to_owned(),
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |".to_owned(),
    ];
    for (variant, runs) in report {
        let m = |f: &dyn Fn(&Run) -> f64| median(runs.iter().map(f).collect());
        lines.push(format!(
            "| {} | {:.0} | {:.0} | {:.2} | {:.2} | {:.2} | {:.2} | {:.1} | {:.0} | {:.0} | {:.1} | {:.1} | {:.1} |",
            variant.name,
            m(&|r| r.startup_ms),
            m(&|r| r.sweep.frames as f64),
            m(&|r| r.sweep.mean_ms),
            m(&|r| r.sweep.p50_ms),
            m(&|r| r.sweep.p95_ms),
            m(&|r| r.sweep.p99_ms),
            m(&|r| r.sweep.max_ms),
            m(&|r| r.sweep.slow as f64),
            m(&|r| r.spring.frames as f64),
            m(&|r| r.spring.max_ms),
            m(&|r| r.memory.pss_kib as f64 / 1024.0),
            m(&|r| r.memory.rss_kib as f64 / 1024.0),
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_odd_and_even() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), 2.5);
        assert_eq!(median(vec![]), 0.0);
    }

    #[test]
    fn row_constants_follow_the_spec() {
        assert_eq!(ROW.1 - ROW.0, 664.0);
        assert_eq!(ICON_6, 612.0);
    }
}
