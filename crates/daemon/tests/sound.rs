//! `dev.frascella.Silica.Sound1` on a private bus, with a fake `pw-play` that records what it
//! is asked to play.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use silica_daemon::sound::{SOUND_PATH, Sound, SoundConfig};
use zbus::{Connection, proxy};

struct Bus {
    child: Child,
    address: String,
}

impl Bus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon (package dbus) is required for these tests");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        Self {
            child,
            address: line.trim().to_owned(),
        }
    }

    async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[proxy(
    interface = "dev.frascella.Silica.Sound1",
    default_path = "/dev/frascella/Silica/Sound1"
)]
trait Sound1 {
    fn play_sound(&self, name: &str) -> zbus::Result<()>;
    fn ui_sounds_enabled(&self) -> zbus::Result<bool>;
    fn set_ui_sounds_enabled(&self, enabled: bool) -> zbus::Result<()>;
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("silica-sound-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Waits until the fake player has written `lines` lines.
fn played(log: &Path, lines: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        let got: Vec<String> = text.lines().map(str::to_owned).collect();
        if got.len() >= lines || Instant::now() > deadline {
            return got;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn plays_by_name_and_respects_the_setting() {
    let dir = temp_dir("play");
    let sounds = dir.join("sounds");
    let user = dir.join("user");
    std::fs::create_dir_all(&sounds).unwrap();
    std::fs::create_dir_all(&user).unwrap();
    std::fs::write(sounds.join("trash-move.wav"), b"RIFF").unwrap();
    // The user's own copy wins over the system's.
    std::fs::write(sounds.join("trash-empty.wav"), b"RIFF").unwrap();
    std::fs::write(user.join("trash-empty.oga"), b"OggS").unwrap();
    let log = dir.join("played.log");
    let player = dir.join("pw-play");
    std::fs::write(
        &player,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&player, std::fs::Permissions::from_mode(0o755)).unwrap();
    let settings = dir.join("config/silica/sound.json");

    let bus = Bus::start();
    zbus::block_on(async {
        let server = bus.connect().await;
        server
            .object_server()
            .at(
                SOUND_PATH,
                Sound::new(SoundConfig {
                    player: player.clone(),
                    dirs: vec![user.clone(), sounds.clone()],
                    settings_file: Some(settings.clone()),
                }),
            )
            .await
            .unwrap();
        let client = bus.connect().await;
        let proxy = Sound1Proxy::builder(&client)
            .destination(server.unique_name().unwrap().to_owned())
            .unwrap()
            .build()
            .await
            .unwrap();

        // On by default.
        assert!(proxy.ui_sounds_enabled().await.unwrap());
        proxy.play_sound("trash-move").await.unwrap();
        assert_eq!(
            played(&log, 1),
            [sounds.join("trash-move.wav").display().to_string()]
        );
        proxy.play_sound("trash-empty").await.unwrap();
        assert_eq!(
            played(&log, 2)[1],
            user.join("trash-empty.oga").display().to_string()
        );

        // Hostile or unknown names never reach the player.
        let invalid = proxy.play_sound("../../etc/passwd").await.unwrap_err();
        assert!(invalid.to_string().contains("InvalidArgs"), "{invalid}");
        let missing = proxy.play_sound("no-such-sound").await.unwrap_err();
        assert!(missing.to_string().contains("FileNotFound"), "{missing}");

        // Off: saved, and nothing plays.
        proxy.set_ui_sounds_enabled(false).await.unwrap();
        assert!(!proxy.ui_sounds_enabled().await.unwrap());
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("false")
        );
        proxy.play_sound("trash-move").await.unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2);

        // Edited by hand back on: the next sound plays.
        std::fs::write(&settings, "{\"ui_sounds\": true}").unwrap();
        proxy.play_sound("trash-move").await.unwrap();
        assert_eq!(played(&log, 3).len(), 3);
    });
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_missing_player_is_not_an_error() {
    let dir = temp_dir("noplayer");
    std::fs::write(dir.join("file-transfer.wav"), b"RIFF").unwrap();
    let bus = Bus::start();
    zbus::block_on(async {
        let server = bus.connect().await;
        server
            .object_server()
            .at(
                SOUND_PATH,
                Sound::new(SoundConfig {
                    player: dir.join("no-player"),
                    dirs: vec![dir.clone()],
                    settings_file: None,
                }),
            )
            .await
            .unwrap();
        let client = bus.connect().await;
        let proxy = Sound1Proxy::builder(&client)
            .destination(server.unique_name().unwrap().to_owned())
            .unwrap()
            .build()
            .await
            .unwrap();
        // Several in a row, beyond the limit of players at once: never an error.
        for _ in 0..8 {
            proxy.play_sound("file-transfer").await.unwrap();
        }
        // Without a file the setting lives in memory.
        proxy.set_ui_sounds_enabled(false).await.unwrap();
        assert!(!proxy.ui_sounds_enabled().await.unwrap());
    });
    std::fs::remove_dir_all(&dir).unwrap();
}
