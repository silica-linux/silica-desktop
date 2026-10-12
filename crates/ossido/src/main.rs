//! Avvio di Ossido.
//!
//! ```text
//! ossido [--backend winit|headless] [--size LxA] [--scale N] [--control PORTA]
//!     [--background RRGGBB] [--animation-scale F] [--spawn COMANDO]...
//! ```

use std::process::ExitCode;

use ossido::{BackendKind, Options};
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_writer(std::io::stderr)
        .init();

    let options = match parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("ossido: {err}");
            return ExitCode::from(2);
        }
    };
    match ossido::run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} richiede un valore"));
        match arg.as_str() {
            "--backend" => {
                options.backend = match value()?.as_str() {
                    "winit" => BackendKind::Winit,
                    "headless" => BackendKind::Headless,
                    other => return Err(format!("backend sconosciuto: {other}")),
                }
            }
            "--size" => {
                let size = value()?;
                let (w, h) = size
                    .split_once('x')
                    .ok_or(format!("dimensione non valida: {size}"))?;
                let parse = |n: &str| n.parse::<i32>().ok().filter(|n| (1..=16384).contains(n));
                options.size = parse(w)
                    .zip(parse(h))
                    .ok_or(format!("dimensione non valida: {size}"))?;
            }
            "--scale" => {
                let scale = value()?;
                options.scale = scale
                    .parse()
                    .ok()
                    .filter(|s| (1..=4).contains(s))
                    .ok_or(format!("scala non valida (1-4): {scale}"))?;
            }
            "--control" => {
                let port = value()?;
                options.control = Some(
                    port.parse()
                        .map_err(|_| format!("porta non valida: {port}"))?,
                );
            }
            "--animation-scale" => {
                let text = value()?;
                options.animation_scale = text
                    .parse::<f64>()
                    .ok()
                    .filter(|scale| scale.is_finite() && (0.0..=1000.0).contains(scale))
                    .ok_or(format!(
                        "scala delle animazioni non valida (0–1000): {text}"
                    ))?;
            }
            "--background" => {
                let hex = value()?;
                options.background =
                    parse_color(&hex).ok_or(format!("colore non valido (RRGGBB): {hex}"))?;
            }
            "--spawn" => {
                let command = value()?;
                options.spawn.push(command);
            }
            _ => return Err(format!("argomento sconosciuto: {arg}")),
        }
    }
    Ok(options)
}

/// `RRGGBB` (con o senza `#`) in RGBA 0–1.
fn parse_color(text: &str) -> Option<[f32; 4]> {
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |at: usize| {
        hex.get(at..at + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .map(|value| f32::from(value) / 255.0)
    };
    Some([channel(0)?, channel(2)?, channel(4)?, 1.0])
}

#[cfg(test)]
mod tests {
    use super::parse_color;

    #[test]
    fn colors_parse_from_hex() {
        assert_eq!(parse_color("#ff0000"), Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(parse_color("00ff00"), Some([0.0, 1.0, 0.0, 1.0]));
        assert_eq!(parse_color("#7a7b8"), None);
        assert_eq!(parse_color("zzzzzz"), None);
        assert_eq!(parse_color("éééé"), None);
    }
}
