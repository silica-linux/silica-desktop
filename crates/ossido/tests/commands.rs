//! Every catalog command can be run by Ossido, from the control channel.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::time::{Duration, Instant};

use common::client::{FIRST_WINDOW_AT, TestWindow};
use common::{Headless, assert_outside_window, pixel, screenshot};
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
        // Deliberately wrong parameters: it only matters that Ossido knows the command.
        if let Err(Error::Command(message)) = client.call(spec.id, json!({"_": 0})) {
            assert!(
                !message.contains("unknown command"),
                "{} is not implemented",
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
    let mut first = TestWindow::open(&ossido.wayland, "first");
    let _second = TestWindow::open(&ossido.wayland, "second");

    // The window opened last is on top and has the focus.
    let list = windows(&ossido);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].title.as_deref(), Some("second"));
    assert!(list[0].focused && !list[1].focused);
    let first_id = list[1].id;
    assert_eq!(
        (list[1].x, list[1].y),
        (FIRST_WINDOW_AT as i32, FIRST_WINDOW_AT as i32)
    );
    // The next window cascades by 29 pt (#172).
    assert_eq!(
        (list[0].x, list[0].y),
        (FIRST_WINDOW_AT as i32 + 29, FIRST_WINDOW_AT as i32 + 29)
    );

    let mut client = ossido.client();
    client.call(WINDOW_FOCUS, json!({"id": first_id})).unwrap();
    let list = windows(&ossido);
    assert_eq!((list[0].id, list[0].focused), (first_id, true));

    // Moved via engine.execute: same command, another entry point.
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
    // Halfway along the left side: far from the cut corners (x 200 is the 1 pt rim).
    assert_eq!(pixel(&shot, 201, 160), [255, 0, 0, 255]);
    assert_outside_window(&shot, 199, 160);

    // Without an id, window.close acts on the focused window.
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
        (WINDOW_MOVE, json!({"x": "ten", "y": 0})),
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
            assert!(status.success(), "exit with {status}");
            break;
        }
        assert!(Instant::now() < deadline, "Ossido did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn perf_measures_frame_intervals_and_process_memory() {
    let ossido = Headless::start("perf");
    let mut window = TestWindow::open(&ossido.wayland, "pace");
    let mut client = ossido.client();
    client.call(PERF_FRAMES, json!({"reset": true})).unwrap();

    window.commit_frames(6, Duration::from_millis(40));
    let report: Vec<SurfaceFrames> =
        serde_json::from_value(client.call(PERF_FRAMES, Value::Null).unwrap()).unwrap();
    let frames = report
        .iter()
        .find(|s| s.surface == "window 1")
        .expect("the window has no measurements");
    assert_eq!(frames.frames, 6);
    assert_eq!(frames.pid, Some(std::process::id()));
    // 5 intervals of about 40 ms: all over 1.5 periods at 60 Hz.
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
