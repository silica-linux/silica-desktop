//! The sources of the system state.
//!
//! - NetworkManager, BlueZ and UPower on the system bus: properties are read, then
//!   reread on every signal of the service. If a service is missing or disappears the source
//!   becomes absent and it retries every `RETRY`.
//! - Backlight: sysfs (`brightness` / `max_brightness`), written with logind's
//!   `SetBrightness` and, if logind is not there, directly into the file.
//! - Audio: PipeWire's `wpctl` on the default output.
//!
//! Brightness and audio have no signals: they are reread every `poll`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_io::Timer;
use futures_lite::{StreamExt, future};
use zbus::fdo::{ObjectManagerProxy, PropertiesProxy};
use zbus::message::Type as MessageType;
use zbus::names::InterfaceName;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream};

use super::{Audio, Battery, Shared, Wifi};

const RETRY: Duration = Duration::from_secs(10);
/// Beyond this time `wpctl` is considered stuck.
const WPCTL_TIMEOUT: Duration = Duration::from_secs(2);

const NM: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const NM_ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const NM_WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const NM_AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
const BLUEZ: &str = "org.bluez";
const BLUEZ_ADAPTER: &str = "org.bluez.Adapter1";
const UPOWER: &str = "org.freedesktop.UPower";
const UPOWER_DISPLAY: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const UPOWER_DEVICE: &str = "org.freedesktop.UPower.Device";
const LOGIND: &str = "org.freedesktop.login1";
/// Default sink of `wpctl`.
const DEFAULT_SINK: &str = "@DEFAULT_AUDIO_SINK@";

type Result<T> = std::result::Result<T, String>;

/// All the sources together; never returns.
pub(super) async fn run(shared: &Shared) {
    future::zip(
        future::zip(network(shared), bluetooth(shared)),
        future::zip(power(shared), poll(shared)),
    )
    .await;
}

pub(super) async fn prop<T>(
    bus: &Connection,
    dest: &str,
    path: &str,
    iface: &str,
    name: &str,
) -> Result<T>
where
    T: TryFrom<OwnedValue>,
    T::Error: std::fmt::Display,
{
    let proxy = PropertiesProxy::builder(bus)
        .destination(dest.to_owned())
        .and_then(|b| b.path(path.to_owned()))
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let interface = InterfaceName::try_from(iface).map_err(|e| e.to_string())?;
    let value = proxy
        .get(interface, name)
        .await
        .map_err(|e| e.to_string())?;
    T::try_from(value).map_err(|e| format!("{iface}.{name}: {e}"))
}

async fn set_prop(
    bus: &Connection,
    dest: &str,
    path: &str,
    iface: &str,
    name: &str,
    value: Value<'_>,
) -> Result<()> {
    let proxy = PropertiesProxy::builder(bus)
        .destination(dest.to_owned())
        .and_then(|b| b.path(path.to_owned()))
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let interface = InterfaceName::try_from(iface).map_err(|e| e.to_string())?;
    proxy
        .set(interface, name, value)
        .await
        .map_err(|e| e.to_string())
}

/// The signals of a service (all of them, or of a single path).
async fn signals(bus: &Connection, sender: &str, path: Option<&str>) -> Result<MessageStream> {
    let mut rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(sender.to_owned())
        .map_err(|e| e.to_string())?;
    if let Some(path) = path {
        rule = rule.path(path.to_owned()).map_err(|e| e.to_string())?;
    }
    MessageStream::for_match_rule(rule.build(), bus, Some(64))
        .await
        .map_err(|e| e.to_string())
}

/// Follows a system bus source: `read` at the start and on every signal of `sender`;
/// if it fails the source is absent (`absent`) and it retries.
async fn follow<F, Fut>(
    shared: &Shared,
    sender: &str,
    path: Option<&str>,
    read: F,
    absent: impl Fn(&Shared),
) where
    F: Fn(Connection) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let Some(bus) = shared.config.system_bus.clone() else {
        absent(shared);
        return future::pending().await;
    };
    loop {
        let result = async {
            let mut stream = signals(&bus, sender, path).await?;
            read(bus.clone()).await?;
            while let Some(message) = stream.next().await {
                if message.is_ok() {
                    read(bus.clone()).await?;
                }
            }
            Ok::<(), String>(())
        }
        .await;
        if let Err(err) = result {
            tracing::debug!("{sender}: {err}");
        }
        absent(shared);
        Timer::after(RETRY).await;
    }
}

