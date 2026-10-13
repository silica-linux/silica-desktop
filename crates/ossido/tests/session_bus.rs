//! `--session-bus`: Ossido avvia un bus di sessione privato e lo passa ai client.

#![allow(clippy::expect_used)]

mod common;

use std::process::Command;
use std::time::{Duration, Instant};

use common::Headless;
use serde_json::{Value, json};
use silica_control::commands::{APP_LAUNCH, SYSTEM_DND, SYSTEM_STATUS, SYSTEM_VOLUME};

/// Attende che `path` esista e non sia vuoto.
fn wait_for_file(path: &std::path::Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && !text.is_empty()
        {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "{} non è comparso",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// I nomi presenti sul bus, con `dbus-send`.
fn list_names(address: &str) -> String {
    let output = Command::new("dbus-send")
        .args([
            &format!("--bus={address}"),
            "--print-reply",
            "--dest=org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus.ListNames",
        ])
        .output()
        .expect("dbus-send (pacchetto dbus) è necessario per questo test");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn clients_get_the_private_bus() {
    let ossido = Headless::start_with("session-bus", (320, 240), &["--session-bus"]);
    let file = ossido.dir.join("bus-address");
    ossido
        .client()
        .call(
            APP_LAUNCH,
            json!({
                "command": format!("printf %s \"$DBUS_SESSION_BUS_ADDRESS\" > {}", file.display())
            }),
        )
        .unwrap();
    let address = wait_for_file(&file);
    assert!(address.starts_with("unix:"), "indirizzo: {address}");
    // Il bus è quello di Ossido, non quello della sessione che lo ospita.
    assert_ne!(
        Some(address.as_str()),
        std::env::var("DBUS_SESSION_BUS_ADDRESS").ok().as_deref()
    );
    list_names(&address);

    // Con `cargo test --workspace` silica-daemon è compilato accanto a Ossido: allora deve
    // prendere il nome delle notifiche sul bus privato.
    let daemon = std::path::Path::new(env!("CARGO_BIN_EXE_ossido")).with_file_name("silica-daemon");
    if daemon.is_file() {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !list_names(&address).contains("org.freedesktop.Notifications") {
            assert!(
                Instant::now() < deadline,
                "silica-daemon non ha preso il nome"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[test]
fn without_the_option_clients_keep_the_host_bus() {
    let ossido = Headless::start_with("no-session-bus", (320, 240), &[]);
    let file = ossido.dir.join("bus-address");
    ossido
        .client()
        .call(
            APP_LAUNCH,
            json!({
                "command": format!("printf 'x%s' \"$DBUS_SESSION_BUS_ADDRESS\" > {}", file.display())
            }),
        )
        .unwrap();
    let seen = wait_for_file(&file);
    let host = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default();
    assert_eq!(seen, format!("x{host}"));
}

#[test]
fn system_commands_reach_the_daemon() {
    // Serve silica-daemon accanto a Ossido (cargo test --workspace lo compila).
    let daemon = std::path::Path::new(env!("CARGO_BIN_EXE_ossido")).with_file_name("silica-daemon");
    if !daemon.is_file() {
        eprintln!("silica-daemon non compilato: test saltato");
        return;
    }
    let ossido = Headless::start_with("system", (320, 240), &["--session-bus"]);
    let mut client = ossido.client();
    // Solo Non disturbare: le altre azioni cambierebbero la macchina che esegue i test.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if client.call(SYSTEM_STATUS, Value::Null).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "system.status mai disponibile");
        std::thread::sleep(Duration::from_millis(100));
    }
    let queued = client.call(SYSTEM_DND, json!({"enabled": true})).unwrap();
    assert_eq!(queued["queued"], true);
    loop {
        let status = client.call(SYSTEM_STATUS, Value::Null).unwrap();
        if status["dnd"] == true {
            break;
        }
        assert!(Instant::now() < deadline, "dnd mai attivo: {status}");
        std::thread::sleep(Duration::from_millis(50));
    }
    // Parametri sbagliati: errore, niente in coda.
    assert!(
        client
            .call(SYSTEM_VOLUME, json!({"value": "alto"}))
            .is_err()
    );
    assert!(client.call(SYSTEM_DND, Value::Null).is_err());
}
