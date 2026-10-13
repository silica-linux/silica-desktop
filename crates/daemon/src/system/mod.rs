//! Lo stato del sistema per il Centro di controllo e la barra dei menu:
//! `dev.frascella.Silica.System1` su `/dev/frascella/Silica/System1`.
//!
//! Le sorgenti sono i servizi del bus di sistema (NetworkManager, BlueZ, UPower, logind),
//! la retroilluminazione in sysfs e l'audio di PipeWire con `wpctl` (`backends.rs`). Ogni
//! sorgente è facoltativa: se manca, le sue chiavi dicono `available = false` e il resto
//! funziona. Lo stato si legge tutto insieme (`State`) e ogni cambiamento emette `Changed`
//! con lo stato intero: per la shell è più semplice di un segnale per proprietà.

mod backends;
mod details;

pub use details::{Network, Output, parse_outputs};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use event_listener::Event;
use futures_lite::future;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, interface};

pub const SYSTEM_PATH: &str = "/dev/frascella/Silica/System1";
pub const SYSTEM_INTERFACE: &str = "dev.frascella.Silica.System1";

/// Dove il daemon trova le sorgenti; i test le sostituiscono con servizi finti.
#[derive(Clone, Debug)]
pub struct SystemConfig {
    /// Bus di sistema (NetworkManager, BlueZ, UPower, logind); `None` se non c'è.
    pub system_bus: Option<Connection>,
    /// La directory delle retroilluminazioni (`/sys/class/backlight`).
    pub backlight_dir: PathBuf,
    /// Il programma `wpctl` di PipeWire.
    pub wpctl: PathBuf,
    /// Ogni quanto rileggere luminosità e volume, che non hanno segnali.
    pub poll: Duration,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            system_bus: None,
            backlight_dir: PathBuf::from("/sys/class/backlight"),
            wpctl: PathBuf::from("wpctl"),
            poll: Duration::from_secs(1),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wifi {
    pub enabled: bool,
    /// Il nome della connessione principale, se è senza fili.
    pub ssid: String,
    /// 0–100.
    pub strength: u8,
    /// Connessione principale via cavo.
    pub wired: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Battery {
    pub percent: f64,
    pub charging: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Audio {
    /// 0–1.
    pub volume: f64,
    pub muted: bool,
    /// Il nome dell'uscita predefinita (`node.description` di PipeWire).
    pub name: String,
}

/// Lo stato intero; `None` vuol dire sorgente assente.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemState {
    pub wifi: Option<Wifi>,
    pub bluetooth: Option<bool>,
    pub battery: Option<Battery>,
    /// 0–1.
    pub brightness: Option<f64>,
    pub audio: Option<Audio>,
    pub do_not_disturb: bool,
}

impl SystemState {
    /// Il dizionario di `State` e `Changed`: chiavi piatte, `*.available` per sorgente.
    pub fn to_dict(&self) -> HashMap<String, OwnedValue> {
        let mut dict = HashMap::new();
        let mut put = |key: &str, value: Value<'_>| {
            if let Ok(owned) = OwnedValue::try_from(value) {
                dict.insert(key.to_owned(), owned);
            }
        };
        let wifi = self.wifi.clone().unwrap_or_default();
        put("wifi.available", Value::Bool(self.wifi.is_some()));
        put("wifi.enabled", Value::Bool(wifi.enabled));
        put("wifi.ssid", Value::from(wifi.ssid));
        put("wifi.strength", Value::U8(wifi.strength));
        put("wifi.wired", Value::Bool(wifi.wired));
        put("bluetooth.available", Value::Bool(self.bluetooth.is_some()));
        put(
            "bluetooth.powered",
            Value::Bool(self.bluetooth.unwrap_or(false)),
        );
        let battery = self.battery.clone().unwrap_or_default();
        put("battery.present", Value::Bool(self.battery.is_some()));
        put("battery.percent", Value::F64(battery.percent));
        put("battery.charging", Value::Bool(battery.charging));
        put(
            "brightness.available",
            Value::Bool(self.brightness.is_some()),
        );
        put("brightness", Value::F64(self.brightness.unwrap_or(0.0)));
        let audio = self.audio.clone().unwrap_or_default();
        put("audio.available", Value::Bool(self.audio.is_some()));
        put("audio.volume", Value::F64(audio.volume));
        put("audio.muted", Value::Bool(audio.muted));
        put("audio.name", Value::from(audio.name));
        put("dnd", Value::Bool(self.do_not_disturb));
        dict
    }
}

#[derive(Clone)]
struct Shared {
    state: Arc<Mutex<SystemState>>,
    /// Sveglia il ciclo che emette `Changed`.
    changed: Arc<Event>,
    config: SystemConfig,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, SystemState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Cambia lo stato; se è diverso sveglia l'emissione di `Changed`.
    fn update(&self, change: impl FnOnce(&mut SystemState)) {
        let mut state = self.state();
        let before = state.clone();
        change(&mut state);
        if *state != before {
            drop(state);
            self.changed.notify(usize::MAX);
        }
    }
}

/// Il servizio `System1` su una connessione al bus di sessione.
pub struct SystemService {
    connection: Connection,
    shared: Shared,
}

impl SystemService {
    /// Pubblica l'interfaccia; le sorgenti partono con `run`.
    pub async fn serve(connection: Connection, config: SystemConfig) -> zbus::Result<Self> {
        let shared = Shared {
            state: Arc::new(Mutex::new(SystemState::default())),
            changed: Arc::new(Event::new()),
            config,
        };
        connection
            .object_server()
            .at(
                SYSTEM_PATH,
                System {
                    shared: shared.clone(),
                },
            )
            .await?;
        Ok(Self { connection, shared })
    }

