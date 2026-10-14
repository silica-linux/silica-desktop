//! Test support: a headless Ossido driven through the control channel.

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

/// A headless Ossido with its runtime directory; it dies with the test.
pub struct Headless {
    child: Child,
    pub dir: PathBuf,
    pub control: String,
    /// Absolute path of Ossido's Wayland socket.
    pub wayland: PathBuf,
}

impl Headless {
    pub fn start(name: &str) -> Self {
        Self::start_with(name, (WIDTH, HEIGHT), &[])
    }

    /// Like `start`, with another output size and more arguments for Ossido.
    pub fn start_with(name: &str, size: (u32, u32), extra: &[&str]) -> Self {
        Self::start_with_env(name, size, extra, &[])
    }

    /// Like `start_with`, with environment variables for Ossido.
    pub fn start_with_env(
        name: &str,
        (width, height): (u32, u32),
        extra: &[&str],
        env: &[(&str, &str)],
    ) -> Self {
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
            // Deterministic screenshots: no animations, unless a test turns them back on.
            .args(["--animation-scale", "0"])
            // Full glass even on llvmpipe (the automatic level there would be frosted).
            .args(["--glass", "full"])
            .args(extra)
            .args(["--control", "0"])
            .env("XDG_RUNTIME_DIR", &dir)
            // The user's settings (keyboard.json, displays.json…) never reach the tests.
            .env("XDG_CONFIG_HOME", dir.join("config"))
            // Nor the user's session bus: Ossido's reports to silica-daemon go nowhere.
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", dir.join("no-bus").display()),
            )
            .env("RUST_LOG", "warn")
            .envs(env.iter().copied())
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
            wayland: wayland.expect("Ossido did not announce the Wayland socket"),
            control: control.expect("Ossido exited without opening the control channel"),
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
            "pixel ({x}, {y}) = {pixel:?}, expected the background color {CLEAR_COLOR:?}"
        );
    }
}

/// The pixel is outside the test window (red): background, shadow or shadow ring.
pub fn assert_outside_window(pixels: &[u8], x: u32, y: u32) {
    let [r, g, b, _] = pixel(pixels, x, y);
    assert!(
        r < 64 && r.abs_diff(g) < 32 && r.abs_diff(b) < 32,
        "pixel ({x}, {y}) = {:?}: expected outside the window",
        [r, g, b]
    );
}

/// RGBA of the pixel (x, y) in a `WIDTH`×`HEIGHT` screenshot.
pub fn pixel(shot: &[u8], x: u32, y: u32) -> [u8; 4] {
    pixel_in(shot, WIDTH, x, y)
}

/// RGBA of the pixel (x, y) in a screenshot `width` wide.
pub fn pixel_in(shot: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    shot[offset..offset + 4].try_into().unwrap()
}

/// A screenshot on file, decoded to RGBA.
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
