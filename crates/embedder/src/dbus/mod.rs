//! The `silica/dbus` channel: D-Bus for Dart (session and system bus).
//!
//! The shell reads the services (`silica-daemon`'s notifications, NetworkManager, UPower,
//! BlueZ) without generated bindings: Dart calls methods and subscribes to signals with JSON
//! values, and here they are converted according to the signature (`json.rs`). Codec
//! `JSONMethodCodec`.
//!
//! - `call {bus?, destination, path, interface?, method, signature?, args?}` → the list of
//!   the reply arguments; a D-Bus error becomes a `PlatformException` with the error name as
//!   the code.
//! - `subscribe {bus?, sender?, path?, interface?, member?}` → an id; then the signals reach
//!   Dart as a `signal {id, sender, path, interface, member, args}` call.
//! - `unsubscribe {id}` → `true` if the subscription existed.
//!
//! `bus` is `session` (default) or `system`. Calls run on a separate thread: a slow reply
//! does not stop the interface.

pub mod json;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::thread::JoinHandle;
use std::time::Duration;

use async_executor::{LocalExecutor, Task};
use async_lock::OnceCell;
use futures_lite::{StreamExt, future};
use serde_json::{Value as Json, json};
use tracing::warn;
use zbus::message::Type as MessageType;
use zbus::zvariant::{Signature, Structure, StructureBuilder};
use zbus::{Connection, MatchRule, Message, MessageStream};

pub const CHANNEL: &str = "silica/dbus";

/// Beyond this time a call fails with `org.freedesktop.DBus.Error.Timeout`.
const CALL_TIMEOUT: Duration = Duration::from_secs(25);
/// Maximum concurrent subscriptions per engine.
const MAX_SUBSCRIPTIONS: usize = 256;
/// Signals queued per subscription before zbus drops the oldest.
const SIGNAL_QUEUE: usize = 64;

/// What the D-Bus thread returns to the platform thread.
pub enum Event<T> {
    /// The (already encoded) reply to a call from Dart.
    Reply(T, Vec<u8>),
    /// A `signal` message to send to Dart on the channel.
    Signal(Vec<u8>),
}

/// Bus addresses; `None` uses the environment's.
#[derive(Clone, Debug, Default)]
pub struct Addresses {
    pub session: Option<String>,
    pub system: Option<String>,
}

/// The bridge: calls from Dart enter with `handle`, outcomes leave through `sink`.
pub struct Bridge<T> {
    requests: async_channel::Sender<(T, Vec<u8>)>,
    _thread: JoinHandle<()>,
}

impl<T: Send + 'static> Bridge<T> {
    pub fn start(
        addresses: Addresses,
        sink: impl Fn(Event<T>) + Send + 'static,
    ) -> Result<Self, String> {
        let (requests, receiver) = async_channel::unbounded();
        let thread = std::thread::Builder::new()
            .name("silica-dbus".to_owned())
            .spawn(move || {
                let executor = Rc::new(LocalExecutor::new());
                let worker = Rc::new(Worker {
                    addresses,
                    session: OnceCell::new(),
                    system: OnceCell::new(),
                    sink: Box::new(sink),
                    subscriptions: RefCell::new(HashMap::new()),
                    next_id: Cell::new(0),
                });
                future::block_on(executor.run(worker.serve(receiver, executor.clone())));
            })
            .map_err(|e| format!("thread D-Bus: {e}"))?;
        Ok(Self {
            requests,
            _thread: thread,
        })
    }

    /// A call from Dart; the reply arrives later as `Event::Reply(token, …)`.
    pub fn handle(&self, data: &[u8], token: T) {
        if self.requests.try_send((token, data.to_vec())).is_err() {
            warn!("silica/dbus: the D-Bus thread does not answer");
        }
    }
}

#[derive(Clone, Copy)]
enum Bus {
    Session,
    System,
}

struct Worker<T> {
    addresses: Addresses,
    session: OnceCell<Connection>,
    system: OnceCell<Connection>,
    sink: Box<dyn Fn(Event<T>)>,
    subscriptions: RefCell<HashMap<u64, Task<()>>>,
    next_id: Cell<u64>,
}

type CallResult = Result<Json, (String, String)>;

