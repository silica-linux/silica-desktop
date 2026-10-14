//! Interface sound effects: `dev.frascella.Silica.Sound1` on `/dev/frascella/Silica/Sound1`.
//!
//! Apps and the shell ask for a sound by name (`PlaySound("trash-move")`); the daemon finds
//! `<name>.wav` (or `.oga`, `.ogg`) in the sound directories and plays it with PipeWire's
//! `pw-play`, without waiting for it. The "Play user interface sound effects" setting
//! (`ui_sounds` in `$XDG_CONFIG_HOME/silica/sound.json`, on by default) silences them all.
//!
//! Sound directories, in order: `$SILICA_SOUNDS_DIR` (the repository's `data/sounds` under
//! `cargo xtask run`), `$XDG_DATA_HOME/silica/sounds`, then `silica/sounds` in each of
//! `$XDG_DATA_DIRS` (`/usr/local/share:/usr/share` by default).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use serde_json::{Value, json};
use zbus::object_server::SignalEmitter;
use zbus::{fdo, interface};

pub const SOUND_PATH: &str = "/dev/frascella/Silica/Sound1";

/// At most this many sounds at once: a client asking in a loop cannot pile up players.
pub const MAX_PLAYING: usize = 4;

/// The extensions looked for, in order (what `pw-play` reads through libsndfile).
const EXTENSIONS: &[&str] = &["wav", "oga", "ogg"];

#[derive(Clone, Debug)]
pub struct SoundConfig {
    /// PipeWire's player.
    pub player: PathBuf,
    /// Where the sounds are looked for, first match wins.
    pub dirs: Vec<PathBuf>,
    /// Where the setting is kept; `None`: only in memory.
    pub settings_file: Option<PathBuf>,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            player: PathBuf::from("pw-play"),
            dirs: default_sound_dirs(),
            settings_file: default_settings_file(),
        }
    }
}

/// The sound directories from the environment (see the module documentation).
pub fn default_sound_dirs() -> Vec<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    sound_dirs(
        var("SILICA_SOUNDS_DIR").map(PathBuf::from),
        var("XDG_DATA_HOME").map(PathBuf::from),
        var("HOME").map(PathBuf::from),
        var("XDG_DATA_DIRS").and_then(|dirs| dirs.into_string().ok()),
    )
}

fn sound_dirs(
    own: Option<PathBuf>,
    data_home: Option<PathBuf>,
    home: Option<PathBuf>,
    data_dirs: Option<String>,
) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = own.into_iter().collect();
    if let Some(data) = data_home.or_else(|| home.map(|home| home.join(".local/share"))) {
        dirs.push(data.join("silica/sounds"));
    }
    let system = data_dirs.unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    dirs.extend(
        system
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(|dir| Path::new(dir).join("silica/sounds")),
    );
    dirs
}

/// `$XDG_CONFIG_HOME/silica/sound.json` (usually `~/.config`).
pub fn default_settings_file() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("silica").join("sound.json"))
}

/// The saved setting: `{"ui_sounds": false}` turns the sounds off; anything else (no file,
/// unreadable, another value) leaves them on.
pub fn read_ui_sounds(file: &Path) -> bool {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|json| json.get("ui_sounds").and_then(Value::as_bool))
        .unwrap_or(true)
}

/// A sound name: 1–64 characters among `a-z`, `0-9` and `-`, not starting with `-`. Nothing
/// that could leave the sound directories or reach the player as an option.
pub fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The file of sound `name`, from the first directory that has it.
pub fn find_sound(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    if !valid_name(name) {
        return None;
    }
    dirs.iter()
        .flat_map(|dir| {
            EXTENSIONS
                .iter()
                .map(move |ext| dir.join(format!("{name}.{ext}")))
        })
        .find(|path| path.is_file())
}

/// The sound service.
pub struct Sound {
    config: SoundConfig,
    /// The setting when there is no file to keep it in.
    enabled: AtomicBool,
    playing: Arc<AtomicUsize>,
}

