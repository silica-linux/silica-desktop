//! `dev.frascella.Silica.System1` con servizi finti: NetworkManager, UPower e BlueZ su un
//! bus "di sistema" privato, `wpctl` e la retroilluminazione in una directory temporanea.

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
            .expect("dbus-daemon (pacchetto dbus) è necessario per questi test");
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

// --- Servizi finti -----------------------------------------------------------------

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
}

struct FakeAp;

#[interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl FakeAp {
    #[zbus(property)]
    fn strength(&self) -> u8 {
        70
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

/// NetworkManager, UPower e BlueZ finti, ciascuno con la sua connessione e il suo nome.
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
    server
        .at("/org/freedesktop/NetworkManager/AccessPoint/9", FakeAp)
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

/// `wpctl` finto: il volume in un file accanto allo script.
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

/// Attende che lo stato soddisfi `ready` (le sorgenti partono in parallelo).
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
    let dir = temp_dir("pieno");
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

            proxy.set_wifi_enabled(false).await.unwrap();
            wait_for(&proxy, |s| !get::<bool>(s, "wifi.enabled")).await;
            // Ogni cambiamento arriva anche come segnale con lo stato intero.
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

            // Senza logind sul bus finto la luminosità si scrive in sysfs.
            proxy.set_brightness(0.25).await.unwrap();
            let written = std::fs::read_to_string(backlight.join("brightness")).unwrap();
            assert_eq!(written.trim(), "250");
            // Mai a zero.
            proxy.set_brightness(0.0).await.unwrap();
            let written = std::fs::read_to_string(backlight.join("brightness")).unwrap();
            assert_eq!(written.trim(), "1");

            proxy.set_do_not_disturb(true).await.unwrap();
            wait_for(&proxy, |s| get::<bool>(s, "dnd")).await;
        };
        future::or(test, async {
            let result = service.run().await;
            panic!("il servizio è finito: {result:?}");
        })
        .await;
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_sources_are_absent_not_errors() {
    let session = Bus::start();
    let dir = temp_dir("vuoto");
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
            panic!("il servizio è finito: {result:?}");
        })
        .await;
    });
    let _ = std::fs::remove_dir_all(&dir);
}
