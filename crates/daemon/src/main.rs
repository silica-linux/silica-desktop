//! `silica-daemon`: Silica's system services on the session bus.

use std::process::ExitCode;

use futures_lite::{StreamExt, future};
use silica_daemon::{Daemon, SystemConfig, SystemService};

const USAGE: &str = "usage: silica-daemon [--address ADDRESS] [--replace]

  --address ADDRESS    bus to connect to (default: DBUS_SESSION_BUS_ADDRESS)
  --replace            takes the names even if another server holds them";

fn main() -> ExitCode {
    let mut address = None;
    let mut replace = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => match args.next() {
                Some(value) => address = Some(value),
                None => {
                    eprintln!("silica-daemon: --address needs an address\n{USAGE}");
                    return ExitCode::from(2);
                }
            },
            "--replace" => replace = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("silica-daemon: unknown option {other}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match zbus::block_on(run(address, replace)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("silica-daemon: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run(address: Option<String>, replace: bool) -> zbus::Result<()> {
    let builder = match address {
        Some(address) => zbus::connection::Builder::address(address.as_str())?,
        None => zbus::connection::Builder::session()?,
    };
    let connection = builder.build().await?;
    // The system bus is for the Control Center; without it, those sources are absent.
    let system_bus = match zbus::Connection::system().await {
        Ok(bus) => Some(bus),
        Err(err) => {
            eprintln!("silica-daemon: system bus unavailable: {err}");
            None
        }
    };
    let system = SystemService::serve(
        connection.clone(),
        SystemConfig {
            system_bus,
            ..SystemConfig::default()
        },
    )
    .await?;
    let daemon = Daemon::serve(connection, replace).await?;
    eprintln!(
        "silica-daemon: notifiche su {}",
        daemon
            .connection()
            .unique_name()
            .map_or("?".to_owned(), |n| n.to_string())
    );
    // Ossido's private bus closes with Ossido: then the daemon exits.
    let lost = async {
        let mut messages = zbus::MessageStream::from(daemon.connection());
        while let Some(Ok(_)) = messages.next().await {}
        Err(zbus::Error::Failure("session bus closed".to_owned()))
    };
    future::or(future::or(daemon.run(), system.run()), lost).await
}