impl Sound {
    pub fn new(config: SoundConfig) -> Self {
        let enabled = config.settings_file.as_deref().is_none_or(read_ui_sounds);
        Self {
            config,
            enabled: AtomicBool::new(enabled),
            playing: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// The setting, read again from its file each time: Settings or a person editing the
    /// file change it without going through the daemon.
    fn ui_sounds(&self) -> bool {
        match &self.config.settings_file {
            Some(file) if file.exists() => read_ui_sounds(file),
            _ => self.enabled.load(Ordering::Relaxed),
        }
    }

    /// Starts the player on `file` and lets it finish on its own; false if it could not start
    /// or too many are already playing.
    fn play(&self, file: &Path) -> bool {
        let playing = Arc::clone(&self.playing);
        if playing
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_PLAYING).then_some(n + 1)
            })
            .is_err()
        {
            return false;
        }
        let child = Command::new(&self.config.player)
            .arg(file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match child {
            Ok(mut child) => {
                // Reaped by its own thread: the D-Bus call does not wait for the sound.
                let spawned = std::thread::Builder::new().name("sound".to_owned()).spawn({
                    let playing = Arc::clone(&playing);
                    move || {
                        let _ = child.wait();
                        playing.fetch_sub(1, Ordering::AcqRel);
                    }
                });
                if spawned.is_err() {
                    playing.fetch_sub(1, Ordering::AcqRel);
                }
                true
            }
            Err(err) => {
                tracing::warn!("{}: {err}", self.config.player.display());
                playing.fetch_sub(1, Ordering::AcqRel);
                false
            }
        }
    }
}

#[interface(name = "dev.frascella.Silica.Sound1")]
impl Sound {
    /// Plays sound `name` unless the interface sounds are off. A malformed name is
    /// `InvalidArgs`, a missing sound `FileNotFound`; a sound dropped because too many are
    /// playing, or with no player, is not an error.
    fn play_sound(&self, name: &str) -> fdo::Result<()> {
        if !valid_name(name) {
            return Err(fdo::Error::InvalidArgs(format!(
                "invalid sound name: {name:?}"
            )));
        }
        if !self.ui_sounds() {
            return Ok(());
        }
        let Some(file) = find_sound(&self.config.dirs, name) else {
            return Err(fdo::Error::FileNotFound(format!("no sound {name}")));
        };
        self.play(&file);
        Ok(())
    }

    /// "Play user interface sound effects".
    fn ui_sounds_enabled(&self) -> bool {
        self.ui_sounds()
    }

    /// Turns the interface sounds on or off; the choice is saved (`sound.json`).
    async fn set_ui_sounds_enabled(
        &self,
        enabled: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.enabled.store(enabled, Ordering::Relaxed);
        if let Some(file) = &self.config.settings_file {
            let text = json!({ "ui_sounds": enabled }).to_string();
            let written = file
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(file, text));
            if let Err(err) = written {
                return Err(fdo::Error::Failed(format!("{}: {err}", file.display())));
            }
        }
        Self::ui_sounds_changed(&emitter, enabled).await?;
        Ok(())
    }

    /// The setting changed.
    #[zbus(signal)]
    async fn ui_sounds_changed(emitter: &SignalEmitter<'_>, enabled: bool) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_inside_the_sound_directories() {
        assert!(valid_name("trash-move"));
        assert!(valid_name("a1"));
        for bad in [
            "",
            "-v",
            "../x",
            "a/b",
            "Trash",
            "a.wav",
            "a b",
            &"x".repeat(65),
        ] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn directories_in_order() {
        let dirs = sound_dirs(
            Some(PathBuf::from("/repo/data/sounds")),
            None,
            Some(PathBuf::from("/home/u")),
            Some("/a::/b".to_owned()),
        );
        assert_eq!(
            dirs,
            [
                "/repo/data/sounds",
                "/home/u/.local/share/silica/sounds",
                "/a/silica/sounds",
                "/b/silica/sounds",
            ]
            .map(PathBuf::from)
        );
        let system = sound_dirs(None, Some(PathBuf::from("/d")), None, None);
        assert_eq!(system[0], PathBuf::from("/d/silica/sounds"));
        assert_eq!(system[2], PathBuf::from("/usr/share/silica/sounds"));
    }

    #[test]
    fn the_setting_defaults_to_on() {
        let dir = std::env::temp_dir().join(format!("silica-sound-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sound.json");
        assert!(read_ui_sounds(&file));
        std::fs::write(&file, "{\"ui_sounds\": false}").unwrap();
        assert!(!read_ui_sounds(&file));
        std::fs::write(&file, "not json").unwrap();
        assert!(read_ui_sounds(&file));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
