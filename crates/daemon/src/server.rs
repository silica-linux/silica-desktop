//! Le interfacce D-Bus delle notifiche.
//!
//! - `org.freedesktop.Notifications` su `/org/freedesktop/Notifications`: la specifica
//!   freedesktop 1.2, per le app.
//! - `dev.frascella.Silica.Notifications1` su `/dev/frascella/Silica/Notifications1`: per la
//!   shell (banner e Centro notifiche).
//!
//! Le due interfacce condividono lo stesso `Store`; ogni cambiamento emette i segnali di
//! entrambe.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use event_listener::Event;
use futures_lite::future;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, interface};

use crate::notifications::{CloseReason, Notification, Request, Store, Urgency, action_keys};

pub const FDO_NAME: &str = "org.freedesktop.Notifications";
pub const FDO_PATH: &str = "/org/freedesktop/Notifications";
pub const SHELL_NAME: &str = "dev.frascella.Silica";
pub const SHELL_PATH: &str = "/dev/frascella/Silica/Notifications1";
pub const SHELL_INTERFACE: &str = "dev.frascella.Silica.Notifications1";

/// Versione della specifica freedesktop implementata.
pub const SPEC_VERSION: &str = "1.2";

/// Capacità annunciate da `GetCapabilities`. Niente markup nel corpo: la shell lo mostra
/// come testo semplice.
pub const CAPABILITIES: &[&str] = &["actions", "body", "icon-static", "persistence"];

#[derive(Clone, Default)]
struct Shared {
    store: Arc<Mutex<Store>>,
    /// Sveglia il ciclo delle scadenze quando cambia la prossima scadenza.
    changed: Arc<Event>,
}

impl Shared {
    /// Lo stato resta coerente anche se un altro thread è caduto tenendo il lucchetto: ogni
    /// operazione dello `Store` lo lascia valido.
    fn store(&self) -> MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Il daemon delle notifiche su una connessione al bus.
pub struct Daemon {
    connection: Connection,
    shared: Shared,
}

impl Daemon {
    /// Pubblica le interfacce su `connection` e prende i nomi `org.freedesktop.Notifications`
    /// e `dev.frascella.Silica`. Con `replace` sostituisce un proprietario che lo consente;
    /// altrimenti fallisce se un nome è già preso.
    pub async fn serve(connection: Connection, replace: bool) -> zbus::Result<Self> {
        let shared = Shared::default();
        let server = connection.object_server();
        server
            .at(
                FDO_PATH,
                Notifications {
                    shared: shared.clone(),
                },
            )
            .await?;
        server
            .at(
                SHELL_PATH,
                ShellNotifications {
                    shared: shared.clone(),
                },
            )
            .await?;
        let mut flags = RequestNameFlags::DoNotQueue | RequestNameFlags::AllowReplacement;
        if replace {
            flags |= RequestNameFlags::ReplaceExisting;
        }
        for name in [FDO_NAME, SHELL_NAME] {
            match connection.request_name_with_flags(name, flags).await? {
                RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
                RequestNameReply::InQueue | RequestNameReply::Exists => {
                    return Err(zbus::Error::NameTaken);
                }
            }
        }
        Ok(Self { connection, shared })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Fa scadere le notifiche con un tempo; non ritorna se non per un errore del bus.
    pub async fn run(&self) -> zbus::Result<()> {
        loop {
            // Ascoltare prima di leggere la scadenza: un cambiamento nel mezzo non si perde.
            let listener = self.shared.changed.listen();
            let next = self.shared.store().next_deadline();
            match next {
                Some(deadline) => {
                    future::or(
                        async {
                            async_io::Timer::at(deadline).await;
                        },
                        listener,
                    )
                    .await;
                }
                None => listener.await,
            }
            let expired = self.shared.store().take_expired(Instant::now());
            for id in expired {
                emit_closed(&self.connection, id, CloseReason::Expired).await?;
            }
        }
    }
}

/// `org.freedesktop.Notifications`.
struct Notifications {
    shared: Shared,
}

#[interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        #[zbus(connection)] connection: &Connection,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        let request = Request {
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            urgency: hint_int(&hints, "urgency")
                .map(Urgency::from_level)
                .unwrap_or_default(),
            category: hint_str(&hints, "category"),
            desktop_entry: hint_str(&hints, "desktop-entry"),
            image_path: hint_str(&hints, "image-path"),
            transient: hint_bool(&hints, "transient"),
            resident: hint_bool(&hints, "resident"),
            expire_timeout,
        };
        let posted = self
            .shared
            .store()
            .notify(request, Instant::now(), unix_ms());
        self.shared.changed.notify(usize::MAX);
        for id in posted.evicted {
            emit_closed(connection, id, CloseReason::Undefined).await?;
        }
        let id = posted.notification.id;
        let emitter = SignalEmitter::new(connection, SHELL_PATH)?;
        ShellNotifications::added(&emitter, posted.notification).await?;
        Ok(id)
    }

    /// Chiude una notifica; se non esiste più non succede niente.
    async fn close_notification(
        &self,
        #[zbus(connection)] connection: &Connection,
        id: u32,
    ) -> zbus::fdo::Result<()> {
        let removed = self.shared.store().remove(id);
        if removed.is_some() {
            emit_closed(connection, id, CloseReason::Closed).await?;
        }
        Ok(())
    }

    fn get_capabilities(&self) -> Vec<String> {
        CAPABILITIES.iter().map(|c| (*c).to_owned()).collect()
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Silica".to_owned(),
            "Silica".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            SPEC_VERSION.to_owned(),
        )
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