    pub fn state(&self) -> SystemState {
        self.shared.state().clone()
    }

    /// Legge le sorgenti e ne segue i cambiamenti; non ritorna se non per un errore del
    /// bus di sessione.
    pub async fn run(&self) -> zbus::Result<()> {
        let sources = backends::run(&self.shared);
        let emit = async {
            let mut last = None;
            loop {
                let listener = self.shared.changed.listen();
                let state = self.shared.state().clone();
                if last.as_ref() != Some(&state) {
                    let emitter = SignalEmitter::new(&self.connection, SYSTEM_PATH)?;
                    System::changed(&emitter, state.to_dict()).await?;
                    last = Some(state);
                }
                listener.await;
            }
        };
        future::or(
            async {
                sources.await;
                future::pending::<zbus::Result<()>>().await
            },
            emit,
        )
        .await
    }
}

struct System {
    shared: Shared,
}

fn failed(error: impl std::fmt::Display) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}

#[interface(name = "dev.frascella.Silica.System1")]
impl System {
    /// Lo stato intero.
    fn state(&self) -> HashMap<String, OwnedValue> {
        self.shared.state().to_dict()
    }

    async fn set_wifi_enabled(&self, enabled: bool) -> zbus::fdo::Result<()> {
        backends::set_wifi_enabled(&self.shared, enabled)
            .await
            .map_err(failed)
    }

    async fn set_bluetooth_powered(&self, powered: bool) -> zbus::fdo::Result<()> {
        backends::set_bluetooth_powered(&self.shared, powered)
            .await
            .map_err(failed)
    }

    /// Volume dell'uscita predefinita, 0–1.
    async fn set_volume(&self, volume: f64) -> zbus::fdo::Result<()> {
        if !volume.is_finite() {
            return Err(zbus::fdo::Error::InvalidArgs("volume non finito".into()));
        }
        backends::set_volume(&self.shared, volume.clamp(0.0, 1.0))
            .await
            .map_err(failed)
    }

    async fn set_muted(&self, muted: bool) -> zbus::fdo::Result<()> {
        backends::set_muted(&self.shared, muted)
            .await
            .map_err(failed)
    }

    /// Luminosità dello schermo, 0–1.
    async fn set_brightness(&self, brightness: f64) -> zbus::fdo::Result<()> {
        if !brightness.is_finite() {
            return Err(zbus::fdo::Error::InvalidArgs(
                "luminosità non finita".into(),
            ));
        }
        backends::set_brightness(&self.shared, brightness.clamp(0.0, 1.0))
            .await
            .map_err(failed)
    }

    /// Le reti Wi-Fi visibili: (nome, segnale 0–100, protetta, salvata, in uso), quella in
    /// uso prima, poi le salvate, poi per segnale.
    async fn wifi_networks(&self) -> zbus::fdo::Result<Vec<(String, u8, bool, bool, bool)>> {
        let list = details::wifi_networks(&self.shared).await.map_err(failed)?;
        Ok(list
            .into_iter()
            .map(|n| (n.ssid, n.strength, n.secure, n.known, n.active))
            .collect())
    }

    /// Si collega a una rete salvata.
    async fn connect_wifi(&self, ssid: String) -> zbus::fdo::Result<()> {
        details::connect_wifi(&self.shared, &ssid)
            .await
            .map_err(failed)
    }

    /// Le uscite audio: (id, nome, predefinita).
    async fn audio_outputs(&self) -> zbus::fdo::Result<Vec<(u32, String, bool)>> {
        let list = details::audio_outputs(&self.shared.config.wpctl)
            .await
            .map_err(failed)?;
        Ok(list
            .into_iter()
            .map(|o| (o.id, o.name, o.default))
            .collect())
    }

    /// Sceglie l'uscita audio predefinita.
    async fn set_audio_output(&self, id: u32) -> zbus::fdo::Result<()> {
        details::set_audio_output(&self.shared.config.wpctl, id)
            .await
            .map_err(failed)
    }

    /// Non disturbare: la shell non mostra i banner.
    fn set_do_not_disturb(&self, enabled: bool) {
        self.shared.update(|state| state.do_not_disturb = enabled);
    }

    #[zbus(signal)]
    async fn changed(
        emitter: &SignalEmitter<'_>,
        state: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()>;
}
