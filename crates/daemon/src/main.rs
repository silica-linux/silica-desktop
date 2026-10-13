//! `silica-daemon`: i servizi di sistema di Silica sul bus di sessione.

use std::process::ExitCode;

use silica_daemon::Daemon;

const USAGE: &str = "uso: silica-daemon [--address INDIRIZZO] [--replace]

  --address INDIRIZZO  bus a cui collegarsi (predefinito: DBUS_SESSION_BUS_ADDRESS)
  --replace            prende i nomi anche se un altro server li ha";

fn main() -> ExitCode {
    let mut address = None;
    let mut replace = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => match args.next() {
                Some(value) => address = Some(value),
                None => {
                    eprintln!("silica-daemon: --address richiede un indirizzo\n{USAGE}");
                    return ExitCode::from(2);
                }
            },
            "--replace" => replace = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("silica-daemon: opzione sconosciuta {other}\n{USAGE}");
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
    let daemon = Daemon::serve(connection, replace).await?;
    eprintln!(
        "silica-daemon: notifiche su {}",
        daemon
            .connection()
            .unique_name()
            .map_or("?".to_owned(), |n| n.to_string())
    );
    daemon.run().await
}
