//! The data of the Control Center detail panels, read on demand (they are not part of
//! the state emitted on every change): the Wi-Fi networks visible to NetworkManager, the
//! paired Bluetooth devices (BlueZ) and PipeWire's audio outputs (`wpctl status`).

use std::collections::HashMap;
use std::path::Path;

use zbus::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::Shared;
use super::backends::{prop, wpctl};

const NM: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const NM_DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const NM_WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const NM_AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
const NM_SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const NM_SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
const NM_CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";
/// `NM_DEVICE_TYPE_WIFI`.
const DEVICE_WIFI: u32 = 2;
/// At most this many networks in the list: a crowded place must not bloat the answer.
const MAX_NETWORKS: usize = 40;
const MAX_OUTPUTS: usize = 32;
const MAX_DEVICES: usize = 32;
const BLUEZ: &str = "org.bluez";
const BLUEZ_DEVICE: &str = "org.bluez.Device1";

type Result<T> = std::result::Result<T, String>;

/// A visible wireless network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    /// 0–100.
    pub strength: u8,
    /// Protected by a password.
    pub secure: bool,
    /// There is a saved connection with this name.
    pub known: bool,
    /// It is the one in use.
    pub active: bool,
}

/// An audio output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub id: u32,
    pub name: String,
    pub default: bool,
}

/// A paired Bluetooth device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub address: String,
    pub name: String,
    /// The freedesktop icon name BlueZ gives (`input-mouse`, `audio-headset`…).
    pub icon: String,
    pub connected: bool,
}

type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

/// The paired devices among BlueZ's objects, connected first, then by name.
pub fn paired_devices(objects: &ManagedObjects) -> Vec<(OwnedObjectPath, Device)> {
    let text = |props: &HashMap<String, OwnedValue>, key: &str| {
        props
            .get(key)
            .and_then(|v| String::try_from(v.clone()).ok())
            .unwrap_or_default()
    };
    let flag = |props: &HashMap<String, OwnedValue>, key: &str| {
        props
            .get(key)
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    };
    let mut list: Vec<(OwnedObjectPath, Device)> = objects
        .iter()
        .filter_map(|(path, interfaces)| {
            let props = interfaces.get(BLUEZ_DEVICE)?;
            if !flag(props, "Paired") {
                return None;
            }
            let address = text(props, "Address");
            let alias = text(props, "Alias");
            let name = if alias.is_empty() {
                text(props, "Name")
            } else {
                alias
            };
            Some((
                path.clone(),
                Device {
                    name: if name.is_empty() {
                        address.clone()
                    } else {
                        name
                    }
                    .chars()
                    .take(128)
                    .collect(),
                    address,
                    icon: text(props, "Icon").chars().take(64).collect(),
                    connected: flag(props, "Connected"),
                },
            ))
        })
        .collect();
    list.sort_by(|(_, a), (_, b)| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    list.truncate(MAX_DEVICES);
    list
}

async fn bluez_objects(bus: &Connection) -> Result<ManagedObjects> {
    let reply = bus
        .call_method(
            Some(BLUEZ),
            "/",
            Some("org.freedesktop.DBus.ObjectManager"),
            "GetManagedObjects",
            &(),
        )
        .await
        .map_err(|e| format!("GetManagedObjects: {e}"))?;
    reply
        .body()
        .deserialize::<ManagedObjects>()
        .map_err(|e| format!("GetManagedObjects: {e}"))
}

pub(super) async fn bluetooth_devices(shared: &Shared) -> Result<Vec<Device>> {
    let bus = bus(shared)?;
    let objects = bluez_objects(bus).await?;
    Ok(paired_devices(&objects)
        .into_iter()
        .map(|(_, d)| d)
        .collect())
}

/// Connects (or disconnects) a paired device, by address.
pub(super) async fn connect_bluetooth(shared: &Shared, address: &str, connect: bool) -> Result<()> {
    let bus = bus(shared)?;
    let objects = bluez_objects(bus).await?;
    let (path, _) = paired_devices(&objects)
        .into_iter()
        .find(|(_, d)| d.address == address)
        .ok_or_else(|| format!("{address}: not a paired device"))?;
    bus.call_method(
        Some(BLUEZ),
        path.as_str(),
        Some(BLUEZ_DEVICE),
        if connect { "Connect" } else { "Disconnect" },
        &(),
    )
    .await
    .map_err(|e| format!("{BLUEZ_DEVICE}: {e}"))?;
    Ok(())
}

fn bus(shared: &Shared) -> Result<&Connection> {
    shared
        .config
        .system_bus
        .as_ref()
        .ok_or_else(|| "system bus absent".to_owned())
}

async fn call<B, R>(bus: &Connection, path: &str, iface: &str, method: &str, body: &B) -> Result<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let reply = bus
        .call_method(Some(NM), path, Some(iface), method, body)
        .await
        .map_err(|e| format!("{iface}.{method}: {e}"))?;
    reply
        .body()
        .deserialize::<R>()
        .map_err(|e| format!("{iface}.{method}: {e}"))
}

