//! L'app demo gira nell'embedder dentro Ossido headless: primo frame, clic, HiDPI, chiusura.
//!
//! Servono l'engine e il bundle: `cargo xtask engine` e `cargo xtask bundle demo`
//! (`cargo xtask ci` li prepara da sé).

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

/// Non è un test: è Ossido stesso, quando questo binario viene rilanciato da `Session`.
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
        spawn: Vec::new(),
    })
    .unwrap();
    std::process::exit(0);
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Un processo figlio che muore con il test, anche se il test va in panic.
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
    /// Ossido headless di `output` pixel fisici a scala `scale`, con l'app demo in una
    /// finestra di `window` pixel logici.
    fn start(name: &str, output: (u32, u32), scale: u32, window: (u32, u32)) -> Self {
        Self::start_with(name, output, scale, &size_args(window))
    }

    /// Come `start`, con argomenti dell'embedder a scelta (per esempio un layer).
    fn start_with(name: &str, output: (u32, u32), scale: u32, args: &[String]) -> Self {
        let engine = silica_embedder::default_engine_dir();
        let bundle = repo().join("apps/demo/build/flutter_assets");
        assert!(
            engine.join("libflutter_engine.so").exists() && bundle.join("kernel_blob.bin").exists(),
            "servono l'engine e il bundle: cargo xtask engine && cargo xtask bundle demo"
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
        // libtest scrive "test ossido_child_process ... " sulla stessa riga del primo
        // println! di Ossido: si cerca la variabile ovunque nella riga.
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

        let wayland = wayland.expect("Ossido senza socket Wayland");
        let embedder = spawn_embedder(&dir, &wayland, args);
        Self {
            dir,
            _ossido: ossido,
            embedder: Some(embedder),
            others: Vec::new(),
            control: control.expect("Ossido senza canale di controllo"),
            wayland,
            output,
        }
    }

    /// Un'altra istanza dell'app demo nella stessa sessione.
    fn spawn(&mut self, args: &[String]) {
        self.others
            .push(KillOnDrop(spawn_embedder(&self.dir, &self.wayland, args)));
    }

    fn layer(&self, namespace: &str) -> LayerInfo {
        wait_for("il layer", || {
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
        wait_for("la finestra dell'embedder", || {
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

    /// Aspetta che il pixel fisico (x, y) diventi `color`.
    fn expect_pixel(&self, x: u32, y: u32, color: [u8; 4]) {
        let mut last = [0; 4];
        let found = wait_until(|| {
            last = self.pixel(x, y);
            last == color
        });
        assert!(found, "pixel ({x}, {y}) = {last:?}, atteso {color:?}");
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
        .arg(repo().join("apps/demo/build/flutter_assets"))
        .env("XDG_RUNTIME_DIR", dir)
        .env("WAYLAND_DISPLAY", wayland)
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
        "{what} non è arrivata in {TIMEOUT:?}"
    );
    found.unwrap()
}

#[test]
fn demo_renders_reacts_to_clicks_and_closes() {
    let mut session = Session::start("demo", (640, 480), 1, (300, 200));
    let window = session.window();
    assert_eq!((window.width, window.height), (300, 200));
    let (x, y) = (window.x as u32, window.y as u32);

    // Primo frame: fondo e quadrato sonda a densità 1.
    session.expect_pixel(x + 150, y + 190, IDLE);
    session.expect_pixel(x + 2, y + 2, LO_DPI);

    // Un clic attraversa Ossido, Wayland, l'embedder e Flutter.
    session
        .client()
        .call(INPUT_CLICK, json!({"x": x + 150, "y": y + 100}))
        .unwrap();
    session.expect_pixel(x + 150, y + 190, TAPPED);

    // window.close: l'embedder spegne l'engine ed esce pulito.
    session.client().call(WINDOW_CLOSE, Value::Null).unwrap();
    let mut embedder = session.embedder.take().unwrap();
    let mut status = None;
    assert!(
        wait_until(|| {
            status = embedder.try_wait().unwrap();
            status.is_some()
        }),
        "l'embedder non è uscito"
    );
    assert!(status.unwrap().success(), "uscita con {status:?}");
}

#[test]
fn keyboard_reaches_text_field_and_hardware_keyboard() {
    let session = Session::start("keys", (640, 480), 1, (300, 200));
    let window = session.window();
    let (x, y) = (window.x as u32, window.y as u32);
    session.expect_pixel(x + 150, y + 30, IDLE);

    // Testo: tasti non consumati dal framework → flutter/textinput → EditableText.
    let mut client = session.client();
    client.call(INPUT_TYPE, json!({"text": "ciao"})).unwrap();
    session.expect_pixel(x + 150, y + 30, TYPED);

    // Backspace e riscrittura passano dal modello di editing dell'embedder.
    client.call(INPUT_KEY, json!({"key": "BackSpace"})).unwrap();
    session.expect_pixel(x + 150, y + 30, IDLE);
    client.call(INPUT_TYPE, json!({"text": "o"})).unwrap();
    session.expect_pixel(x + 150, y + 30, TYPED);

    // Esc: evento tasto gestito da HardwareKeyboard, che azzera il campo.
    client.call(INPUT_KEY, json!({"key": "Escape"})).unwrap();
    session.expect_pixel(x + 150, y + 30, IDLE);
}

#[test]
fn demo_renders_at_scale_two() {
    let session = Session::start("hidpi", (640, 480), 2, (200, 150));
    let window = session.window();
    // Geometria logica invariata; lo screenshot è in pixel fisici.
    assert_eq!((window.width, window.height), (200, 150));
    let (x, y) = (window.x as u32 * 2, window.y as u32 * 2);
    session.expect_pixel(x + 2, y + 2, HI_DPI);
    // Il quadrato sonda è 16 pixel logici, quindi 32 fisici.
    session.expect_pixel(x + 31, y + 31, HI_DPI);
    session.expect_pixel(x + 33, y + 33, IDLE);
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
        "barra",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let mut session = Session::start_with("layer", (640, 480), 1, &bar_args);

    // Ancorato in alto su tutta la larghezza, con la tastiera.
    let bar = session.layer("barra");
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

    // Una finestra normale si apre sotto la zona esclusiva.
    session.spawn(&size_args((200, 100)));
    let window = session.window();
    assert!(
        window.y >= BAR as i32,
        "finestra in {} sopra la barra",
        window.y
    );
    let (wx, wy) = (window.x as u32, window.y as u32);
    session.expect_pixel(wx + 100, wy + 90, IDLE);

    // La tastiera va alla barra, non alla finestra.
    let mut client = session.client();
    client.call(INPUT_TYPE, json!({"text": "ciao"})).unwrap();
    session.expect_pixel(600, 40, TYPED);
    session.expect_pixel(wx + 100, wy + 90, IDLE);

    // Il clic su un layer arriva al layer.
    client.call(INPUT_KEY, json!({"key": "Escape"})).unwrap();
    session.expect_pixel(600, 40, IDLE);
    client
        .call(INPUT_CLICK, json!({"x": 600, "y": 40}))
        .unwrap();
    session.expect_pixel(600, 40, TAPPED);
    session.expect_pixel(wx + 100, wy + 90, IDLE);
}