impl<T: 'static> Worker<T> {
    async fn serve(
        self: Rc<Self>,
        requests: async_channel::Receiver<(T, Vec<u8>)>,
        executor: Rc<LocalExecutor<'static>>,
    ) {
        while let Ok((token, data)) = requests.recv().await {
            let Ok(call) = serde_json::from_slice::<Json>(&data) else {
                self.reply(token, Err(invalid("non-JSON message")));
                continue;
            };
            let args = call.get("args").cloned().unwrap_or(Json::Null);
            match call.get("method").and_then(Json::as_str) {
                Some("call") => {
                    let worker = self.clone();
                    executor
                        .spawn(async move {
                            let result = worker.call(&args).await;
                            worker.reply(token, result);
                        })
                        .detach();
                }
                Some("subscribe") => {
                    if self.subscriptions.borrow().len() >= MAX_SUBSCRIPTIONS {
                        self.reply(token, Err(failed("too many subscriptions")));
                        continue;
                    }
                    let id = self.next_id.get().wrapping_add(1);
                    self.next_id.set(id);
                    let worker = self.clone();
                    let task = executor.spawn(async move {
                        worker.subscribe(id, &args, token).await;
                    });
                    self.subscriptions.borrow_mut().insert(id, task);
                }
                Some("unsubscribe") => {
                    let id = args.get("id").and_then(Json::as_u64).unwrap_or(0);
                    // Dropping the task stops it and removes the rule from the bus.
                    let removed = self.subscriptions.borrow_mut().remove(&id).is_some();
                    self.reply(token, Ok(json!(removed)));
                }
                // Unknown method: empty reply, which Dart reads as not implemented.
                _ => (self.sink)(Event::Reply(token, Vec::new())),
            }
        }
    }

    fn reply(&self, token: T, result: CallResult) {
        let envelope = match result {
            Ok(value) => json!([value]),
            Err((code, message)) => json!([code, message, null]),
        };
        (self.sink)(Event::Reply(
            token,
            serde_json::to_vec(&envelope).unwrap_or_default(),
        ));
    }

    async fn connection(&self, args: &Json) -> Result<&Connection, (String, String)> {
        let bus = match args.get("bus").and_then(Json::as_str) {
            None | Some("session") => Bus::Session,
            Some("system") => Bus::System,
            Some(other) => return Err(invalid(&format!("unknown bus: {other}"))),
        };
        let (cell, address) = match bus {
            Bus::Session => (&self.session, &self.addresses.session),
            Bus::System => (&self.system, &self.addresses.system),
        };
        cell.get_or_try_init(|| async {
            let builder = match (address, bus) {
                (Some(address), _) => zbus::connection::Builder::address(address.as_str()),
                (None, Bus::Session) => zbus::connection::Builder::session(),
                (None, Bus::System) => zbus::connection::Builder::system(),
            };
            builder?.build().await
        })
        .await
        .map_err(|e| dbus_error(&e))
    }

    async fn call(&self, args: &Json) -> CallResult {
        let text = |name: &str| args.get(name).and_then(Json::as_str);
        let required = |name: &str| text(name).ok_or_else(|| invalid(&format!("missing «{name}»")));
        let destination = required("destination")?;
        let path = required("path")?;
        let method = required("method")?;
        let interface = text("interface");
        let signature = text("signature").unwrap_or("");
        let values = match args.get("args") {
            None | Some(Json::Null) => Vec::new(),
            Some(Json::Array(items)) => json::to_args(signature, items).map_err(|e| invalid(&e))?,
            Some(_) => return Err(invalid("«args» must be a list")),
        };
        let connection = self.connection(args).await?;
        let call = async {
            let reply = if values.is_empty() {
                connection
                    .call_method(Some(destination), path, interface, method, &())
                    .await
            } else {
                let mut body = StructureBuilder::new();
                for value in values {
                    body = body.append_field(value);
                }
                let body = body.build().map_err(|e| invalid(&e.to_string()))?;
                connection
                    .call_method(Some(destination), path, interface, method, &body)
                    .await
            };
            reply.map_err(|e| dbus_error(&e))
        };
        let timeout = async {
            async_io::Timer::after(CALL_TIMEOUT).await;
            Err((
                "org.freedesktop.DBus.Error.Timeout".to_owned(),
                format!("{method}: no reply"),
            ))
        };
        let reply = future::or(call, timeout).await?;
        Ok(Json::Array(reply_args(&reply)?))
    }

    async fn subscribe(&self, id: u64, args: &Json, token: T) {
        let stream = match self.signal_stream(args).await {
            Ok(stream) => stream,
            Err(err) => {
                self.reply(token, Err(err));
                self.subscriptions.borrow_mut().remove(&id);
                return;
            }
        };
        self.reply(token, Ok(json!(id)));
        let mut stream = stream;
        while let Some(message) = stream.next().await {
            let Ok(message) = message else { continue };
            let header = message.header();
            let args = reply_args(&message).unwrap_or_default();
            let signal = json!({
                "method": "signal",
                "args": {
                    "id": id,
                    "sender": header.sender().map(|s| s.to_string()),
                    "path": header.path().map(|p| p.to_string()),
                    "interface": header.interface().map(|i| i.to_string()),
                    "member": header.member().map(|m| m.to_string()),
                    "args": args,
                }
            });
            if let Ok(bytes) = serde_json::to_vec(&signal) {
                (self.sink)(Event::Signal(bytes));
            }
        }
    }

    async fn signal_stream(&self, args: &Json) -> Result<MessageStream, (String, String)> {
        let text = |name: &str| args.get(name).and_then(Json::as_str);
        let bad = |e: zbus::Error| invalid(&e.to_string());
        let mut rule = MatchRule::builder().msg_type(MessageType::Signal);
        if let Some(sender) = text("sender") {
            rule = rule.sender(sender).map_err(bad)?;
        }
        if let Some(path) = text("path") {
            rule = rule.path(path).map_err(bad)?;
        }
        if let Some(interface) = text("interface") {
            rule = rule.interface(interface).map_err(bad)?;
        }
        if let Some(member) = text("member") {
            rule = rule.member(member).map_err(bad)?;
        }
        let rule = rule.build().to_owned();
        let connection = self.connection(args).await?;
        MessageStream::for_match_rule(rule, connection, Some(SIGNAL_QUEUE))
            .await
            .map_err(|e| dbus_error(&e))
    }
}