/// The saved wireless connections: network name → path.
async fn saved_networks(bus: &Connection) -> HashMap<String, OwnedObjectPath> {
    let mut saved = HashMap::new();
    let Ok(list) =
        call::<_, Vec<OwnedObjectPath>>(bus, NM_SETTINGS_PATH, NM_SETTINGS, "ListConnections", &())
            .await
    else {
        return saved;
    };
    for path in list {
        let Ok(settings) = call::<_, HashMap<String, HashMap<String, OwnedValue>>>(
            bus,
            path.as_str(),
            NM_CONNECTION,
            "GetSettings",
            &(),
        )
        .await
        else {
            continue;
        };
        let ssid = settings
            .get("802-11-wireless")
            .and_then(|wifi| wifi.get("ssid"))
            .and_then(|value| Vec::<u8>::try_from(value.clone()).ok())
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        if let Some(ssid) = ssid.filter(|s| !s.is_empty()) {
            saved.entry(ssid).or_insert(path);
        }
    }
    saved
}

/// The visible networks: the one in use first, then the saved ones, then by signal; one per name.
pub(super) async fn wifi_networks(shared: &Shared) -> Result<Vec<Network>> {
    let bus = bus(shared)?;
    let devices: Vec<OwnedObjectPath> = call(bus, NM_PATH, NM, "GetDevices", &()).await?;
    let saved = saved_networks(bus).await;
    let mut networks: HashMap<String, Network> = HashMap::new();
    for device in devices {
        let Ok(kind) = prop::<u32>(bus, NM, device.as_str(), NM_DEVICE, "DeviceType").await else {
            continue;
        };
        if kind != DEVICE_WIFI {
            continue;
        }
        let active =
            prop::<OwnedObjectPath>(bus, NM, device.as_str(), NM_WIRELESS, "ActiveAccessPoint")
                .await
                .ok();
        let aps: Vec<OwnedObjectPath> =
            prop(bus, NM, device.as_str(), NM_WIRELESS, "AccessPoints").await?;
        for ap in aps {
            let Ok(raw) = prop::<Vec<u8>>(bus, NM, ap.as_str(), NM_AP, "Ssid").await else {
                continue;
            };
            let ssid: String = String::from_utf8_lossy(&raw).chars().take(64).collect();
            if ssid.trim().is_empty() {
                continue;
            }
            let strength = prop::<u8>(bus, NM, ap.as_str(), NM_AP, "Strength")
                .await
                .unwrap_or(0)
                .min(100);
            let flags = prop::<u32>(bus, NM, ap.as_str(), NM_AP, "Flags")
                .await
                .unwrap_or(0);
            let wpa = prop::<u32>(bus, NM, ap.as_str(), NM_AP, "WpaFlags")
                .await
                .unwrap_or(0);
            let rsn = prop::<u32>(bus, NM, ap.as_str(), NM_AP, "RsnFlags")
                .await
                .unwrap_or(0);
            let is_active = active.as_ref() == Some(&ap);
            let network = Network {
                known: saved.contains_key(&ssid),
                ssid: ssid.clone(),
                strength,
                secure: flags & 1 != 0 || wpa != 0 || rsn != 0,
                active: is_active,
            };
            networks
                .entry(ssid)
                .and_modify(|seen| {
                    seen.active |= network.active;
                    seen.strength = seen.strength.max(network.strength);
                })
                .or_insert(network);
        }
    }
    let mut list: Vec<Network> = networks.into_values().collect();
    sort_networks(&mut list);
    list.truncate(MAX_NETWORKS);
    Ok(list)
}

fn sort_networks(list: &mut [Network]) {
    list.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then(b.known.cmp(&a.known))
            .then(b.strength.cmp(&a.strength))
            .then(a.ssid.cmp(&b.ssid))
    });
}

/// Connects to a saved network. A new network asks for a password, which is not available here.
pub(super) async fn connect_wifi(shared: &Shared, ssid: &str) -> Result<()> {
    let bus = bus(shared)?;
    let saved = saved_networks(bus).await;
    let connection = saved
        .get(ssid)
        .ok_or_else(|| format!("{ssid}: network not saved (a password is needed)"))?;
    let root = OwnedObjectPath::try_from("/").map_err(|e| e.to_string())?;
    let _: OwnedObjectPath = call(
        bus,
        NM_PATH,
        NM,
        "ActivateConnection",
        &(connection, &root, &root),
    )
    .await?;
    Ok(())
}

