//! Il canale `silica/dbus` contro un `dbus-daemon` privato, senza engine Flutter: le
//! chiamate entrano come le manderebbe Dart e le risposte escono codificate.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};
use silica_embedder::dbus::{Addresses, Bridge, Event};

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
            .expect("dbus-daemon (pacchetto dbus) è necessario per questo test");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        Self {
            child,
            address: line.trim().to_owned(),
        }
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Harness {
    bridge: Bridge<u32>,
    events: mpsc::Receiver<Event<u32>>,
    next: u32,
    _bus: Bus,
    address: String,
}

impl Harness {
    fn new() -> Self {
        let bus = Bus::start();
        let (sender, events) = mpsc::channel();
        let bridge = Bridge::start(
            Addresses {
                session: Some(bus.address.clone()),
                system: None,
            },
            move |event| {
                let _ = sender.send(event);
            },
        )
        .unwrap();
        Self {
            bridge,
            events,
            next: 0,
            address: bus.address.clone(),
            _bus: bus,
        }
    }

    /// Manda una chiamata e restituisce la busta della risposta (`[valore]` o
    /// `[codice, messaggio, null]`).
    fn call(&mut self, method: &str, args: Value) -> Value {
        self.next += 1;
        let token = self.next;
        let message = serde_json::to_vec(&json!({"method": method, "args": args})).unwrap();
        self.bridge.handle(&message, token);
        loop {
            match self.events.recv_timeout(Duration::from_secs(10)).unwrap() {
                Event::Reply(t, bytes) if t == token => {
                    return serde_json::from_slice(&bytes).unwrap();
                }
                _ => {}
            }
        }
    }

    fn next_signal(&self) -> Value {
        loop {
            if let Event::Signal(bytes) = self.events.recv_timeout(Duration::from_secs(10)).unwrap()
            {
                return serde_json::from_slice(&bytes).unwrap();
            }
        }
    }
}

fn bus_call(method: &str, signature: &str, args: Value) -> Value {
    json!({
        "destination": "org.freedesktop.DBus",
        "path": "/org/freedesktop/DBus",
        "interface": "org.freedesktop.DBus",
        "method": method,
        "signature": signature,
        "args": args,
    })
}

#[test]
fn calls_return_json() {
    let mut h = Harness::new();
    let reply = h.call("call", bus_call("ListNames", "", json!([])));
    let names = reply[0][0].as_array().unwrap();
    assert!(names.iter().any(|n| n == "org.freedesktop.DBus"));

    // Argomenti convertiti secondo la firma: RequestName(s, u) → u.
    let reply = h.call(
        "call",
        bus_call("RequestName", "su", json!(["dev.frascella.Prova", 4])),
    );
    assert_eq!(reply, json!([[1]]));

    // Un dizionario a{sv} diventa un oggetto.
    let reply = h.call(
        "call",
        bus_call(
            "GetConnectionCredentials",
            "s",
            json!(["org.freedesktop.DBus"]),
        ),
    );
    assert!(reply[0][0].get("UnixUserID").is_some(), "{reply}");
}

#[test]
fn errors_keep_the_dbus_name() {
    let mut h = Harness::new();
    let reply = h.call(
        "call",
        bus_call("GetNameOwner", "s", json!(["dev.frascella.Assente"])),
    );
    assert_eq!(reply[0], "org.freedesktop.DBus.Error.NameHasNoOwner");

    // Firma e argomenti che non tornano: errore, non crash.
    let reply = h.call("call", bus_call("GetNameOwner", "u", json!(["x"])));
    assert_eq!(reply[0], "org.freedesktop.DBus.Error.InvalidArgs");
    let reply = h.call("call", json!({"path": "/"}));
    assert_eq!(reply[0], "org.freedesktop.DBus.Error.InvalidArgs");
    let reply = h.call("call", json!({"bus": "altro"}));
    assert_eq!(reply[0], "org.freedesktop.DBus.Error.InvalidArgs");

    // Metodo sconosciuto del canale: risposta vuota (non implementato).
    h.bridge.handle(br#"{"method":"boh"}"#, 999);
    loop {
        if let Event::Reply(999, bytes) = h.events.recv_timeout(Duration::from_secs(10)).unwrap() {
            assert!(bytes.is_empty());
            break;
        }
    }
    // Messaggio non JSON.
    h.bridge.handle(b"\xff", 1000);
    loop {
        if let Event::Reply(1000, bytes) = h.events.recv_timeout(Duration::from_secs(10)).unwrap() {
            let reply: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(reply[0], "org.freedesktop.DBus.Error.InvalidArgs");
            break;
        }
    }
}

#[test]
fn signals_reach_dart_until_unsubscribed() {
    let mut h = Harness::new();
    let reply = h.call(
        "subscribe",
        json!({
            "sender": "org.freedesktop.DBus",
            "interface": "org.freedesktop.DBus",
            "member": "NameOwnerChanged",
            "args": null,
        }),
    );
    let id = reply[0].as_u64().unwrap();

    // Un'altra connessione che prende un nome fa emettere NameOwnerChanged.
    let other = zbus::block_on(async {
        let conn = zbus::connection::Builder::address(h.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap();
        conn.request_name("dev.frascella.Segnale").await.unwrap();
        conn
    });
    let mut seen = false;
    for _ in 0..10 {
        let signal = h.next_signal();
        assert_eq!(signal["method"], "signal");
        assert_eq!(signal["args"]["id"], id);
        assert_eq!(signal["args"]["member"], "NameOwnerChanged");
        if signal["args"]["args"][0] == "dev.frascella.Segnale" {
            seen = true;
            break;
        }
    }
    assert!(seen);

    assert_eq!(h.call("unsubscribe", json!({"id": id})), json!([true]));
    assert_eq!(h.call("unsubscribe", json!({"id": id})), json!([false]));
    drop(other);
}
