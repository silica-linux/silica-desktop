//! Shared tools of the test benches (`bench_glass.rs`): nested Ossido with the control
//! channel, frame measurements, synthetic pointer, median.
//!
//! The M1.5 Dock bench (Flutter against iced, `docs/gate1-dock.md`) was removed with the
//! prototypes after Gate 1; its measurements remain in the documents.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use silica_control::Client;
use silica_control::commands::{INPUT_POINTER_MOVE, PERF_FRAMES, SESSION_QUIT, SurfaceFrames};

use crate::{Result, wslg_socket};

/// Logical output: 1280×800, multiplied by `--scale`.
#[cfg(test)]
const OUTPUT: (f64, f64) = (1280.0, 800.0);
/// Band swept by the pointer over the Dock icons, and its height.
pub(crate) const ROW: (f64, f64) = (308.0, 972.0);
pub(crate) const ROW_Y: f64 = 760.0;
pub(crate) const EVENTS_PER_SECOND: f64 = 120.0;
/// Back and forth along the row.
pub(crate) const SWEEP_PERIOD: f64 = 2.0;
const SURFACE: &str = "layer dock";

/// Nested Ossido for one repetition; it closes when it goes out of scope.
pub(crate) struct Session {
    child: Child,
    pub(crate) control: String,
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

pub(crate) fn start_ossido(root: &Path, scale: u32, extra: &[&str]) -> Result<Session> {
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
        .args(extra)
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
        .map_err(|e| format!("cannot start Ossido: {e}"))?;
    let stdout = child.stdout.take().ok_or("Ossido's stdout missing")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut control = None;
    for line in lines.by_ref() {
        let line = line.map_err(|e| e.to_string())?;
        if let Some(addr) = line.strip_prefix("OSSIDO_CONTROL=") {
            control = Some(addr.trim().to_owned());
            break;
        }
    }
    // The rest of stdout is discarded, but the pipe stays open while Ossido lives.
    std::thread::spawn(move || lines.for_each(drop));
    let control = control.ok_or("Ossido did not open the control channel")?;
    Ok(Session { child, control })
}

pub(crate) fn frames(client: &mut Client, reset: bool) -> Result<Option<SurfaceFrames>> {
    let value = client
        .call(PERF_FRAMES, json!({ "reset": reset }))
        .map_err(|e| e.to_string())?;
    let list: Vec<SurfaceFrames> = serde_json::from_value(value).map_err(|e| e.to_string())?;
    Ok(list.into_iter().find(|s| s.surface == SURFACE))
}

pub(crate) fn pointer(client: &mut Client, x: f64, y: f64) -> Result<()> {
    client
        .call(INPUT_POINTER_MOVE, json!({ "x": x, "y": y }))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub(crate) fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    match (values.len() % 2, values.get(mid), mid.checked_sub(1)) {
        (1, Some(v), _) => *v,
        (_, Some(v), Some(before)) => values.get(before).map_or(*v, |b| (b + v) / 2.0),
        _ => 0.0,
    }
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
    fn the_sweep_stays_inside_the_output() {
        assert!(ROW.0 > 0.0 && ROW.1 < OUTPUT.0 && ROW_Y < OUTPUT.1);
    }
}