/// The outputs (`Sinks:` in the `Audio` section of `wpctl status`); `*` marks the default.
pub fn parse_outputs(text: &str) -> Vec<Output> {
    let mut outputs = Vec::new();
    let mut in_audio = false;
    let mut in_sinks = false;
    for line in text.lines() {
        if !line.starts_with([' ', '│', '├', '└']) {
            in_audio = line.trim() == "Audio";
            in_sinks = false;
            continue;
        }
        if !in_audio {
            continue;
        }
        let body = line.trim_start_matches([' ', '│', '├', '└', '─']);
        if body.ends_with(':') {
            in_sinks = body == "Sinks:";
            continue;
        }
        if !in_sinks {
            continue;
        }
        let default = body.starts_with('*');
        let body = body.trim_start_matches(['*', ' ']);
        let Some((id, rest)) = body.split_once(". ") else {
            continue;
        };
        let Ok(id) = id.trim().parse::<u32>() else {
            continue;
        };
        let name = rest.split(" [vol:").next().unwrap_or(rest).trim();
        if name.is_empty() {
            continue;
        }
        outputs.push(Output {
            id,
            name: name.chars().take(128).collect(),
            default,
        });
        if outputs.len() >= MAX_OUTPUTS {
            break;
        }
    }
    outputs
}

pub(super) async fn audio_outputs(program: &Path) -> Result<Vec<Output>> {
    Ok(parse_outputs(&wpctl(program, &["status"]).await?))
}

pub(super) async fn set_audio_output(program: &Path, id: u32) -> Result<()> {
    let outputs = audio_outputs(program).await?;
    if !outputs.iter().any(|o| o.id == id) {
        return Err(format!("output {id} absent"));
    }
    wpctl(program, &["set-default", &id.to_string()]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wpctl_status_gives_the_sinks() {
        let text = "\
PipeWire 'pipewire-0' [1.6.2]
 └─ Clients:
        33. WirePlumber                         [1.6.2]

Audio
 ├─ Devices:
 │      85. Raptor Lake-P/U/H cAVS              [alsa]
 │
 ├─ Sinks:
 │      62. HDMI / DisplayPort 2 Output [vol: 1.00]
 │  *   74. Speaker      [vol: 0.83]
 │
 ├─ Sources:
 │  *   72. Digital Microphone [vol: 1.00]
 │
 └─ Streams:
        94. Firefox

Video
 ├─ Sinks:
 │      12. Non audio
";
        let outputs = parse_outputs(text);
        assert_eq!(
            outputs,
            [
                Output {
                    id: 62,
                    name: "HDMI / DisplayPort 2 Output".into(),
                    default: false
                },
                Output {
                    id: 74,
                    name: "Speaker".into(),
                    default: true
                },
            ]
        );
        assert!(parse_outputs("").is_empty());
        assert!(parse_outputs("Audio\n ├─ Sinks:\n │  *   x. broken\n").is_empty());
    }

    #[test]
    fn paired_devices_connected_first_then_by_name() -> std::result::Result<(), String> {
        let device = |address: &str, alias: &str, paired: bool, connected: bool| {
            let mut props = HashMap::new();
            let value =
                |v: zbus::zvariant::Value<'_>| OwnedValue::try_from(v).map_err(|e| e.to_string());
            props.insert("Address".to_owned(), value(address.into())?);
            props.insert("Alias".to_owned(), value(alias.into())?);
            props.insert("Icon".to_owned(), value("input-mouse".into())?);
            props.insert("Paired".to_owned(), value(paired.into())?);
            props.insert("Connected".to_owned(), value(connected.into())?);
            let mut interfaces = HashMap::new();
            interfaces.insert(BLUEZ_DEVICE.to_owned(), props);
            let path = OwnedObjectPath::try_from(format!(
                "/org/bluez/hci0/dev_{}",
                address.replace(':', "_")
            ))
            .map_err(|e| e.to_string())?;
            Ok::<_, String>((path, interfaces))
        };
        let mut objects = HashMap::new();
        for (a, n, p, c) in [
            ("00:00:00:00:00:01", "Zeta mouse", true, false),
            ("00:00:00:00:00:02", "headphones", true, true),
            ("00:00:00:00:00:03", "Alpha keyboard", true, false),
            ("00:00:00:00:00:04", "stranger", false, false),
        ] {
            let (path, interfaces) = device(a, n, p, c)?;
            objects.insert(path, interfaces);
        }
        let names: Vec<_> = paired_devices(&objects)
            .into_iter()
            .map(|(_, d)| d.name)
            .collect();
        assert_eq!(names, ["headphones", "Alpha keyboard", "Zeta mouse"]);
        Ok(())
    }

    #[test]
    fn networks_sort_active_known_then_strength() {
        let net = |ssid: &str, strength, known, active| Network {
            ssid: ssid.into(),
            strength,
            secure: true,
            known,
            active,
        };
        let mut list = vec![
            net("strong", 90, false, false),
            net("saved", 40, true, false),
            net("home", 60, true, true),
            net("weak", 10, false, false),
        ];
        sort_networks(&mut list);
        let names: Vec<_> = list.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(names, ["home", "saved", "strong", "weak"]);
    }
}