/// `dev.frascella.Silica.Notifications1`: la vista della shell.
struct ShellNotifications {
    shared: Shared,
}

#[interface(name = "dev.frascella.Silica.Notifications1")]
impl ShellNotifications {
    /// Lo storico, dalla più vecchia alla più recente.
    fn list(&self) -> Vec<Notification> {
        self.shared.store().list()
    }

    /// L'utente ha chiuso la notifica.
    async fn dismiss(
        &self,
        #[zbus(connection)] connection: &Connection,
        id: u32,
    ) -> zbus::fdo::Result<()> {
        let removed = self.shared.store().remove(id);
        if removed.is_some() {
            emit_closed(connection, id, CloseReason::Dismissed).await?;
        }
        Ok(())
    }

    /// L'utente ha chiuso tutte le notifiche.
    async fn clear(&self, #[zbus(connection)] connection: &Connection) -> zbus::fdo::Result<()> {
        let ids = self.shared.store().clear();
        for id in ids {
            emit_closed(connection, id, CloseReason::Dismissed).await?;
        }
        Ok(())
    }

    /// L'utente ha scelto un'azione (`default` è il clic sulla notifica). Senza l'hint
    /// `resident` la notifica poi si chiude.
    async fn invoke_action(
        &self,
        #[zbus(connection)] connection: &Connection,
        id: u32,
        action_key: String,
    ) -> zbus::fdo::Result<()> {
        let resident = {
            let store = self.shared.store();
            let notification = store
                .get(id)
                .ok_or_else(|| zbus::fdo::Error::InvalidArgs(format!("notifica {id} assente")))?;
            if !action_keys(notification).any(|key| key == action_key) {
                return Err(zbus::fdo::Error::InvalidArgs(format!(
                    "azione «{action_key}» assente nella notifica {id}"
                )));
            }
            notification.resident
        };
        let emitter = SignalEmitter::new(connection, FDO_PATH)?;
        Notifications::action_invoked(&emitter, id, &action_key).await?;
        if !resident && self.shared.store().remove(id).is_some() {
            emit_closed(connection, id, CloseReason::Dismissed).await?;
        }
        Ok(())
    }

    /// Una notifica nuova o sostituita (stesso id).
    #[zbus(signal)]
    async fn added(emitter: &SignalEmitter<'_>, notification: Notification) -> zbus::Result<()>;

    /// Una notifica tolta, con il motivo della specifica freedesktop.
    #[zbus(signal)]
    async fn removed(emitter: &SignalEmitter<'_>, id: u32, reason: u32) -> zbus::Result<()>;
}

/// `NotificationClosed` per le app e `Removed` per la shell.
async fn emit_closed(connection: &Connection, id: u32, reason: CloseReason) -> zbus::Result<()> {
    let reason = reason as u32;
    let fdo = SignalEmitter::new(connection, FDO_PATH)?;
    Notifications::notification_closed(&fdo, id, reason).await?;
    let shell = SignalEmitter::new(connection, SHELL_PATH)?;
    ShellNotifications::removed(&shell, id, reason).await
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Gli hint arrivano con tipi diversi a seconda della libreria del client: `urgency` è un
/// byte per la specifica, ma c'è chi manda un intero.
fn hint_int(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<i64> {
    match &**hints.get(key)? {
        Value::U8(v) => Some(i64::from(*v)),
        Value::I16(v) => Some(i64::from(*v)),
        Value::U16(v) => Some(i64::from(*v)),
        Value::I32(v) => Some(i64::from(*v)),
        Value::U32(v) => Some(i64::from(*v)),
        Value::I64(v) => Some(*v),
        Value::U64(v) => i64::try_from(*v).ok(),
        _ => None,
    }
}

fn hint_bool(hints: &HashMap<String, OwnedValue>, key: &str) -> bool {
    match hints.get(key).map(|v| &**v) {
        Some(Value::Bool(v)) => *v,
        _ => hint_int(hints, key).is_some_and(|v| v != 0),
    }
}

fn hint_str(hints: &HashMap<String, OwnedValue>, key: &str) -> String {
    match hints.get(key).map(|v| &**v) {
        Some(Value::Str(s)) => s.as_str().to_owned(),
        Some(Value::ObjectPath(p)) => p.as_str().to_owned(),
        _ => String::new(),
    }
}
