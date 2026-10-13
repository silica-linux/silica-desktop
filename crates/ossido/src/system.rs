//! I comandi `system.*`: Wi-Fi, Bluetooth, volume, luminosità, Non disturbare.
//!
//! Lo stato e le azioni sono di `silica-daemon` (`dev.frascella.Silica.System1` sul bus di
//! sessione, `docs/daemon.md`). Ossido non aspetta mai il daemon nel ciclo degli eventi: un
//! thread tiene in cache lo stato (lettura iniziale, poi i segnali `Changed`), un altro
//! esegue le chiamate in coda. I comandi rispondono subito: `system.status` dalla cache, le
//! azioni con `queued`.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value as Json};
use tracing::{debug, warn};
use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type as MessageType;
use zbus::zvariant::{OwnedValue, Value};

const DESTINATION: &str = "dev.frascella.Silica";
const PATH: &str = "/dev/frascella/Silica/System1";
const INTERFACE: &str = "dev.frascella.Silica.System1";
const RETRY: Duration = Duration::from_secs(2);

/// Passo dei tasti di volume e luminosità: 16 tacche fra 0 e il massimo (misurato: 16
/// pressioni da 0 al massimo).
pub const STEP: f64 = 1.0 / 16.0;

/// Con ⌥⇧ il passo è un quarto (misurato: 50 → 52, 53, 55, 56, 58).
pub const FINE_STEP: f64 = STEP / 4.0;

/// Una chiamata al daemon.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Wifi(bool),
    Bluetooth(bool),
    Volume(f64),
    Muted(bool),
    Brightness(f64),
    DoNotDisturb(bool),
}

impl Call {
    fn send(&self, connection: &Connection) -> zbus::Result<()> {
        match *self {
            Call::Wifi(on) => set(connection, "SetWifiEnabled", on),
            Call::Bluetooth(on) => set(connection, "SetBluetoothPowered", on),
            Call::Volume(v) => set(connection, "SetVolume", v),
            Call::Muted(on) => set(connection, "SetMuted", on),
            Call::Brightness(v) => set(connection, "SetBrightness", v),
            Call::DoNotDisturb(on) => set(connection, "SetDoNotDisturb", on),
        }
    }
}

fn set<T>(connection: &Connection, method: &str, value: T) -> zbus::Result<()>
where
    T: zbus::export::serde::Serialize + zbus::zvariant::Type,
{
    connection
        .call_method(Some(DESTINATION), PATH, Some(INTERFACE), method, &(value,))
        .map(|_| ())
}

/// Il collegamento con `silica-daemon`.
pub struct SystemLink {
    calls: Sender<Call>,
    cache: Arc<Mutex<Option<Json>>>,
}

impl SystemLink {
    /// Parte con il bus di sessione `address` (quello privato di Ossido) o, senza, con
    /// quello dell'ambiente.
    pub fn start(address: Option<String>) -> Self {
        let cache = Arc::new(Mutex::new(None));
        let (calls, receiver) = mpsc::channel();
        let listener_cache = cache.clone();
        let listener_address = address.clone();
        let spawned = std::thread::Builder::new()
            .name("ossido-system".to_owned())
            .spawn(move || listen(listener_address, &listener_cache))
            .and_then(|_| {
                std::thread::Builder::new()
                    .name("ossido-system-calls".to_owned())
                    .spawn(move || serve_calls(address, &receiver))
            });
        if let Err(err) = spawned {
            warn!("thread di system.*: {err}");
        }
        Self { calls, cache }
    }

    /// Prima dell'avvio del ciclo: nessun thread, ogni comando fallisce.
    pub fn disconnected() -> Self {
        let (calls, _) = mpsc::channel();
        Self {
            calls,
            cache: Arc::new(Mutex::new(None)),
        }
    }

    /// L'ultimo stato noto, se il daemon ha risposto almeno una volta.
    pub fn status(&self) -> Option<Json> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn queue(&self, call: Call) -> bool {
        self.calls.send(call).is_ok()
    }

    fn number(&self, key: &str) -> Option<f64> {
        self.status()?.get(key)?.as_f64()
    }

    fn flag(&self, key: &str) -> Option<bool> {
        self.status()?.get(key)?.as_bool()
    }

    /// Volume più o meno un passo (o un quarto con `fine`); alzarlo toglie il muto.
    pub fn volume_step(&self, up: bool, fine: bool) -> bool {
        let Some(volume) = self.number("audio.volume") else {
            return false;
        };
        let target = step_by(volume, up, if fine { FINE_STEP } else { STEP });
        if up && self.flag("audio.muted") == Some(true) {
            self.queue(Call::Muted(false));
        }
        self.queue(Call::Volume(target))
    }