// --- Network -----------------------------------------------------------------------

async fn network(shared: &Shared) {
    follow(
        shared,
        NM,
        None,
        |bus| async move {
            let wifi = read_network(&bus).await?;
            shared.update(|s| s.wifi = Some(wifi));
            Ok(())
        },
        |shared| shared.update(|s| s.wifi = None),
    )
    .await;
}

async fn read_network(bus: &Connection) -> Result<Wifi> {
    let enabled: bool = prop(bus, NM, NM_PATH, NM, "WirelessEnabled").await?;
    let primary: OwnedObjectPath = prop(bus, NM, NM_PATH, NM, "PrimaryConnection").await?;
    let mut wifi = Wifi {
        enabled,
        ..Wifi::default()
    };
    if primary.as_str() == "/" {
        return Ok(wifi);
    }
    let kind: String = prop(bus, NM, primary.as_str(), NM_ACTIVE, "Type").await?;
    match kind.as_str() {
        "802-11-wireless" => {
            wifi.ssid = prop(bus, NM, primary.as_str(), NM_ACTIVE, "Id").await?;
            let devices: Vec<OwnedObjectPath> =
                prop(bus, NM, primary.as_str(), NM_ACTIVE, "Devices").await?;
            for device in devices {
                let Ok(ap) = prop::<OwnedObjectPath>(
                    bus,
                    NM,
                    device.as_str(),
                    NM_WIRELESS,
                    "ActiveAccessPoint",
                )
                .await
                else {
                    continue;
                };
                if ap.as_str() != "/"
                    && let Ok(strength) = prop::<u8>(bus, NM, ap.as_str(), NM_AP, "Strength").await
                {
                    wifi.strength = strength.min(100);
                    break;
                }
            }
        }
        "802-3-ethernet" => wifi.wired = true,
        _ => {}
    }
    Ok(wifi)
}

pub(super) async fn set_wifi_enabled(shared: &Shared, enabled: bool) -> Result<()> {
    let bus = shared
        .config
        .system_bus
        .as_ref()
        .ok_or("system bus absent")?;
    set_prop(
        bus,
        NM,
        NM_PATH,
        NM,
        "WirelessEnabled",
        Value::Bool(enabled),
    )
    .await?;
    shared.update(|s| {
        if let Some(wifi) = &mut s.wifi {
            wifi.enabled = enabled;
        }
    });
    Ok(())
}

// --- Bluetooth ---------------------------------------------------------------------

async fn bluetooth(shared: &Shared) {
    follow(
        shared,
        BLUEZ,
        None,
        |bus| async move {
            let powered = match adapter(&bus).await? {
                Some(path) => {
                    Some(prop::<bool>(&bus, BLUEZ, path.as_str(), BLUEZ_ADAPTER, "Powered").await?)
                }
                None => None,
            };
            shared.update(|s| s.bluetooth = powered);
            Ok(())
        },
        |shared| shared.update(|s| s.bluetooth = None),
    )
    .await;
}

/// The first Bluetooth adapter, if any.
async fn adapter(bus: &Connection) -> Result<Option<OwnedObjectPath>> {
    let manager = ObjectManagerProxy::builder(bus)
        .destination(BLUEZ)
        .and_then(|b| b.path("/"))
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let objects = manager
        .get_managed_objects()
        .await
        .map_err(|e| e.to_string())?;
    let mut adapters: Vec<OwnedObjectPath> = objects
        .into_iter()
        .filter(|(_, interfaces)| interfaces.keys().any(|name| name.as_str() == BLUEZ_ADAPTER))
        .map(|(path, _)| path)
        .collect();
    adapters.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    Ok(adapters.into_iter().next())
}

pub(super) async fn set_bluetooth_powered(shared: &Shared, powered: bool) -> Result<()> {
    let bus = shared
        .config
        .system_bus
        .as_ref()
        .ok_or("system bus absent")?;
    let path = adapter(bus).await?.ok_or("no Bluetooth adapter")?;
    set_prop(
        bus,
        BLUEZ,
        path.as_str(),
        BLUEZ_ADAPTER,
        "Powered",
        Value::Bool(powered),
    )
    .await?;
    shared.update(|s| s.bluetooth = Some(powered));
    Ok(())
}

// --- Battery -----------------------------------------------------------------------

