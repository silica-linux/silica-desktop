//! `dev.frascella.Silica.System1` with fake services: NetworkManager, UPower and BlueZ on a
//! private "system" bus, `wpctl` and the backlight in a temporary directory.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use futures_lite::{StreamExt, future};
use silica_daemon::{SystemConfig, SystemService};
use zbus::fdo::ObjectManager;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, interface, proxy};

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

// --- Fake services -----------------------------------------------------------------

struct FakeNm {
    wireless: bool,
}

#[interface(name = "org.freedesktop.NetworkManager")]
impl FakeNm {
    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.wireless
    }
    #[zbus(property)]
    fn set_wireless_enabled(&mut self, value: bool) {
        self.wireless = value;
    }
    #[zbus(property)]
    fn primary_connection(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/ActiveConnection/1").unwrap()
    }
    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/Devices/2").unwrap()]
    }
    fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        _device: OwnedObjectPath,
        _specific: OwnedObjectPath,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        if connection.as_str() != "/org/freedesktop/NetworkManager/Settings/5" {
            return Err(zbus::fdo::Error::InvalidArgs(connection.to_string()));
        }
        Ok(
            OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/ActiveConnection/1")
                .unwrap(),
        )
    }
}

struct FakeSettings;

#[interface(name = "org.freedesktop.NetworkManager.Settings")]
impl FakeSettings {
    fn list_connections(&self) -> Vec<OwnedObjectPath> {
        vec![OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/Settings/5").unwrap()]
    }
}

struct FakeSaved;

#[interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl FakeSaved {
    fn get_settings(&self) -> HashMap<String, HashMap<String, OwnedValue>> {
        let ssid = OwnedValue::try_from(zbus::zvariant::Value::from(b"Ufficio".to_vec())).unwrap();
        HashMap::from([(
            "802-11-wireless".to_owned(),
            HashMap::from([("ssid".to_owned(), ssid)]),
        )])
    }
}

struct FakeDevice;

#[interface(name = "org.freedesktop.NetworkManager.Device")]
impl FakeDevice {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        2
    }
}

struct FakeActive;

#[interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl FakeActive {
    #[zbus(property, name = "Type")]
    fn kind(&self) -> String {
        "802-11-wireless".into()
    }
    #[zbus(property)]
    fn id(&self) -> String {
        "Casa".into()
    }
    #[zbus(property)]
    fn devices(&self) -> Vec<OwnedObjectPath> {
        vec![OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/Devices/2").unwrap()]
    }
}

struct FakeWireless;

#[interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl FakeWireless {
    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/AccessPoint/9").unwrap()
    }
    #[zbus(property)]
    fn access_points(&self) -> Vec<OwnedObjectPath> {
        [9, 10, 11]
            .map(|n| {
                OwnedObjectPath::try_from(format!(
                    "/org/freedesktop/NetworkManager/AccessPoint/{n}"
                ))
                .unwrap()
            })
            .to_vec()
    }
}

struct FakeAp {
    ssid: &'static str,
    strength: u8,
    rsn: u32,
}

#[interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl FakeAp {
    #[zbus(property)]
    fn strength(&self) -> u8 {
        self.strength
    }
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.ssid.as_bytes().to_vec()
    }
    #[zbus(property)]
    fn flags(&self) -> u32 {
        0
    }
    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        0
    }
    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        self.rsn
    }
}

struct FakeBattery;

#[interface(name = "org.freedesktop.UPower.Device")]
impl FakeBattery {
    #[zbus(property)]
    fn is_present(&self) -> bool {
        true
    }
    #[zbus(property, name = "Type")]
    fn kind(&self) -> u32 {
        2
    }
    #[zbus(property)]
    fn percentage(&self) -> f64 {
        80.0
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        1
    }
}

struct FakeAdapter {
    powered: bool,
}

#[interface(name = "org.bluez.Adapter1")]
impl FakeAdapter {
    #[zbus(property)]
    fn powered(&self) -> bool {
        self.powered
    }
    #[zbus(property)]
    fn set_powered(&mut self, value: bool) {
        self.powered = value;
    }
}