    pub fn mute_toggle(&self) -> bool {
        match self.flag("audio.muted") {
            Some(muted) => self.queue(Call::Muted(!muted)),
            None => false,
        }
    }

    pub fn brightness_step(&self, up: bool, fine: bool) -> bool {
        let size = if fine { FINE_STEP } else { STEP };
        match self.number("brightness") {
            Some(value) => self.queue(Call::Brightness(step_by(value, up, size))),
            None => false,
        }
    }
}

/// Un passo di 1/16 verso l'alto o il basso, agganciato alle tacche.
#[cfg(test)]
fn step(value: f64, up: bool) -> f64 {
    step_by(value, up, STEP)
}

/// Un passo di `size` agganciato alle sue tacche.
pub fn step_by(value: f64, up: bool, size: f64) -> f64 {
    let notch = (value / size).round();
    let next = if up { notch + 1.0 } else { notch - 1.0 };
    (next * size).clamp(0.0, 1.0)
}

fn connect(address: Option<&str>) -> zbus::Result<Connection> {
    match address {
        Some(address) => zbus::blocking::connection::Builder::address(address)?.build(),
        None => Connection::session(),
    }
}

/// Lo stato in cache: `State` all'inizio, poi ogni `Changed`. Se il daemon o il bus
/// spariscono si riprova.
fn listen(address: Option<String>, cache: &Mutex<Option<Json>>) {
    let store = |state: Option<Json>| {
        *cache.lock().unwrap_or_else(|e| e.into_inner()) = state;
    };
    loop {
        let result = (|| -> zbus::Result<()> {
            let connection = connect(address.as_deref())?;
            let rule = zbus::MatchRule::builder()
                .msg_type(MessageType::Signal)
                .interface(INTERFACE)?
                .member("Changed")?
                .path(PATH)?
                .build();
            let signals = MessageIterator::for_match_rule(rule, &connection, Some(16))?;
            let reply =
                connection.call_method(Some(DESTINATION), PATH, Some(INTERFACE), "State", &())?;
            store(Some(to_json(&reply.body().deserialize()?)));
            for message in signals {
                let message = message?;
                let state: HashMap<String, OwnedValue> = message.body().deserialize()?;
                store(Some(to_json(&state)));
            }
            Ok(())
        })();
        if let Err(err) = result {
            debug!("silica-daemon (System1): {err}");
        }
        store(None);
        std::thread::sleep(RETRY);
    }
}

fn serve_calls(address: Option<String>, calls: &Receiver<Call>) {
    let mut connection: Option<Connection> = None;
    while let Ok(call) = calls.recv() {
        if connection.is_none() {
            connection = connect(address.as_deref())
                .map_err(|err| warn!("system.*: bus di sessione: {err}"))
                .ok();
        }
        let Some(conn) = &connection else { continue };
        if let Err(err) = call.send(conn) {
            warn!("system.*: {call:?}: {err}");
            if matches!(err, zbus::Error::InputOutput(_)) {
                connection = None;
            }
        }
    }
}

fn to_json(state: &HashMap<String, OwnedValue>) -> Json {
    let mut map = Map::new();
    for (key, value) in state {
        let json = match &**value {
            Value::Bool(v) => Json::from(*v),
            Value::U8(v) => Json::from(*v),
            Value::I32(v) => Json::from(*v),
            Value::U32(v) => Json::from(*v),
            Value::F64(v) => serde_json::Number::from_f64(*v).map_or(Json::Null, Json::Number),
            Value::Str(v) => Json::from(v.as_str()),
            _ => Json::Null,
        };
        map.insert(key.clone(), json);
    }
    Json::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_snap_to_sixteenths() {
        assert_eq!(step(0.5, true), 0.5625);
        assert_eq!(step(0.5, false), 0.4375);
        // 0,46 sta fra due tacche: si aggancia alla più vicina (0,4375) e poi si muove.
        assert_eq!(step(0.46, true), 0.5);
        assert_eq!(step(1.0, true), 1.0);
        assert_eq!(step(0.0, false), 0.0);
        // Un quarto di passo con ⌥⇧: 50 → 51,6 (il riferimento arrotonda a 52).
        assert_eq!(step_by(0.5, true, FINE_STEP), 0.515625);
    }
}
