//! Avvio di Ossido.
//!
//! ```text
//! ossido [--backend winit|headless] [--size LxA] [--control PORTA] [--spawn COMANDO]...
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
            "--control" => {
                let port = value()?;
                options.control = Some(
                    port.parse()
                        .map_err(|_| format!("porta non valida: {port}"))?,
                );
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