/// Fake NetworkManager, UPower and BlueZ, each with its own connection and name.
async fn fake_services(bus: &Bus) -> Vec<Connection> {
    let nm = bus.connect().await;
    let server = nm.object_server();
    server
        .at("/org/freedesktop/NetworkManager", FakeNm { wireless: true })
        .await
        .unwrap();
    server
        .at(
            "/org/freedesktop/NetworkManager/ActiveConnection/1",
            FakeActive,
        )
        .await
        .unwrap();
    server
        .at("/org/freedesktop/NetworkManager/Devices/2", FakeWireless)
        .await
        .unwrap();
    for (n, ssid, strength, rsn) in [
        (9, "Casa", 70, 0x188),
        (10, "Ufficio", 40, 0x188),
        (11, "Bar", 90, 0),
    ] {
        server
            .at(
                format!("/org/freedesktop/NetworkManager/AccessPoint/{n}"),
                FakeAp {
                    ssid,
                    strength,
                    rsn,
                },
            )
            .await
            .unwrap();
    }
    server
        .at("/org/freedesktop/NetworkManager/Devices/2", FakeDevice)
        .await
        .unwrap();
    server
        .at("/org/freedesktop/NetworkManager/Settings", FakeSettings)
        .await
        .unwrap();
    server
        .at("/org/freedesktop/NetworkManager/Settings/5", FakeSaved)
        .await
        .unwrap();
    nm.request_name("org.freedesktop.NetworkManager")
        .await
        .unwrap();

    let upower = bus.connect().await;
    upower
        .object_server()
        .at("/org/freedesktop/UPower/devices/DisplayDevice", FakeBattery)
        .await
        .unwrap();
    upower.request_name("org.freedesktop.UPower").await.unwrap();

    let bluez = bus.connect().await;
    bluez.object_server().at("/", ObjectManager).await.unwrap();
    bluez
        .object_server()
        .at("/org/bluez/hci0", FakeAdapter { powered: true })
        .await
        .unwrap();
    bluez.request_name("org.bluez").await.unwrap();
    vec![nm, upower, bluez]
}

/// Fake `wpctl`: the volume in a file next to the script.
fn fake_wpctl(dir: &Path) -> PathBuf {
    let script = dir.join("wpctl");
    std::fs::write(
        &script,
        r#"#!/bin/sh
S="$(dirname "$0")/volume"
case "$1" in
  get-volume)
    v=$(cat "$S" 2>/dev/null || echo 0.5)
    if [ "$(cat "$S.mute" 2>/dev/null)" = 1 ]; then echo "Volume: $v [MUTED]"; else echo "Volume: $v"; fi ;;
  set-volume) echo "$3" > "$S" ;;
  set-mute) echo "$3" > "$S.mute" ;;
  inspect) echo '  * node.description = "Test speakers"' ;;
  status) printf 'Audio\n ├─ Sinks:\n │      62. Headphones [vol: 1.00]\n │  *   74. Test speakers [vol: 0.50]\n │  \n ├─ Sources:\n' ;;
  set-default) echo "$2" > "$S.default" ;;
  *) exit 1 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("silica-system-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// (name, signal, protected, saved, in use).
type Network = (String, u8, bool, bool, bool);

#[proxy(
    interface = "dev.frascella.Silica.System1",
    default_path = "/dev/frascella/Silica/System1"
)]
trait System {
    fn state(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
    fn set_wifi_enabled(&self, enabled: bool) -> zbus::Result<()>;
    fn set_bluetooth_powered(&self, powered: bool) -> zbus::Result<()>;
    fn set_volume(&self, volume: f64) -> zbus::Result<()>;
    fn set_muted(&self, muted: bool) -> zbus::Result<()>;
    fn set_brightness(&self, brightness: f64) -> zbus::Result<()>;
    fn set_dark(&self, dark: bool) -> zbus::Result<()>;
    fn wifi_networks(&self) -> zbus::Result<Vec<Network>>;
    fn connect_wifi(&self, ssid: &str) -> zbus::Result<()>;
    fn audio_outputs(&self) -> zbus::Result<Vec<(u32, String, bool)>>;
    fn set_audio_output(&self, id: u32) -> zbus::Result<()>;
    fn set_do_not_disturb(&self, enabled: bool) -> zbus::Result<()>;
    #[zbus(signal)]
    fn changed(&self, state: HashMap<String, OwnedValue>) -> zbus::Result<()>;
}

fn get<T: TryFrom<OwnedValue>>(state: &HashMap<String, OwnedValue>, key: &str) -> T
where
    T::Error: std::fmt::Debug,
{
    T::try_from(state[key].try_clone().unwrap()).unwrap()
}

/// Waits until the state satisfies `ready` (the sources start in parallel).
async fn wait_for(
    proxy: &SystemProxy<'_>,
    ready: impl Fn(&HashMap<String, OwnedValue>) -> bool,
) -> HashMap<String, OwnedValue> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let state = proxy.state().await.unwrap();
        if ready(&state) {
            return state;
        }
        assert!(Instant::now() < deadline, "stato mai raggiunto: {state:?}");
        async_io::Timer::after(Duration::from_millis(20)).await;
    }
}

