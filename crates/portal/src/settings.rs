//! `org.freedesktop.impl.portal.Settings`: Silica's appearance for every app
//! (`org.freedesktop.appearance` › `color-scheme`: 0 no preference, 1 dark, 2 light).
//! The source is the daemon (`dev.frascella.Silica.System1`, key `appearance.dark`); without
//! the daemon, the file `$XDG_CONFIG_HOME/silica/appearance.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};

pub const APPEARANCE: &str = "org.freedesktop.appearance";
pub const COLOR_SCHEME: &str = "color-scheme";
pub const DAEMON_NAME: &str = "dev.frascella.Silica";
pub const DAEMON_PATH: &str = "/dev/frascella/Silica/System1";
pub const DAEMON_INTERFACE: &str = "dev.frascella.Silica.System1";

/// The `color-scheme` value for the appearance.
pub fn color_scheme(dark: bool) -> u32 {
    if dark { 1 } else { 2 }
}

/// `$XDG_CONFIG_HOME/silica/appearance.json` (usually `~/.config`).
pub fn appearance_file() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("silica").join("appearance.json"))
}

/// `{"dark": true}` in the file; without the file, light.
pub fn read_file(file: &Path) -> bool {
    std::fs::read_to_string(file).is_ok_and(|text| text.replace(' ', "").contains("\"dark\":true"))
}

/// A namespace asked by `ReadAll`: empty, or with a trailing `*` as a prefix.
fn matches(pattern: &str, namespace: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => namespace.starts_with(prefix),
        None => pattern.is_empty() || pattern == namespace,
    }
}

#[derive(Clone)]
pub struct Settings {
    pub dark: Arc<AtomicBool>,
}

#[interface(name = "org.freedesktop.impl.portal.Settings")]
impl Settings {
    async fn read_all(
        &self,
        namespaces: Vec<String>,
    ) -> HashMap<String, HashMap<String, OwnedValue>> {
        let mut out = HashMap::new();
        if namespaces.is_empty() || namespaces.iter().any(|p| matches(p, APPEARANCE)) {
            let scheme = OwnedValue::from(color_scheme(self.dark.load(Ordering::Relaxed)));
            out.insert(
                APPEARANCE.to_owned(),
                HashMap::from([(COLOR_SCHEME.to_owned(), scheme)]),
            );
        }
        out
    }

    async fn read(&self, namespace: String, key: String) -> zbus::fdo::Result<OwnedValue> {
        if namespace == APPEARANCE && key == COLOR_SCHEME {
            return Ok(OwnedValue::from(color_scheme(
                self.dark.load(Ordering::Relaxed),
            )));
        }
        Err(zbus::fdo::Error::Failed(format!(
            "unknown setting: {namespace} {key}"
        )))
    }

    #[zbus(signal)]
    pub async fn setting_changed(
        emitter: &SignalEmitter<'_>,
        namespace: &str,
        key: &str,
        value: Value<'_>,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn version(&self) -> u32 {
        2
    }
}

/// The daemon state (`State`): `appearance.dark`, if present.
fn dark_in(state: &HashMap<String, OwnedValue>) -> Option<bool> {
    state
        .get("appearance.dark")
        .and_then(|v| bool::try_from(v).ok())
}

/// Follows the daemon: reads the state and listens to `Changed`; on every appearance change it
/// updates `dark` and emits `SettingChanged`. Returns when the daemon is gone or the bus closes.
pub async fn follow_daemon(
    connection: &zbus::Connection,
    settings: &Settings,
    path: &str,
) -> zbus::Result<()> {
    use futures_lite::StreamExt;
    let proxy = zbus::Proxy::new(connection, DAEMON_NAME, DAEMON_PATH, DAEMON_INTERFACE).await?;
    let mut changes = proxy.receive_signal("Changed").await?;
    let state: HashMap<String, OwnedValue> = proxy.call("State", &()).await?;
    let emitter = SignalEmitter::new(connection, path.to_owned())?;
    let publish = async |dark: bool| -> zbus::Result<()> {
        if settings.dark.swap(dark, Ordering::Relaxed) != dark {
            Settings::setting_changed(
                &emitter,
                APPEARANCE,
                COLOR_SCHEME,
                Value::from(color_scheme(dark)),
            )
            .await?;
        }
        Ok(())
    };
    if let Some(dark) = dark_in(&state) {
        publish(dark).await?;
    }
    while let Some(message) = changes.next().await {
        let Ok(state) = message.body().deserialize::<HashMap<String, OwnedValue>>() else {
            continue;
        };
        if let Some(dark) = dark_in(&state) {
            publish(dark).await?;
        }
    }
    Ok(())
}
