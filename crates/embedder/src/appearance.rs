//! L'aspetto del sistema per Flutter: `flutter/settings` con `platformBrightness`, letto dal
//! portale Settings (`org.freedesktop.appearance` › `color-scheme`: 1 scuro) e seguito con
//! `SettingChanged`. Prima il backend di Silica (`silica-portal`), poi il frontend
//! (`xdg-desktop-portal`); senza nessuno dei due, chiaro.

use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::json;
use tracing::{info, warn};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;

pub const SETTINGS_CHANNEL: &str = "flutter/settings";

/// Prima il backend di Silica (dentro Silica è lui a decidere: il frontend senza la
/// configurazione di Silica ricadrebbe sul backend GTK e sulla preferenza di GNOME), poi il
/// frontend (Silica dentro un'altra sessione).
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

/// Il messaggio `flutter/settings` (JSONMessageCodec).
pub fn settings_message(dark: bool) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "textScaleFactor": 1.0,
        "alwaysUse24HourFormat": true,
        "platformBrightness": if dark { "dark" } else { "light" },
    }))
    .unwrap_or_default()
}

/// Un valore di `color-scheme`, anche dentro una variante (`Read` della v1 la incapsula).
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

/// Legge `color-scheme` dal primo portale che risponde: il proxy e lo scuro.
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

/// Segue l'aspetto su un thread: manda `true` (scuro) o `false` a ogni cambio.
pub fn watch(
    sender: smithay_client_toolkit::reexports::calloop::channel::Sender<bool>,
) -> Option<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("silica-aspetto".to_owned())
        .spawn(move || {
            let connection = match Connection::session() {
                Ok(connection) => connection,
                Err(err) => {
                    info!("aspetto: nessun bus di sessione ({err}), chiaro");
                    return;
                }
            };
            // Il portale può arrivare dopo l'app: qualche tentativo.
            let mut found = None;
            for _ in 0..10 {
                found = read(&connection);
                if found.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            let Some((proxy, dark)) = found else {
                info!("aspetto: nessun portale Settings, chiaro");
                return;
            };
            if sender.send(dark).is_err() {
                return;
            }
            let signals = match proxy.receive_signal("SettingChanged") {
                Ok(signals) => signals,
                Err(err) => {
                    warn!("aspetto: SettingChanged non disponibile: {err}");
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
        .map_err(|err| warn!("aspetto: thread non avviato: {err}"))
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