async fn power(shared: &Shared) {
    follow(
        shared,
        UPOWER,
        Some(UPOWER_DISPLAY),
        |bus| async move {
            let battery = read_battery(&bus).await?;
            shared.update(|s| s.battery = battery);
            Ok(())
        },
        |shared| shared.update(|s| s.battery = None),
    )
    .await;
}

async fn read_battery(bus: &Connection) -> Result<Option<Battery>> {
    let present: bool = prop(bus, UPOWER, UPOWER_DISPLAY, UPOWER_DEVICE, "IsPresent").await?;
    // Type 2: battery (UPower's aggregate on a desktop can be "line power").
    let kind: u32 = prop(bus, UPOWER, UPOWER_DISPLAY, UPOWER_DEVICE, "Type").await?;
    if !present || kind != 2 {
        return Ok(None);
    }
    let percent: f64 = prop(bus, UPOWER, UPOWER_DISPLAY, UPOWER_DEVICE, "Percentage").await?;
    // State: 1 charging, 4 fully charged, 5 pending charge (plugged in).
    let state: u32 = prop(bus, UPOWER, UPOWER_DISPLAY, UPOWER_DEVICE, "State").await?;
    Ok(Some(Battery {
        percent: if percent.is_finite() {
            percent.clamp(0.0, 100.0)
        } else {
            0.0
        },
        charging: matches!(state, 1 | 4 | 5),
    }))
}

// --- Brightness and audio ----------------------------------------------------------

/// Every how many volume readings the output name is reread (it rarely changes).
const NAME_EVERY: u32 = 5;

async fn poll(shared: &Shared) {
    let mut tick: u32 = 0;
    let mut name = String::new();
    loop {
        let brightness = read_backlight(&shared.config.backlight_dir).map(|b| b.fraction());
        let mut audio = read_audio(&shared.config.wpctl).await;
        if let Some(audio) = &mut audio {
            if (tick.is_multiple_of(NAME_EVERY) || name.is_empty())
                && let Some(read) = read_audio_name(&shared.config.wpctl).await
            {
                name = read;
            }
            audio.name.clone_from(&name);
        }
        tick = tick.wrapping_add(1);
        shared.update(|s| {
            s.brightness = brightness;
            s.audio = audio;
        });
        Timer::after(shared.config.poll).await;
    }
}

/// The volume just reread, with the output name already known.
fn keep_name(shared: &Shared, audio: Option<Audio>) {
    shared.update(|s| {
        let name = s.audio.as_ref().map(|a| a.name.clone()).unwrap_or_default();
        s.audio = audio.map(|mut a| {
            a.name = name;
            a
        });
    });
}

struct Backlight {
    name: String,
    dir: PathBuf,
    value: u64,
    max: u64,
}

impl Backlight {
    fn fraction(&self) -> f64 {
        (self.value.min(self.max) as f64 / self.max as f64).clamp(0.0, 1.0)
    }
}

/// The first backlight with a valid maximum.
fn read_backlight(root: &Path) -> Option<Backlight> {
    let read =
        |path: PathBuf| -> Option<u64> { std::fs::read_to_string(path).ok()?.trim().parse().ok() };
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    entries.into_iter().find_map(|dir| {
        let max = read(dir.join("max_brightness")).filter(|m| *m > 0)?;
        let value = read(dir.join("brightness"))?;
        let name = dir.file_name()?.to_string_lossy().into_owned();
        Some(Backlight {
            name,
            dir,
            value,
            max,
        })
    })
}

pub(super) async fn set_brightness(shared: &Shared, fraction: f64) -> Result<()> {
    let backlight = read_backlight(&shared.config.backlight_dir).ok_or("no backlight")?;
    // Never zero: a black screen cannot be turned back on with a cursor nobody sees.
    let value = ((fraction * backlight.max as f64).round() as u64).clamp(1, backlight.max);
    let via_logind = match &shared.config.system_bus {
        Some(bus) => {
            let value = u32::try_from(value).unwrap_or(u32::MAX);
            bus.call_method(
                Some(LOGIND),
                "/org/freedesktop/login1/session/auto",
                Some("org.freedesktop.login1.Session"),
                "SetBrightness",
                &("backlight", backlight.name.as_str(), value),
            )
            .await
            .is_ok()
        }
        None => false,
    };
    if !via_logind {
        std::fs::write(backlight.dir.join("brightness"), value.to_string())
            .map_err(|e| format!("backlight {}: {e}", backlight.name))?;
    }
    let fraction = value as f64 / backlight.max as f64;
    shared.update(|s| s.brightness = Some(fraction));
    Ok(())
}

