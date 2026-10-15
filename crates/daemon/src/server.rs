//! The D-Bus interfaces of notifications.
//!
//! - `org.freedesktop.Notifications` on `/org/freedesktop/Notifications`: the freedesktop
//!   specification 1.2, for apps.
//! - `dev.frascella.Silica.Notifications1` on `/dev/frascella/Silica/Notifications1`: for the
//!   shell (banners and Notification Center).
//!
//! The two interfaces share the same `Store`; every change emits the signals of
//! both.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use event_listener::Event;
use futures_lite::{StreamExt, future};
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

/// Version of the freedesktop specification implemented.
pub const SPEC_VERSION: &str = "1.2";

/// Capabilities announced by `GetCapabilities`. No markup in the body: the shell shows it
/// as plain text. `inline-reply` is the KDE extension apps already use: a notification
/// with the `inline-reply` action gets a reply field, and the text comes back with the
/// `NotificationReplied` signal.
pub const CAPABILITIES: &[&str] = &[
    "actions",
    "body",
    "icon-static",
    "persistence",
    "inline-reply",
];

/// The action key that asks for a reply field (KDE's inline-reply extension).
pub const REPLY_ACTION: &str = "inline-reply";

#[derive(Clone, Default)]
struct Shared {
    store: Arc<Mutex<Store>>,
    /// Wakes the expiration loop when the next expiration changes.
    changed: Arc<Event>,
}

impl Shared {
    /// The state stays consistent even if another thread panicked holding the lock: every
    /// `Store` operation leaves it valid.
    fn store(&self) -> MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The notification daemon on a bus connection.
pub struct Daemon {
    connection: Connection,
    shared: Shared,
    /// `NameLost` signals, subscribed before the names are requested so that a loss right
    /// after the request (a racing instance with `--replace`) is not missed.
    lost: Mutex<Option<zbus::fdo::NameLostStream>>,
}

impl Daemon {
    /// Publishes the interfaces on `connection` and takes the names `org.freedesktop.Notifications`
    /// and `dev.frascella.Silica`. With `replace` it replaces an owner that allows it;
    /// otherwise it fails if a name is already taken.
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
        let lost = zbus::fdo::DBusProxy::new(&connection)
            .await?
            .receive_name_lost()
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
        Ok(Self {
            connection,
            shared,
            lost: Mutex::new(Some(lost)),
        })
    }

    /// Returns the name this daemon lost (`org.freedesktop.Notifications` or
    /// `dev.frascella.Silica`, taken by another instance with `--replace`): a daemon that
    /// does not own its names has nothing to serve and must exit. Fails if called twice or
    /// if the bus closes.
    pub async fn name_lost(&self) -> zbus::Result<String> {
        let taken = self
            .lost
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some(mut lost) = taken else {
            return Err(zbus::Error::Failure("name_lost already awaited".to_owned()));
        };
        while let Some(signal) = lost.next().await {
            if let Ok(args) = signal.args() {
                let name = args.name().as_str();
                if name == FDO_NAME || name == SHELL_NAME {
                    return Ok(name.to_owned());
                }
            }
        }
        Err(zbus::Error::Failure("session bus closed".to_owned()))
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Expires the notifications that have a time; returns only on a bus error.
    pub async fn run(&self) -> zbus::Result<()> {
        loop {
            // Listen before reading the expiration: a change in between is not lost.
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

    /// Closes a notification; if it no longer exists nothing happens.
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

    /// The user replied from the notification (inline-reply extension).
    #[zbus(signal)]
    async fn notification_replied(
        emitter: &SignalEmitter<'_>,
        id: u32,
        text: &str,
    ) -> zbus::Result<()>;
}

/// `dev.frascella.Silica.Notifications1`: the shell's view.
struct ShellNotifications {
    shared: Shared,
}

#[interface(name = "dev.frascella.Silica.Notifications1")]
impl ShellNotifications {
    /// The history, from the oldest to the most recent.
    fn list(&self) -> Vec<Notification> {
        self.shared.store().list()
    }

    /// The user closed the notification.
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

    /// The user closed every notification.
    async fn clear(&self, #[zbus(connection)] connection: &Connection) -> zbus::fdo::Result<()> {
        let ids = self.shared.store().clear();
        for id in ids {
            emit_closed(connection, id, CloseReason::Dismissed).await?;
        }
        Ok(())
    }

    /// The user chose an action (`default` is the click on the notification). Without the
    /// `resident` hint the notification then closes.
    async fn invoke_action(
        &self,
        #[zbus(connection)] connection: &Connection,
        id: u32,
        action_key: String,
    ) -> zbus::fdo::Result<()> {
        let resident = {
            let store = self.shared.store();
            let notification = store.get(id).ok_or_else(|| {
                zbus::fdo::Error::InvalidArgs(format!("notification {id} absent"))
            })?;
            if !action_keys(notification).any(|key| key == action_key) {
                return Err(zbus::fdo::Error::InvalidArgs(format!(
                    "action «{action_key}» absent in notification {id}"
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

    /// The user replied in the notification's field. Only for notifications that offer the
    /// `inline-reply` action; without the `resident` hint the notification then closes.
    async fn reply(
        &self,
        #[zbus(connection)] connection: &Connection,
        id: u32,
        text: String,
    ) -> zbus::fdo::Result<()> {
        let resident = {
            let store = self.shared.store();
            let notification = store.get(id).ok_or_else(|| {
                zbus::fdo::Error::InvalidArgs(format!("notification {id} absent"))
            })?;
            if !action_keys(notification).any(|key| key == REPLY_ACTION) {
                return Err(zbus::fdo::Error::InvalidArgs(format!(
                    "notification {id} does not take a reply"
                )));
            }
            notification.resident
        };
        let emitter = SignalEmitter::new(connection, FDO_PATH)?;
        Notifications::notification_replied(&emitter, id, &text).await?;
        if !resident && self.shared.store().remove(id).is_some() {
            emit_closed(connection, id, CloseReason::Dismissed).await?;
        }
        Ok(())
    }

    /// A new or replaced notification (same id).
    #[zbus(signal)]
    async fn added(emitter: &SignalEmitter<'_>, notification: Notification) -> zbus::Result<()>;

    /// A removed notification, with the freedesktop specification reason.
    #[zbus(signal)]
    async fn removed(emitter: &SignalEmitter<'_>, id: u32, reason: u32) -> zbus::Result<()>;
}

/// `NotificationClosed` for apps and `Removed` for the shell.
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

/// Hints arrive with different types depending on the client library: `urgency` is a
/// byte per the specification, but some send an integer.
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
