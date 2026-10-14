//! The system appearance for Flutter: `flutter/settings` with `platformBrightness`, read from
//! the Settings portal (`org.freedesktop.appearance` › `color-scheme`: 1 dark) and followed
//! with `SettingChanged`. First Silica's backend (`silica-portal`), then the frontend
//! (`xdg-desktop-portal`); without either, light.

use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::json;
use tracing::{info, warn};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;

pub const SETTINGS_CHANNEL: &str = "flutter/settings";

/// First Silica's backend (inside Silica it is the one that decides: the frontend without
/// Silica's configuration would fall back to the GTK backend and GNOME's preference), then
/// the frontend (Silica inside another session).
const SOURCES: [(&str, &str); 2] = [
    (
        "org.freedesktop.impl.portal.desktop.silica",
        "org.freedesktop.impl.portal.Settings",
    ),
    (
        "org.freedesktop.portal.Desktop",
        "org.freedesktop.portal.Settings",
    ),
];
const PATH: &str = "/org/freedesktop/portal/desktop";
const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";

/// The `flutter/settings` message (JSONMessageCodec).
pub fn settings_message(dark: bool) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "textScaleFactor": 1.0,
        "alwaysUse24HourFormat": true,
        "platformBrightness": if dark { "dark" } else { "light" },
    }))
    .unwrap_or_default()
}

/// A `color-scheme` value, even inside a variant (v1's `Read` wraps it).
fn scheme_of(value: &OwnedValue) -> Option<u32> {
    if let Ok(scheme) = u32::try_from(value) {
        return Some(scheme);
    }
    let inner = zbus::zvariant::Value::try_from(value).ok()?;
    match inner {
        zbus::zvariant::Value::Value(boxed) => u32::try_from(&*boxed).ok(),
        _ => None,
    }
}

/// Reads `color-scheme` from the first portal that answers: the proxy and the dark flag.
fn read(connection: &Connection) -> Option<(Proxy<'static>, bool)> {
    for (name, interface) in SOURCES {
        let Ok(proxy) = Proxy::new(connection, name, PATH, interface) else {
            continue;
        };
        let value: Result<OwnedValue, _> = proxy
            .call("ReadOne", &(NAMESPACE, KEY))
            .or_else(|_| proxy.call("Read", &(NAMESPACE, KEY)));
        if let Ok(value) = value
            && let Some(scheme) = scheme_of(&value)
        {
            return Some((proxy, scheme == 1));
        }
    }
    None
}

/// Follows the appearance on a thread: sends `true` (dark) or `false` on every change.
pub fn watch(
    sender: smithay_client_toolkit::reexports::calloop::channel::Sender<bool>,
) -> Option<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("silica-appearance".to_owned())
        .spawn(move || {
            let connection = match Connection::session() {
                Ok(connection) => connection,
                Err(err) => {
                    info!("appearance: no session bus ({err}), light");
                    return;
                }
            };
            // The portal may arrive after the app: a few attempts.
            let mut found = None;
            for _ in 0..10 {
                found = read(&connection);
                if found.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            let Some((proxy, dark)) = found else {
                info!("appearance: no Settings portal, light");
                return;
            };
            if sender.send(dark).is_err() {
                return;
            }
            let signals = match proxy.receive_signal("SettingChanged") {
                Ok(signals) => signals,
                Err(err) => {
                    warn!("appearance: SettingChanged unavailable: {err}");
                    return;
                }
            };
            for message in signals {
                let Ok((namespace, key, value)) =
                    message.body().deserialize::<(String, String, OwnedValue)>()
                else {
                    continue;
                };
                if namespace == NAMESPACE
                    && key == KEY
                    && let Some(scheme) = scheme_of(&value)
                    && sender.send(scheme == 1).is_err()
                {
                    return;
                }
            }
        })
        .map_err(|err| warn!("appearance: thread not started: {err}"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_message_carries_the_brightness() {
        let dark: serde_json::Value = serde_json::from_slice(&settings_message(true)).unwrap();
        assert_eq!(dark["platformBrightness"], "dark");
        let light: serde_json::Value = serde_json::from_slice(&settings_message(false)).unwrap();
        assert_eq!(light["platformBrightness"], "light");
        assert_eq!(light["alwaysUse24HourFormat"], true);
    }

    #[test]
    fn color_scheme_also_inside_a_variant() {
        assert_eq!(scheme_of(&OwnedValue::from(1u32)), Some(1));
        let boxed = OwnedValue::try_from(zbus::zvariant::Value::Value(Box::new(
            zbus::zvariant::Value::U32(2),
        )))
        .unwrap();
        assert_eq!(scheme_of(&boxed), Some(2));
    }
}