pub(super) async fn wpctl(program: &Path, args: &[&str]) -> Result<String> {
    let run = async {
        let output = async_process::Command::new(program)
            .args(args)
            .stdin(async_process::Stdio::null())
            .output()
            .await
            .map_err(|e| format!("{}: {e}", program.display()))?;
        if !output.status.success() {
            return Err(format!("{} {args:?}: {}", program.display(), output.status));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    };
    future::or(run, async {
        Timer::after(WPCTL_TIMEOUT).await;
        Err(format!("{}: no response", program.display()))
    })
    .await
}

/// The output name from `wpctl inspect`: `node.description = "…"`, otherwise
/// `node.nick`; at most 128 characters.
fn parse_audio_name(text: &str) -> Option<String> {
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let (k, v) = line.trim_start_matches([' ', '*']).split_once(" = ")?;
            (k.trim() == key).then(|| v.trim().trim_matches('"').to_owned())
        })
    };
    value("node.description")
        .or_else(|| value("node.nick"))
        .filter(|name| !name.is_empty())
        .map(|name| name.chars().take(128).collect())
}

async fn read_audio_name(program: &Path) -> Option<String> {
    parse_audio_name(&wpctl(program, &["inspect", DEFAULT_SINK]).await.ok()?)
}

/// `Volume: 0.46` or `Volume: 0.46 [MUTED]`.
fn parse_volume(text: &str) -> Option<Audio> {
    let mut words = text.split_whitespace();
    if words.next()? != "Volume:" {
        return None;
    }
    let volume: f64 = words.next()?.parse().ok().filter(|v: &f64| v.is_finite())?;
    Some(Audio {
        volume: volume.clamp(0.0, 1.0),
        muted: text.contains("[MUTED]"),
        name: String::new(),
    })
}

async fn read_audio(program: &Path) -> Option<Audio> {
    parse_volume(&wpctl(program, &["get-volume", DEFAULT_SINK]).await.ok()?)
}

pub(super) async fn set_volume(shared: &Shared, volume: f64) -> Result<()> {
    let program = &shared.config.wpctl;
    wpctl(
        program,
        &["set-volume", DEFAULT_SINK, &format!("{volume:.3}")],
    )
    .await?;
    keep_name(shared, read_audio(program).await);
    Ok(())
}

pub(super) async fn set_muted(shared: &Shared, muted: bool) -> Result<()> {
    let program = &shared.config.wpctl;
    wpctl(
        program,
        &["set-mute", DEFAULT_SINK, if muted { "1" } else { "0" }],
    )
    .await?;
    keep_name(shared, read_audio(program).await);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wpctl_output_is_parsed_defensively() {
        let audio = |volume, muted| Audio {
            volume,
            muted,
            name: String::new(),
        };
        assert_eq!(parse_volume("Volume: 0.46\n"), Some(audio(0.46, false)));
        assert_eq!(parse_volume("Volume: 1.20 [MUTED]"), Some(audio(1.0, true)));
        assert_eq!(
            parse_audio_name(
                "id 52, type PipeWire:Interface:Node\n  * node.description = \"Altoparlanti\"\n    node.nick = \"Speaker\""
            ),
            Some("Altoparlanti".to_owned())
        );
        assert_eq!(
            parse_audio_name("    node.nick = \"Speaker\""),
            Some("Speaker".to_owned())
        );
        assert_eq!(parse_audio_name("niente"), None);
        assert_eq!(parse_volume("Volume: NaN"), None);
        assert_eq!(parse_volume("errore"), None);
        assert_eq!(parse_volume(""), None);
    }

    #[test]
    fn backlight_reads_the_first_valid_device() {
        let root = std::env::temp_dir().join(format!("silica-backlight-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("a_rotto")).unwrap();
        std::fs::write(root.join("a_rotto/max_brightness"), "0").unwrap();
        std::fs::create_dir_all(root.join("intel")).unwrap();
        std::fs::write(root.join("intel/max_brightness"), "1000\n").unwrap();
        std::fs::write(root.join("intel/brightness"), "250\n").unwrap();
        let backlight = read_backlight(&root).unwrap();
        assert_eq!(backlight.name, "intel");
        assert!((backlight.fraction() - 0.25).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&root);
    }
}
