//! Supporto ai test: un Ossido headless pilotato dal canale di controllo.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub mod client;

use ossido::state::CLEAR_COLOR;
use silica_control::{Client, ScreenshotParams};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 200;

/// Un Ossido headless con la sua directory runtime; muore con il test.
pub struct Headless {
    child: Child,
    pub dir: PathBuf,
    pub control: String,
    /// Percorso assoluto del socket Wayland di Ossido.
    pub wayland: PathBuf,
}

impl Headless {
    pub fn start(name: &str) -> Self {
        Self::start_with(name, (WIDTH, HEIGHT), &[])
    }

    /// Come `start`, con un'altra dimensione dell'output e argomenti in più per Ossido.
    pub fn start_with(name: &str, (width, height): (u32, u32), extra: &[&str]) -> Self {
        let dir = std::env::temp_dir().join(format!("ossido-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        let mut child = Command::new(env!("CARGO_BIN_EXE_ossido"))
            .args([
                "--backend",
                "headless",
                "--size",
                &format!("{width}x{height}"),
            ])
            // Screenshot deterministici: niente animazioni, salvo chi le riaccende.
            .args(["--animation-scale", "0"])
            // Vetro pieno anche su llvmpipe (l'automatico lì sarebbe smerigliato).
            .args(["--glass", "full"])
            .args(extra)
            .args(["--control", "0"])
            .env("XDG_RUNTIME_DIR", &dir)
            .env("RUST_LOG", "warn")
            .env_remove("WAYLAND_DISPLAY")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut wayland = None;
        let mut control = None;
        for line in BufReader::new(child.stdout.take().unwrap()).lines() {
            let line = line.unwrap();
            if let Some(name) = line.strip_prefix("WAYLAND_DISPLAY=") {
                wayland = Some(dir.join(name));
            }
            if let Some(addr) = line.strip_prefix("OSSIDO_CONTROL=") {
                control = Some(addr.to_owned());
                break;
            }
        }
        Self {
            child,
            wayland: wayland.expect("Ossido non ha annunciato il socket Wayland"),
            control: control.expect("Ossido è uscito senza aprire il canale di controllo"),
            dir,
        }
    }

    pub fn try_wait(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().unwrap()
    }

    pub fn client(&self) -> Client {
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

pub fn decode(png_bytes: &[u8]) -> (png::OutputInfo, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    (info, pixels)
}

pub fn assert_clear_color(pixels: &[u8], x: u32, y: u32) {
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

/// Il pixel è fuori dalla finestra di prova (rossa): fondo, ombra o anello d'ombra.
pub fn assert_outside_window(pixels: &[u8], x: u32, y: u32) {
    let [r, g, b, _] = pixel(pixels, x, y);
    assert!(
        r < 64 && r.abs_diff(g) < 32 && r.abs_diff(b) < 32,
        "pixel ({x}, {y}) = {:?}: atteso fuori dalla finestra",
        [r, g, b]
    );
}

/// RGBA del pixel (x, y) in uno screenshot `WIDTH`×`HEIGHT`.
pub fn pixel(shot: &[u8], x: u32, y: u32) -> [u8; 4] {
    pixel_in(shot, WIDTH, x, y)
}

/// RGBA del pixel (x, y) in uno screenshot largo `width`.
pub fn pixel_in(shot: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    shot[offset..offset + 4].try_into().unwrap()
}

/// Uno screenshot su file, decodificato in RGBA.
pub fn screenshot(ossido: &Headless) -> Vec<u8> {
    let path = ossido.dir.join("shot.png");
    ossido
        .client()
        .screenshot(&ScreenshotParams {
            path: Some(path.to_string_lossy().into_owned()),
        })
        .unwrap();
    decode(&std::fs::read(&path).unwrap()).1
}
