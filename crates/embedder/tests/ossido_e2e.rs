//! The demo app runs in the embedder inside headless Ossido: first frame, click, HiDPI, close.
//!
//! The engine and the bundle are needed: `cargo xtask engine` and `cargo xtask bundle demo`
//! (`cargo xtask ci` prepares them by itself).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use silica_control::commands::{
    INPUT_CLICK, INPUT_KEY, INPUT_TYPE, LAYER_LIST, LayerInfo, WINDOW_CLOSE, WINDOW_LIST,
    WindowInfo,
};
use silica_control::{Client, ScreenshotParams};

const IDLE: [u8; 4] = [0x33, 0x66, 0xCC, 0xFF];
const TAPPED: [u8; 4] = [0xCC, 0x66, 0x33, 0xFF];
const TYPED: [u8; 4] = [0x33, 0xCC, 0x66, 0xFF];
const LO_DPI: [u8; 4] = [0xFF, 0x00, 0xFF, 0xFF];
const HI_DPI: [u8; 4] = [0x00, 0xFF, 0x00, 0xFF];
const APP_ID: &str = "dev.frascella.silica.Flutter";
const CHILD_ENV: &str = "SILICA_E2E_OSSIDO";
const TIMEOUT: Duration = Duration::from_secs(20);

/// Not a test: it is Ossido itself, when this binary is relaunched by `Session`.
#[test]
fn ossido_child_process() {
    let Ok(config) = std::env::var(CHILD_ENV) else {
        return;
    };
    let (size, scale) = config.split_once('@').unwrap();
    let (w, h) = size.split_once('x').unwrap();
    ossido::run(ossido::Options {
        backend: ossido::BackendKind::Headless,
        size: (w.parse().unwrap(), h.parse().unwrap()),
        scale: scale.parse().unwrap(),
        control: Some(0),
        // Deterministic screenshots: windows appear right away at full size.
        animation_scale: 0.0,
        ..Default::default()
    })
    .unwrap();
    std::process::exit(0);
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A child process that dies with the test, even if the test panics.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Session {
    dir: PathBuf,
    _ossido: KillOnDrop,
    embedder: Option<Child>,
    others: Vec<KillOnDrop>,
    control: String,
    wayland: String,
    output: (u32, u32),
}

impl Session {
    /// Headless Ossido of `output` physical pixels at scale `scale`, with the demo app in a
    /// window of `window` logical pixels.
    fn start(name: &str, output: (u32, u32), scale: u32, window: (u32, u32)) -> Self {
        Self::start_with(name, output, scale, &size_args(window))
    }

    /// Like `start`, with embedder arguments of choice (for example a layer).
    fn start_with(name: &str, output: (u32, u32), scale: u32, args: &[String]) -> Self {
        let engine = silica_embedder::default_engine_dir();
        let bundle = repo().join("testapps/demo/build/flutter_assets");
        assert!(
            engine.join("libflutter_engine.so").exists() && bundle.join("kernel_blob.bin").exists(),
            "the engine and the bundle are needed: cargo xtask engine && cargo xtask bundle demo"
        );

        let dir = std::env::temp_dir().join(format!("silica-e2e-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        let mut ossido = KillOnDrop(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "ossido_child_process",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD_ENV, format!("{}x{}@{scale}", output.0, output.1))
                .env("XDG_RUNTIME_DIR", &dir)
                .env_remove("WAYLAND_DISPLAY")
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut wayland = None;
        let mut control = None;
        // libtest writes "test ossido_child_process ... " on the same line as Ossido's first
        // println!: the variable is searched anywhere in the line.
        let value = |line: &str, key: &str| line.split_once(key).map(|(_, v)| v.trim().to_owned());
        for line in BufReader::new(ossido.0.stdout.take().unwrap()).lines() {
            let line = line.unwrap();
            if let Some(name) = value(&line, "WAYLAND_DISPLAY=") {
                wayland = Some(name);
            }
            if let Some(addr) = value(&line, "OSSIDO_CONTROL=") {
                control = Some(addr);
                break;
            }
        }

        let wayland = wayland.expect("Ossido without a Wayland socket");
        let embedder = spawn_embedder(&dir, &wayland, args);
        Self {
            dir,
            _ossido: ossido,
            embedder: Some(embedder),
            others: Vec::new(),
            control: control.expect("Ossido without a control channel"),
            wayland,
            output,
        }
    }

    /// Another instance of the demo app in the same session.
    fn spawn(&mut self, args: &[String]) {
        self.others
            .push(KillOnDrop(spawn_embedder(&self.dir, &self.wayland, args)));
    }

    fn layer(&self, namespace: &str) -> LayerInfo {
        wait_for("the layer", || {
            let list = self.client().call(LAYER_LIST, Value::Null).unwrap();
            let layers: Vec<LayerInfo> = serde_json::from_value(list).unwrap();
            layers
                .into_iter()
                .find(|l| l.namespace == namespace && l.width > 0 && l.height > 0)
        })
    }

    fn client(&self) -> Client {
        Client::connect(&self.control).unwrap()
    }

    fn window(&self) -> WindowInfo {
        wait_for("the embedder window", || {
            let list = self.client().call(WINDOW_LIST, Value::Null).unwrap();
            let windows: Vec<WindowInfo> = serde_json::from_value(list).unwrap();
            windows
                .into_iter()
                .find(|w| w.app_id.as_deref() == Some(APP_ID) && w.width > 0)
        })
    }

    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let path = self.dir.join("shot.png");
        self.client()
            .screenshot(&ScreenshotParams {
                path: Some(path.to_string_lossy().into_owned()),
            })
            .unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(std::fs::read(&path).unwrap()))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut pixels).unwrap();
        let offset = ((y * self.output.0 + x) * 4) as usize;
        pixels[offset..offset + 4].try_into().unwrap()
    }

    /// Waits for the physical pixel (x, y) to become `color`.
    fn expect_pixel(&self, x: u32, y: u32, color: [u8; 4]) {
        let mut last = [0; 4];
        let found = wait_until(|| {
            last = self.pixel(x, y);
            last == color
        });
        assert!(found, "pixel ({x}, {y}) = {last:?}, expected {color:?}");
    }

    /// Like `expect_pixel`, within a few levels (the faint edge of another window's shadow).
    fn expect_pixel_near(&self, x: u32, y: u32, color: [u8; 4]) {
        let mut last = [0; 4];
        let found = wait_until(|| {
            last = self.pixel(x, y);
            last.iter().zip(color).all(|(a, b)| a.abs_diff(b) <= 4)
        });
        assert!(
            found,
            "pixel ({x}, {y}) = {last:?}, expected about {color:?}"
        );
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut embedder) = self.embedder.take() {
            let _ = embedder.kill();
            let _ = embedder.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn size_args((w, h): (u32, u32)) -> Vec<String> {
    vec!["--size".to_owned(), format!("{w}x{h}")]
}

fn spawn_embedder(dir: &Path, wayland: &str, args: &[String]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_silica-embedder"))
        .args(args)
        .arg(repo().join("testapps/demo/build/flutter_assets"))
        .env("XDG_RUNTIME_DIR", dir)
        .env("WAYLAND_DISPLAY", wayland)
        // Headless Ossido offers no GPU buffers: Mesa draws in software (llvmpipe) on wl_shm.
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env(
            "RUST_LOG",
            std::env::var("E2E_LOG").unwrap_or_else(|_| "warn".into()),
        )
        .spawn()
        .unwrap()
}

fn wait_until(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn wait_for<T>(what: &str, mut find: impl FnMut() -> Option<T>) -> T {
    let mut found = None;
    assert!(
        wait_until(|| {
            found = find();
            found.is_some()
        }),
        "{what} did not arrive within {TIMEOUT:?}"
    );
    found.unwrap()
}

#[test]
fn demo_renders_reacts_to_clicks_and_closes() {
    let mut session = Session::start("demo", (640, 480), 1, (300, 200));
    let window = session.window();
    assert_eq!((window.width, window.height), (300, 200));
    let (x, y) = (window.x as u32, window.y as u32);

    // First frame: background and probe square at density 1.
    session.expect_pixel(x + 150, y + 190, IDLE);
    // Inside the probe but outside the corner cut by Ossido (superellipse, radius 33.5).
    session.expect_pixel(x + 15, y + 15, LO_DPI);

    // A click crosses Ossido, Wayland, the embedder and Flutter.
    session
        .client()
        .call(INPUT_CLICK, json!({"x": x + 150, "y": y + 100}))
        .unwrap();
    session.expect_pixel(x + 150, y + 190, TAPPED);

    // window.close: as on the reference the app keeps running without windows (#142). The
    // window leaves the list and the process stays, registered in the app registry.
    session.client().call(WINDOW_CLOSE, Value::Null).unwrap();
    assert!(
        wait_until(|| {
            let list = session.client().call(WINDOW_LIST, Value::Null).unwrap();
            let windows: Vec<WindowInfo> = serde_json::from_value(list).unwrap();
            windows.iter().all(|w| w.app_id.as_deref() != Some(APP_ID))
        }),
        "the closed window is still listed"
    );
    let mut embedder = session.embedder.take().unwrap();
    assert!(
        embedder.try_wait().unwrap().is_none(),
        "the app ended with its window"
    );
    let registry = session.dir.join("silica-apps");
    assert_eq!(
        silica_control::apps::instances(&registry, Some(APP_ID)).len(),
        1,
        "the app is not in the registry"
    );

    // Launched again: the running process shows its window and the new one exits at once.
    let mut second = spawn_embedder(&session.dir, &session.wayland, &size_args((300, 200)));
    let mut second_status = None;
    assert!(
        wait_until(|| {
            second_status = second.try_wait().unwrap();
            second_status.is_some()
        }),
        "the second launch did not hand over"
    );
    assert!(second_status.unwrap().success());
    let shown = session.window();
    assert_eq!((shown.width, shown.height), (300, 200));
    session.expect_pixel(x + 150, y + 190, TAPPED);

    // app.quit with the app id (Quit in the Dock menu): it ends cleanly.
    session
        .client()
        .call("app.quit", json!({ "app_id": APP_ID }))
        .unwrap();
    let mut status = None;
    assert!(
        wait_until(|| {
            status = embedder.try_wait().unwrap();
            status.is_some()
        }),
        "the embedder did not exit"
    );
    assert!(status.unwrap().success(), "exit with {status:?}");
}

#[test]
fn keyboard_reaches_text_field_and_hardware_keyboard() {
    let session = Session::start("keys", (640, 480), 1, (300, 200));
    let window = session.window();
    let (x, y) = (window.x as u32, window.y as u32);
    session.expect_pixel(x + 150, y + 30, IDLE);

    // Text: keys not consumed by the framework → flutter/textinput → EditableText.
    let mut client = session.client();
    client.call(INPUT_TYPE, json!({"text": "word"})).unwrap();
    session.expect_pixel(x + 150, y + 30, TYPED);

    // Backspace and retyping go through the embedder's editing model.
    client.call(INPUT_KEY, json!({"key": "BackSpace"})).unwrap();
    session.expect_pixel(x + 150, y + 30, IDLE);
    client.call(INPUT_TYPE, json!({"text": "d"})).unwrap();
    session.expect_pixel(x + 150, y + 30, TYPED);

    // Esc: key event handled by HardwareKeyboard, which clears the field.
    client.call(INPUT_KEY, json!({"key": "Escape"})).unwrap();
    session.expect_pixel(x + 150, y + 30, IDLE);
}

#[test]
fn demo_renders_at_scale_two() {
    let session = Session::start("hidpi", (640, 480), 2, (200, 150));
    let window = session.window();
    // Logical geometry unchanged; the screenshot is in physical pixels.
    assert_eq!((window.width, window.height), (200, 150));
    let (x, y) = (window.x as u32 * 2, window.y as u32 * 2);
    session.expect_pixel(x + 30, y + 30, HI_DPI);
    // The probe square is 16 logical pixels, so 32 physical.
    session.expect_pixel(x + 31, y + 31, HI_DPI);
    session.expect_pixel(x + 33, y + 33, IDLE);
}

#[test]
fn gl_renderer_draws_at_scale_two_and_reacts_to_clicks() {
    let mut args = size_args((200, 150));
    args.extend(["--renderer".to_owned(), "gl".to_owned()]);
    let session = Session::start_with("gl", (640, 480), 2, &args);
    // The first GL buffer after switching to scale 2 may have the old size
    // (GlSurface::present): wait for the requested one.
    let window = wait_for("the window at 200 × 150", || {
        Some(session.window()).filter(|w| (w.width, w.height) == (200, 150))
    });
    let (x, y) = (window.x as u32 * 2, window.y as u32 * 2);
    // Same pixels as the software renderer: buffer scale and EGL window size.
    session.expect_pixel(x + 31, y + 31, HI_DPI);
    session.expect_pixel(x + 33, y + 33, IDLE);
    session
        .client()
        .call(
            INPUT_CLICK,
            json!({ "x": window.x + 100, "y": window.y + 75 }),
        )
        .unwrap();
    session.expect_pixel(x + 200, y + 280, TAPPED);
}

#[test]
fn impeller_renderer_draws_and_reacts_to_clicks() {
    // With a GPU it is Impeller; on llvmpipe (CI) the embedder falls back to Skia, because there
    // Impeller crashes inside the engine (gate 1 measurements, docs/gate1-dock.md).
    let mut args = size_args((200, 150));
    args.extend(["--renderer".to_owned(), "impeller".to_owned()]);
    let session = Session::start_with("impeller", (640, 480), 2, &args);
    let window = wait_for("the window at 200 × 150", || {
        Some(session.window()).filter(|w| (w.width, w.height) == (200, 150))
    });
    let (x, y) = (window.x as u32 * 2, window.y as u32 * 2);
    session.expect_pixel(x + 31, y + 31, HI_DPI);
    session.expect_pixel(x + 33, y + 33, IDLE);
    session
        .client()
        .call(
            INPUT_CLICK,
            json!({ "x": window.x + 100, "y": window.y + 75 }),
        )
        .unwrap();
    session.expect_pixel(x + 200, y + 280, TAPPED);
}

#[test]
fn gl_layer_at_scale_two_gets_its_size_and_clicks() {
    // The M1.5 case: OpenGL layer at scale 2. The first buffer after a resize may come out
    // at the old size (see GlSurface::present): it depends on timing, it was seen on
    // WSL with d3d12 and on GitHub with llvmpipe (gl_renderer_draws_at_scale_two_...).
    let args: Vec<String> = [
        "--renderer",
        "gl",
        "--layer",
        "bottom",
        "--anchor",
        "left,right,bottom",
        "--size",
        "320x100",
        "--namespace",
        "gl-layer",
    ]
    .map(str::to_owned)
    .to_vec();
    let session = Session::start_with("gl-layer", (640, 480), 2, &args);
    let layer = wait_for("the layer at the requested size", || {
        Some(session.layer("gl-layer")).filter(|l| (l.width, l.height) == (320, 100))
    });
    let (x, y) = (layer.x as u32 * 2, layer.y as u32 * 2);
    session.expect_pixel(x + 100, y + 150, IDLE);
    session
        .client()
        .call(
            INPUT_CLICK,
            json!({ "x": layer.x + 160, "y": layer.y + 50 }),
        )
        .unwrap();
    session.expect_pixel(x + 100, y + 150, TAPPED);
}

#[test]
fn layer_bar_reserves_space_takes_keyboard_and_clicks() {
    const BAR: u32 = 48;
    let bar_args: Vec<String> = [
        "--layer",
        "top",
        "--anchor",
        "top,left,right",
        "--size",
        "640x48",
        "--exclusive-zone",
        "48",
        "--keyboard",
        "exclusive",
        "--namespace",
        "bar",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let mut session = Session::start_with("layer", (640, 480), 1, &bar_args);

    // Anchored at the top across the whole width, with the keyboard.
    let bar = session.layer("bar");
    assert_eq!(
        (bar.x, bar.y, bar.width, bar.height),
        (0, 0, 640, BAR as i32)
    );
    assert_eq!(
        (bar.layer.as_str(), bar.exclusive_zone),
        ("top", BAR as i32)
    );
    assert!(bar.focused);
    session.expect_pixel(600, 40, IDLE);

    // A normal window opens below the exclusive zone.
    session.spawn(&size_args((200, 100)));
    let window = session.window();
    assert!(
        window.y >= BAR as i32,
        "window at {} above the bar",
        window.y
    );
    let (wx, wy) = (window.x as u32, window.y as u32);
    session.expect_pixel(wx + 100, wy + 90, IDLE);

    // The keyboard goes to the bar, not to the window.
    let mut client = session.client();
    client.call(INPUT_TYPE, json!({"text": "word"})).unwrap();
    session.expect_pixel(600, 40, TYPED);
    session.expect_pixel(wx + 100, wy + 90, IDLE);

    // A click on a layer reaches the layer.
    client.call(INPUT_KEY, json!({"key": "Escape"})).unwrap();
    session.expect_pixel(600, 40, IDLE);
    client
        .call(INPUT_CLICK, json!({"x": 600, "y": 40}))
        .unwrap();
    session.expect_pixel(600, 40, TAPPED);
    session.expect_pixel(wx + 100, wy + 90, IDLE);
}

#[test]
fn glass_widget_declares_its_region_to_ossido() {
    let session = Session::start("glass", (640, 480), 1, (300, 200));
    let window = session.window();
    // The demo puts a 40 × 20 Glass at the top right (testapps/demo, glassSize).
    let list = wait_for("the demo's glass regions", || {
        let list = session
            .client()
            .call(silica_control::commands::GLASS_LIST, Value::Null)
            .ok()?;
        list.as_array().filter(|list| !list.is_empty()).cloned()
    });
    let region = &list[0]["regions"][0];
    assert_eq!(list[0]["surface"], format!("window {}", window.id));
    assert_eq!(
        (region["x"].as_i64(), region["y"].as_i64()),
        (Some(260), Some(0))
    );
    assert_eq!(
        (region["width"].as_i64(), region["height"].as_i64()),
        (Some(40), Some(20))
    );
    assert_eq!(region["radius"].as_f64(), Some(6.0));
    assert_eq!(region["material"], "clear");
}

#[test]
fn the_window_title_comes_from_the_command_line() {
    let mut args = size_args((300, 200));
    args.extend(["--title".to_owned(), "Title test".to_owned()]);
    let session = Session::start_with("title", (640, 480), 1, &args);
    let window = session.window();
    assert_eq!(window.title.as_deref(), Some("Title test"));
}

const SECOND: [u8; 4] = [0x88, 0x33, 0xAA, 0xFF];

/// The app's windows in the session, top first.
fn app_windows(session: &Session) -> Vec<WindowInfo> {
    let list = session.client().call(WINDOW_LIST, Value::Null).unwrap();
    let windows: Vec<WindowInfo> = serde_json::from_value(list).unwrap();
    windows
        .into_iter()
        .filter(|w| w.app_id.as_deref() == Some(APP_ID) && w.width > 0)
        .collect()
}

/// The demo's second window (`F2`, testapps/demo), once it is listed at its size.
fn second_window(session: &Session) -> WindowInfo {
    wait_for("the second window", || {
        app_windows(session)
            .into_iter()
            .find(|w| w.title.as_deref() == Some("Second") && (w.width, w.height) == (200, 120))
    })
}

/// The point of `main` (logical) farthest from `other` and from its shadow, away from the
/// edges and the rounded corners.
fn uncovered(main: &WindowInfo, other: &WindowInfo) -> (i32, i32) {
    const EDGE: i32 = 12;
    const CORNER: i32 = 34;
    let distance = |(x, y): (i32, i32)| {
        let dx = (other.x - x).max(x - (other.x + other.width)).max(0);
        let dy = (other.y - y).max(y - (other.y + other.height)).max(0);
        dx.max(dy)
    };
    let corner = |(x, y): (i32, i32)| {
        let near = |v: i32, start: i32, len: i32| v - start < CORNER || start + len - v < CORNER;
        near(x, main.x, main.width) && near(y, main.y, main.height)
    };
    let points = (main.x + EDGE..main.x + main.width - EDGE)
        .step_by(4)
        .flat_map(|x| {
            (main.y + EDGE..main.y + main.height - EDGE)
                .step_by(4)
                .map(move |y| (x, y))
        });
    let best = points
        .filter(|point| !corner(*point))
        .max_by_key(|point| distance(*point))
        .expect("a window without points");
    assert!(
        distance(best) >= 30,
        "the second window covers the main one"
    );
    best
}

/// Several windows in one process (#146): F2 opens a second view in its own xdg_toplevel,
/// both draw, a tap closes it from the app, its close button closes it through the app.
fn second_window_opens_draws_and_closes(name: &str, renderer: &[&str]) {
    let mut args = size_args((300, 200));
    args.push("--multi-window".to_owned());
    args.extend(renderer.iter().map(|s| (*s).to_owned()));
    let mut session = Session::start_with(name, (800, 600), 1, &args);
    let main = wait_for("the main window at 300 × 200", || {
        Some(session.window()).filter(|w| (w.width, w.height) == (300, 200))
    });
    session.expect_pixel(main.x as u32 + 150, main.y as u32 + 100, IDLE);

    // F2 in the main window: a second window of the same app, 200 × 120, all its color.
    let mut client = session.client();
    client.call(INPUT_KEY, json!({"key": "F2"})).unwrap();
    let second = second_window(&session);
    assert_ne!(second.id, main.id);
    session.expect_pixel(second.x as u32 + 100, second.y as u32 + 60, SECOND);
    // The main window still draws its own view.
    let (x, y) = uncovered(&main, &second);
    session.expect_pixel_near(x as u32, y as u32, IDLE);

    // The keyboard goes to the window that has it: the second window's field, not the main
    // window's (the engine is told which view has the focus).
    client.call(INPUT_TYPE, json!({"text": "word"})).unwrap();
    session.expect_pixel(second.x as u32 + 100, second.y as u32 + 30, TYPED);
    session.expect_pixel_near(x as u32, y as u32, IDLE);

    // A child window (set_parent): it opened over its parent, offset, and a click on the
    // parent does not bury it.
    assert_eq!((second.x, second.y), (main.x + 40, main.y + 40));
    client.call(INPUT_CLICK, json!({"x": x, "y": y})).unwrap();
    session.expect_pixel_near(x as u32, y as u32, TAPPED);
    let order: Vec<u64> = app_windows(&session).iter().map(|w| w.id).collect();
    assert_eq!(order, [second.id, main.id], "the child window went behind");

    // A tap in the second window: the app closes it (closeWindow); the process stays.
    client
        .call(
            INPUT_CLICK,
            json!({"x": second.x + 100, "y": second.y + 60}),
        )
        .unwrap();
    assert!(
        wait_until(|| app_windows(&session).len() == 1),
        "the second window is still listed"
    );
    let embedder = session.embedder.as_mut().unwrap();
    assert!(embedder.try_wait().unwrap().is_none(), "the app ended");
    // The keyboard is back in the parent window, with its field focused again.
    client.call(INPUT_TYPE, json!({"text": "word"})).unwrap();
    session.expect_pixel_near(x as u32, y as u32, TYPED);
    client.call(INPUT_KEY, json!({"key": "Escape"})).unwrap();
    session.expect_pixel_near(x as u32, y as u32, IDLE);

    // Again, closed by its close button: Ossido asks, the app answers closeRequested.
    client.call(INPUT_KEY, json!({"key": "F2"})).unwrap();
    let second = second_window(&session);
    session.expect_pixel(second.x as u32 + 100, second.y as u32 + 60, SECOND);
    client.call(WINDOW_CLOSE, json!({"id": second.id})).unwrap();
    assert!(
        wait_until(|| app_windows(&session).len() == 1),
        "the closed second window is still listed"
    );
    assert_eq!(session.window().id, main.id);
    let embedder = session.embedder.as_mut().unwrap();
    assert!(embedder.try_wait().unwrap().is_none(), "the app ended");
}

#[test]
fn a_second_window_opens_draws_and_closes_in_software() {
    second_window_opens_draws_and_closes("multi-sw", &[]);
}

#[test]
fn a_second_window_opens_draws_and_closes_with_gl() {
    second_window_opens_draws_and_closes("multi-gl", &["--renderer", "gl"]);
}
