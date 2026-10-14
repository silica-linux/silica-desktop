//! L'uscita forzata: `app.force_quit` termina i processi delle finestre di un'app, e solo quelli.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::process::Command;

use common::Headless;
use common::client::{APP_ID, TestWindow};
use serde_json::json;
use silica_control::commands::{APP_FORCE_QUIT, WINDOW_LIST, WindowInfo};

const CHILD_SOCKET: &str = "SILICA_TEST_FORCE_QUIT_SOCKET";

/// Il figlio: apre una finestra dell'app "dev.prova.Bloccata" e aspetta.
#[test]
fn force_quit_child() {
    if let Ok(socket) = std::env::var(CHILD_SOCKET) {
        APP_ID.with(|id| *id.borrow_mut() = "dev.prova.Bloccata".to_owned());
        let _window = TestWindow::open(std::path::Path::new(&socket), "bloccata");
        std::thread::sleep(std::time::Duration::from_secs(20));
    }
}

#[test]
fn force_quit_kills_only_the_processes_of_that_app() {
    let ossido = Headless::start("force-quit");
    let _mine = TestWindow::open(&ossido.wayland, "mia");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "force_quit_child",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_SOCKET, &ossido.wayland)
        .spawn()
        .unwrap();
    let mut client = ossido.client();
    // La finestra del figlio arriva.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let list: Vec<WindowInfo> =
            serde_json::from_value(client.call(WINDOW_LIST, serde_json::Value::Null).unwrap())
                .unwrap();
        if list.iter().any(|w| w.title.as_deref() == Some("bloccata")) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "la finestra del figlio non arriva"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let reply = client
        .call(APP_FORCE_QUIT, json!({"app_id": "dev.prova.Bloccata"}))
        .unwrap();
    assert_eq!(reply["killed"], 1);
    let status = child.wait().unwrap();
    assert!(!status.success(), "il figlio doveva essere terminato");
    // Un'app che non c'è: errore, nessun processo toccato (il test è ancora vivo).
    assert!(
        client
            .call(APP_FORCE_QUIT, json!({"app_id": "dev.prova.Nessuna"}))
            .is_err()
    );
}