#[test]
fn reads_and_changes_the_system() {
    let session = Bus::start();
    let system = Bus::start();
    let dir = temp_dir("full");
    let backlight = dir.join("backlight/intel");
    std::fs::create_dir_all(&backlight).unwrap();
    std::fs::write(backlight.join("max_brightness"), "1000").unwrap();
    std::fs::write(backlight.join("brightness"), "500").unwrap();
    let wpctl = fake_wpctl(&dir);

    zbus::block_on(async {
        let _fakes = fake_services(&system).await;
        let connection = session.connect().await;
        let name = connection.unique_name().unwrap().to_string();
        let service = SystemService::serve(
            connection,
            SystemConfig {
                system_bus: Some(system.connect().await),
                backlight_dir: dir.join("backlight"),
                wpctl,
                poll: Duration::from_millis(50),
                appearance_file: Some(dir.join("appearance.json")),
            },
        )
        .await
        .unwrap();
        let client = session.connect().await;
        let test = async {
            let proxy = SystemProxy::builder(&client)
                .destination(name)
                .unwrap()
                .build()
                .await
                .unwrap();
            let mut changed = proxy.receive_changed().await.unwrap();

            let state = wait_for(&proxy, |s| {
                get::<bool>(s, "wifi.available")
                    && get::<bool>(s, "battery.present")
                    && get::<bool>(s, "bluetooth.available")
                    && get::<bool>(s, "audio.available")
            })
            .await;
            assert_eq!(get::<String>(&state, "wifi.ssid"), "Casa");
            assert_eq!(get::<u8>(&state, "wifi.strength"), 70);
            assert!(get::<bool>(&state, "wifi.enabled"));
            assert_eq!(get::<f64>(&state, "battery.percent"), 80.0);
            assert!(get::<bool>(&state, "battery.charging"));
            assert!(get::<bool>(&state, "bluetooth.powered"));
            assert!((get::<f64>(&state, "brightness") - 0.5).abs() < 1e-9);
            assert!((get::<f64>(&state, "audio.volume") - 0.5).abs() < 1e-9);
            let state = wait_for(&proxy, |s| !get::<String>(s, "audio.name").is_empty()).await;
            assert_eq!(get::<String>(&state, "audio.name"), "Test speakers");

            proxy.set_wifi_enabled(false).await.unwrap();
            wait_for(&proxy, |s| !get::<bool>(s, "wifi.enabled")).await;
            // Every change also arrives as a signal with the whole state.
            let mut saw_wifi_off = false;
            for _ in 0..20 {
                let signal = changed.next().await.unwrap();
                let state = signal.args().unwrap().state;
                if state.contains_key("wifi.enabled") && !get::<bool>(&state, "wifi.enabled") {
                    saw_wifi_off = true;
                    break;
                }
            }
            assert!(saw_wifi_off);

            proxy.set_bluetooth_powered(false).await.unwrap();
            wait_for(&proxy, |s| !get::<bool>(s, "bluetooth.powered")).await;

            proxy.set_volume(0.3).await.unwrap();
            proxy.set_muted(true).await.unwrap();
            let state = wait_for(&proxy, |s| get::<bool>(s, "audio.muted")).await;
            assert!((get::<f64>(&state, "audio.volume") - 0.3).abs() < 1e-9);
            assert!(proxy.set_volume(f64::NAN).await.is_err());

            // Without logind on the fake bus the brightness is written to sysfs.
            proxy.set_brightness(0.25).await.unwrap();
            let written = std::fs::read_to_string(backlight.join("brightness")).unwrap();
            assert_eq!(written.trim(), "250");
            // Never zero.
            proxy.set_brightness(0.0).await.unwrap();
            let written = std::fs::read_to_string(backlight.join("brightness")).unwrap();
            assert_eq!(written.trim(), "1");
            // Two requests from Silica; a change from outside (adaptive brightness) changes
            // the value but not the requests: the HUD does not appear.
            // The polling may report for one round the value read before the write: it
            // waits for the state with both requests and the written value.
            wait_for(&proxy, |s| {
                get::<u64>(s, "brightness.requests") == 2
                    && (get::<f64>(s, "brightness") - 0.001).abs() < 1e-9
            })
            .await;
            std::fs::write(backlight.join("brightness"), "700").unwrap();
            let state =
                wait_for(&proxy, |s| (get::<f64>(s, "brightness") - 0.7).abs() < 1e-9).await;
            assert_eq!(get::<u64>(&state, "brightness.requests"), 2);

            proxy.set_do_not_disturb(true).await.unwrap();
            wait_for(&proxy, |s| get::<bool>(s, "dnd")).await;

            // The dark appearance: in the state and in the file, for the next start.
            proxy.set_dark(true).await.unwrap();
            wait_for(&proxy, |s| get::<bool>(s, "appearance.dark")).await;
            assert!(silica_daemon::system::read_appearance(
                &dir.join("appearance.json")
            ));

            // Networks: the one in use, then the saved one, then the others by signal.
            let networks = proxy.wifi_networks().await.unwrap();
            assert_eq!(
                networks,
                [
                    ("Casa".to_owned(), 70, true, false, true),
                    ("Ufficio".to_owned(), 40, true, true, false),
                    ("Bar".to_owned(), 90, false, false, false),
                ]
            );
            proxy.connect_wifi("Ufficio").await.unwrap();
            assert!(proxy.connect_wifi("Bar").await.is_err());

            // Audio outputs from `wpctl status`.
            let outputs = proxy.audio_outputs().await.unwrap();
            assert_eq!(
                outputs,
                [
                    (62, "Headphones".to_owned(), false),
                    (74, "Test speakers".to_owned(), true),
                ]
            );
            proxy.set_audio_output(62).await.unwrap();
            let chosen = std::fs::read_to_string(dir.join("volume.default")).unwrap();
            assert_eq!(chosen.trim(), "62");
            assert!(proxy.set_audio_output(999).await.is_err());
        };
        future::or(test, async {
            let result = service.run().await;
            panic!("the service ended: {result:?}");
        })
        .await;
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_sources_are_absent_not_errors() {
    let session = Bus::start();
    let dir = temp_dir("empty");
    zbus::block_on(async {
        let connection = session.connect().await;
        let name = connection.unique_name().unwrap().to_string();
        let service = SystemService::serve(
            connection,
            SystemConfig {
                system_bus: None,
                backlight_dir: dir.join("nessuna"),
                wpctl: dir.join("nessun-wpctl"),
                poll: Duration::from_millis(50),
                appearance_file: Some(dir.join("appearance.json")),
            },
        )
        .await
        .unwrap();
        let client = session.connect().await;
        let test = async {
            let proxy = SystemProxy::builder(&client)
                .destination(name)
                .unwrap()
                .build()
                .await
                .unwrap();
            async_io::Timer::after(Duration::from_millis(200)).await;
            let state = proxy.state().await.unwrap();
            for key in [
                "wifi.available",
                "bluetooth.available",
                "battery.present",
                "brightness.available",
                "audio.available",
            ] {
                assert!(!get::<bool>(&state, key), "{key}");
            }
            assert!(proxy.set_wifi_enabled(true).await.is_err());
            assert!(proxy.set_brightness(0.5).await.is_err());
            assert!(proxy.set_volume(0.5).await.is_err());
        };
        future::or(test, async {
            let result = service.run().await;
            panic!("the service ended: {result:?}");
        })
        .await;
    });
    let _ = std::fs::remove_dir_all(&dir);
}