/// The arguments of a message, as JSON.
fn reply_args(message: &Message) -> Result<Vec<Json>, (String, String)> {
    let body = message.body();
    if matches!(body.signature(), Signature::Unit) {
        return Ok(Vec::new());
    }
    // The arguments of a message are read as the fields of a structure. The signature is needed
    // as written in the header: once parsed, `us` (two arguments) and `(us)` (one structure)
    // become the same `Signature`.
    let raw = header_signature(message.data().bytes())
        .ok_or_else(|| failed("unreadable message signature"))?;
    let fields = format!("({raw})");
    let (structure, _): (Structure<'_>, usize) = body
        .data()
        .deserialize_for_dynamic_signature(fields.as_str())
        .map_err(|e| failed(&format!("unreadable reply: {e}")))?;
    Ok(structure.fields().iter().map(json::to_json).collect())
}

/// The body signature as written in the message header (field 8). Format: D-Bus
/// specification, "Message Format"; the bytes come from the bus and are read with checks.
fn header_signature(bytes: &[u8]) -> Option<&str> {
    let little = match bytes.first()? {
        b'l' => true,
        b'B' => false,
        _ => return None,
    };
    let u32_at = |at: usize| -> Option<usize> {
        let raw: [u8; 4] = bytes.get(at..at.checked_add(4)?)?.try_into().ok()?;
        let value = if little {
            u32::from_le_bytes(raw)
        } else {
            u32::from_be_bytes(raw)
        };
        usize::try_from(value).ok()
    };
    let align = |at: usize, to: usize| at.checked_add(to - 1).map(|v| v / to * to);
    let end = u32_at(12)?.checked_add(16)?;
    let mut at = 16;
    loop {
        at = align(at, 8)?;
        if at >= end {
            return Some("");
        }
        let code = *bytes.get(at)?;
        let sig_len = usize::from(*bytes.get(at.checked_add(1)?)?);
        let sig_start = at.checked_add(2)?;
        let sig = bytes.get(sig_start..sig_start.checked_add(sig_len)?)?;
        at = sig_start.checked_add(sig_len)?.checked_add(1)?;
        match sig {
            b"g" => {
                let len = usize::from(*bytes.get(at)?);
                let start = at.checked_add(1)?;
                let value = bytes.get(start..start.checked_add(len)?)?;
                if code == 8 {
                    return std::str::from_utf8(value).ok();
                }
                at = start.checked_add(len)?.checked_add(1)?;
            }
            b"s" | b"o" => {
                at = align(at, 4)?;
                let len = u32_at(at)?;
                at = at.checked_add(4)?.checked_add(len)?.checked_add(1)?;
            }
            b"u" => at = align(at, 4)?.checked_add(4)?,
            _ => return None,
        }
    }
}

fn dbus_error(error: &zbus::Error) -> (String, String) {
    match error {
        zbus::Error::MethodError(name, description, _) => {
            (name.to_string(), description.clone().unwrap_or_default())
        }
        other => failed(&other.to_string()),
    }
}

fn invalid(message: &str) -> (String, String) {
    (
        "org.freedesktop.DBus.Error.InvalidArgs".to_owned(),
        message.to_owned(),
    )
}

fn failed(message: &str) -> (String, String) {
    (
        "org.freedesktop.DBus.Error.Failed".to_owned(),
        message.to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(
        body: &(impl zbus::export::serde::Serialize + zbus::zvariant::DynamicType),
    ) -> Message {
        Message::signal("/a/b", "dev.frascella.Test", "Changed")
            .unwrap()
            .build(body)
            .unwrap()
    }

    #[test]
    fn one_struct_is_not_many_arguments() {
        let many = signal(&(7u32, "x"));
        assert_eq!(header_signature(many.data().bytes()), Some("us"));
        assert_eq!(reply_args(&many).unwrap(), [json!(7), json!("x")]);

        let one = signal(&((7u32, "x"),));
        assert_eq!(header_signature(one.data().bytes()), Some("(us)"));
        assert_eq!(reply_args(&one).unwrap(), [json!([7, "x"])]);

        let none = signal(&());
        assert!(reply_args(&none).unwrap().is_empty());
    }

    #[test]
    fn broken_headers_are_rejected() {
        let message = signal(&(7u32, "x"));
        let bytes = message.data().bytes();
        for cut in 0..bytes.len().min(64) {
            // Never a panic, whatever the truncation.
            let _ = header_signature(&bytes[..cut]);
        }
        assert_eq!(header_signature(b"x"), None);
    }
}
