//! Ogni comando del catalogo è eseguibile da Ossido, dal canale di controllo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::time::{Duration, Instant};

use common::client::{FIRST_WINDOW_AT, TestWindow};
use common::{Headless, assert_clear_color, pixel, screenshot};
use serde_json::{Value, json};
use silica_control::commands::{
    ENGINE_COMMANDS, ENGINE_EXECUTE, PERF_FRAMES, PERF_PROCESS, ProcessInfo, SESSION_QUIT,
    SurfaceFrames, WINDOW_CLOSE, WINDOW_FOCUS, WINDOW_LIST, WINDOW_MOVE, WindowInfo,
};
use silica_control::{CATALOG, Error};

fn windows(ossido: &Headless) -> Vec<WindowInfo> {
    let list = ossido.client().call(WINDOW_LIST, Value::Null).unwrap();
    serde_json::from_value(list).unwrap()
}

#[test]
fn every_catalog_command_is_implemented() {
    let ossido = Headless::start("catalog");
    let mut client = ossido.client();
    for spec in CATALOG.iter().filter(|spec| spec.id != SESSION_QUIT) {
        // Parametri volutamente sbagliati: conta solo che Ossido conosca il comando.
        if let Err(Error::Command(message)) = client.call(spec.id, json!({"_": 0})) {
            assert!(
                !message.contains("comando sconosciuto"),
                "{} non è implementato",
                spec.id
            );
        }
    }
}

#[test]
fn engine_commands_returns_the_catalog() {
    let ossido = Headless::start("engine-commands");
    let catalog = ossido.client().call(ENGINE_COMMANDS, Value::Null).unwrap();
    let ids: Vec<&str> = catalog
        .as_array()
        .unwrap()
        .iter()
        .map(|spec| spec["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, CATALOG.iter().map(|spec| spec.id).collect::<Vec<_>>());
}

#[test]
fn window_list_focus_move_and_close() {
    let ossido = Headless::start("windows");
    let mut first = TestWindow::open(&ossido.wayland, "prima");
    let _second = TestWindow::open(&ossido.wayland, "seconda");

    // La finestra aperta per ultima è in cima e ha il focus.
    let list = windows(&ossido);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].title.as_deref(), Some("seconda"));
    assert!(list[0].focused && !list[1].focused);
    let first_id = list[1].id;
    assert_eq!(
        (list[1].x, list[1].y),
        (FIRST_WINDOW_AT as i32, FIRST_WINDOW_AT as i32)
    );

    let mut client = ossido.client();
    client.call(WINDOW_FOCUS, json!({"id": first_id})).unwrap();
    let list = windows(&ossido);
    assert_eq!((list[0].id, list[0].focused), (first_id, true));

    // Spostata via engine.execute: stesso comando, altro ingresso.
    client
        .call(
            ENGINE_EXECUTE,
            json!({"id": WINDOW_MOVE, "params": {"x": 200, "y": 130}}),
        )
        .unwrap();
    let moved = windows(&ossido)
        .into_iter()
        .find(|w| w.id == first_id)
        .unwrap();
    assert_eq!((moved.x, moved.y), (200, 130));
    let shot = screenshot(&ossido);
    // A metà del lato sinistro: lontano dagli angoli tagliati.
    assert_eq!(pixel(&shot, 200, 160), [255, 0, 0, 255]);
    assert_clear_color(&shot, 199, 160);

    // Senza id, window.close agisce sulla finestra con il focus.
    assert!(!first.close_requested());
    client.call(WINDOW_CLOSE, Value::Null).unwrap();
    assert!(first.close_requested());
}

#[test]
fn bad_targets_and_params_are_errors() {
    let ossido = Headless::start("errors");
    let mut client = ossido.client();
    for (command, params) in [
        (WINDOW_CLOSE, Value::Null),
        (WINDOW_FOCUS, json!({"id": 99})),
        (WINDOW_MOVE, json!({"x": "dieci", "y": 0})),
        (ENGINE_EXECUTE, json!({"id": ENGINE_EXECUTE})),
    ] {
        assert!(
            matches!(client.call(command, params.clone()), Err(Error::Command(_))),
            "{command} {params}"
        );
    }
}

#[test]
fn session_quit_stops_ossido() {
    let mut ossido = Headless::start("quit");
    ossido.client().call(SESSION_QUIT, Value::Null).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = ossido.try_wait() {
            assert!(status.success(), "uscita con {status}");
            break;
        }
        assert!(Instant::now() < deadline, "Ossido non è uscito");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn perf_measures_frame_intervals_and_process_memory() {
    let ossido = Headless::start("perf");
    let mut window = TestWindow::open(&ossido.wayland, "ritmo");
    let mut client = ossido.client();
    client.call(PERF_FRAMES, json!({"reset": true})).unwrap();

    window.commit_frames(6, Duration::from_millis(40));
    let report: Vec<SurfaceFrames> =
        serde_json::from_value(client.call(PERF_FRAMES, Value::Null).unwrap()).unwrap();
    let frames = report
        .iter()
        .find(|s| s.surface == "window 1")
        .expect("la finestra non ha misure");
    assert_eq!(frames.frames, 6);
    assert_eq!(frames.pid, Some(std::process::id()));
    // 5 intervalli da circa 40 ms: tutti oltre 1,5 periodi a 60 Hz.
    assert!(frames.p50_ms >= 35.0 && frames.p50_ms < 200.0, "{frames:?}");
    assert_eq!(frames.slow, 5);
    assert!(frames.startup_ms.is_none());

    let info: ProcessInfo = serde_json::from_value(
        client
            .call(PERF_PROCESS, json!({"pid": std::process::id()}))
            .unwrap(),
    )
    .unwrap();
    assert!(info.pss_kib > 0 && info.rss_kib >= info.pss_kib);
    assert!(client.call(PERF_PROCESS, json!({"pid": u32::MAX})).is_err());
}
