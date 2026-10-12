//! Avvia Ossido in headless e lo pilota dal canale di controllo.

// Codice di test: un errore deve far fallire il test (AGENTS.md vale per il codice non di test).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use base64::Engine;
use ossido::state::CLEAR_COLOR;
use silica_control::{Client, Error, ScreenshotParams};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;

/// Un Ossido headless con la sua directory runtime; muore con il test.
struct Headless {
    child: Child,
    dir: PathBuf,
    control: String,
}

impl Headless {
    fn start(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ossido-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        let mut child = Command::new(env!("CARGO_BIN_EXE_ossido"))
            .args([
                "--backend",
                "headless",
                "--size",
                &format!("{WIDTH}x{HEIGHT}"),
            ])
            .args(["--control", "0"])
            .env("XDG_RUNTIME_DIR", &dir)
            .env("RUST_LOG", "warn")
            .env_remove("WAYLAND_DISPLAY")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let control = stdout
            .lines()
            .map(Result::unwrap)
            .find_map(|line| line.strip_prefix("OSSIDO_CONTROL=").map(str::to_owned))
            .expect("Ossido è uscito senza aprire il canale di controllo");
        Self {
            child,
            dir,
            control,
        }
    }

    fn client(&self) -> Client {
        Client::connect(&self.control).unwrap()
    }
}

impl Drop for Headless {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn decode(png_bytes: &[u8]) -> (png::OutputInfo, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    (info, pixels)
}

fn assert_clear_color(pixels: &[u8], x: u32, y: u32) {
    let offset = ((y * WIDTH + x) * 4) as usize;
    let pixel = &pixels[offset..offset + 4];
    for (channel, expected) in pixel.iter().zip(CLEAR_COLOR) {
        let expected = (expected * 255.0).round() as i32;
        assert!(
            (i32::from(*channel) - expected).abs() <= 1,
            "pixel ({x}, {y}) = {pixel:?}, atteso il colore di fondo {CLEAR_COLOR:?}"
        );
    }
}

#[test]
fn screenshot_to_file_has_output_size_and_background() {
    let ossido = Headless::start("file");
    let path = ossido.dir.join("shot.png");
    let shot = ossido
        .client()
        .screenshot(&ScreenshotParams {
            path: Some(path.to_string_lossy().into_owned()),
        })
        .unwrap();
    assert_eq!((shot.width, shot.height), (WIDTH, HEIGHT));

    let (info, pixels) = decode(&std::fs::read(&path).unwrap());
    assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    for (x, y) in [(0, 0), (WIDTH / 2, HEIGHT / 2), (WIDTH - 1, HEIGHT - 1)] {
        assert_clear_color(&pixels, x, y);
    }
}

#[test]
fn screenshot_inline_returns_base64_png() {
    let ossido = Headless::start("inline");
    let mut client = ossido.client();
    let first = client.screenshot(&ScreenshotParams::default()).unwrap();
    // La connessione resta aperta per più comandi.
    let second = client.screenshot(&ScreenshotParams::default()).unwrap();

    for shot in [first, second] {
        assert!(shot.path.is_none());
        let png_bytes = base64::engine::general_purpose::STANDARD
            .decode(shot.png_base64.unwrap())
            .unwrap();
        let (info, _) = decode(&png_bytes);
        assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    }
}

#[test]
fn unknown_command_is_an_error_not_a_crash() {
    let ossido = Headless::start("unknown");
    let mut client = ossido.client();
    match client.call("ui.nope", serde_json::Value::Null) {
        Err(Error::Command(message)) => assert!(message.contains("ui.nope")),
        other => panic!("atteso un errore di comando, ottenuto {other:?}"),
    }
    // Ossido è ancora vivo e risponde.
    client.screenshot(&ScreenshotParams::default()).unwrap();
}
