//! Le notifiche ricevute: storico in memoria, sostituzioni, scadenze.
//!
//! Logica pura, senza D-Bus: il server (`server.rs`) la chiama e trasforma i risultati in
//! segnali. Tutto quello che arriva dai client è ostile: testi troncati, azioni limitate,
//! storico di dimensione fissa.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// Notifiche tenute nello storico: oltre si scarta la più vecchia.
pub const MAX_HISTORY: usize = 256;
/// Caratteri massimi del nome dell'app, dell'icona, della categoria.
pub const MAX_NAME: usize = 256;
/// Caratteri massimi del titolo.
pub const MAX_SUMMARY: usize = 1024;
/// Caratteri massimi del corpo.
pub const MAX_BODY: usize = 8192;
/// Voci massime della lista delle azioni (coppie chiave, etichetta).
pub const MAX_ACTIONS: usize = 32;

/// Motivo della chiusura, come nella specifica freedesktop (segnale `NotificationClosed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum CloseReason {
    Expired = 1,
    Dismissed = 2,
    Closed = 3,
    Undefined = 4,
}

/// Urgenza (hint `urgency`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Urgency {
    Low = 0,
    #[default]
    Normal = 1,
    Critical = 2,
}

impl Urgency {
    pub fn from_level(level: i64) -> Self {
        match level {
            i64::MIN..=0 => Self::Low,
            1 => Self::Normal,
            _ => Self::Critical,
        }
    }
}

/// Una notifica come la vede la shell (`dev.frascella.Silica.Notifications1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    /// Coppie chiave, etichetta, appiattite come nella specifica.
    pub actions: Vec<String>,
    /// 0 bassa, 1 normale, 2 critica.
    pub urgency: u8,
    pub category: String,
    pub desktop_entry: String,
    pub image_path: String,
    /// Non va nello storico del Centro notifiche: solo il banner.
    pub transient: bool,
    /// Resta dopo l'invocazione di un'azione.
    pub resident: bool,
    /// Millisecondi dall'epoca Unix alla ricezione.
    pub timestamp_ms: u64,
    /// Come richiesto dal client: -1 predefinito, 0 mai, altrimenti millisecondi.
    pub expire_timeout: i32,
}

/// Una richiesta `Notify`, con gli hint già letti.
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub app_name: String,
    pub replaces_id: u32,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<String>,
    pub urgency: Urgency,
    pub category: String,
    pub desktop_entry: String,
    pub image_path: String,
    pub transient: bool,
    pub resident: bool,
    pub expire_timeout: i32,
}

/// Esito di `Store::notify`.
#[derive(Debug)]
pub struct Posted {
    pub notification: Notification,
    /// Notifiche tolte per far posto nello storico.
    pub evicted: Vec<u32>,
}

#[derive(Debug)]
struct Entry {
    notification: Notification,
    deadline: Option<Instant>,
}

#[derive(Debug)]
pub struct Store {
    entries: Vec<Entry>,
    last_id: u32,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_id: 0,
        }
    }

    /// Aggiunge una notifica, o sostituisce quella con `replaces_id` se esiste ancora.
    pub fn notify(&mut self, request: Request, now: Instant, timestamp_ms: u64) -> Posted {
        let deadline = deadline(request.expire_timeout, now);
        let replaced = (request.replaces_id != 0)
            .then(|| {
                self.entries
                    .iter()
                    .position(|e| e.notification.id == request.replaces_id)
            })
            .flatten();
        let id = match replaced {
            Some(_) => request.replaces_id,
            None => self.next_id(),
        };
        let notification = sanitize(id, request, timestamp_ms);
        let entry = Entry {
            notification: notification.clone(),
            deadline,
        };
        let mut evicted = Vec::new();
        match replaced.and_then(|index| self.entries.get_mut(index)) {
            Some(slot) => *slot = entry,
            None => {
                while self.entries.len() >= MAX_HISTORY {
                    let old = self.entries.remove(0);
                    evicted.push(old.notification.id);
                }
                self.entries.push(entry);
            }
        }
        Posted {
            notification,
            evicted,
        }
    }

    /// Toglie una notifica; `None` se non c'era.
    pub fn remove(&mut self, id: u32) -> Option<Notification> {
        let index = self.entries.iter().position(|e| e.notification.id == id)?;
        Some(self.entries.remove(index).notification)
    }

    /// Toglie tutte le notifiche e ne restituisce gli id.
    pub fn clear(&mut self) -> Vec<u32> {
        self.entries.drain(..).map(|e| e.notification.id).collect()
    }

    pub fn get(&self, id: u32) -> Option<&Notification> {
        self.entries
            .iter()
            .map(|e| &e.notification)
            .find(|n| n.id == id)
    }

    /// Dalla più vecchia alla più recente.
    pub fn list(&self) -> Vec<Notification> {
        self.entries
            .iter()
            .map(|e| e.notification.clone())
            .collect()
    }

    /// La prossima scadenza, se ce n'è una.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.entries.iter().filter_map(|e| e.deadline).min()
    }

    /// Toglie le notifiche scadute entro `now` e ne restituisce gli id.
    pub fn take_expired(&mut self, now: Instant) -> Vec<u32> {
        let mut expired = Vec::new();
        self.entries.retain(|e| match e.deadline {
            Some(deadline) if deadline <= now => {
                expired.push(e.notification.id);
                false
            }
            _ => true,
        });
        expired
    }

    /// Id successivo, mai 0 e mai uno ancora in uso.
    fn next_id(&mut self) -> u32 {
        loop {
            self.last_id = self.last_id.wrapping_add(1);
            if self.last_id != 0 && self.get(self.last_id).is_none() {
                return self.last_id;
            }
        }
    }
}

/// Scadenza della notifica. Come nel Centro notifiche del riferimento, le notifiche restano
/// finché l'utente non le chiude (-1 e 0); scadono solo se il client chiede un tempo.
fn deadline(expire_timeout: i32, now: Instant) -> Option<Instant> {
    let ms = u64::try_from(expire_timeout).ok().filter(|&ms| ms > 0)?;
    now.checked_add(Duration::from_millis(ms))
}

fn sanitize(id: u32, request: Request, timestamp_ms: u64) -> Notification {
    let mut actions: Vec<String> = request
        .actions
        .into_iter()
        .take(MAX_ACTIONS)
        .map(|a| truncate(a, MAX_NAME))
        .collect();
    // Le azioni sono coppie: una chiave senza etichetta si scarta.
    if actions.len() % 2 == 1 {
        actions.pop();
    }
    Notification {
        id,
        app_name: truncate(request.app_name, MAX_NAME),
        app_icon: truncate(request.app_icon, MAX_NAME),
        summary: truncate(request.summary, MAX_SUMMARY),
        body: truncate(request.body, MAX_BODY),
        actions,
        urgency: request.urgency as u8,
        category: truncate(request.category, MAX_NAME),
        desktop_entry: truncate(request.desktop_entry, MAX_NAME),
        image_path: truncate(request.image_path, MAX_NAME),
        transient: request.transient,
        resident: request.resident,
        timestamp_ms,
        expire_timeout: request.expire_timeout,
    }
}

fn truncate(mut text: String, max_chars: usize) -> String {
    if let Some((index, _)) = text.char_indices().nth(max_chars) {
        text.truncate(index);
    }
    text
}

/// Le chiavi delle azioni (le voci pari della lista).
pub fn action_keys(notification: &Notification) -> impl Iterator<Item = &str> {
    notification.actions.iter().step_by(2).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(summary: &str) -> Request {
        Request {
            app_name: "prova".into(),
            summary: summary.into(),
            expire_timeout: -1,
            ..Request::default()
        }
    }

    #[test]
    fn ids_grow_and_skip_zero() {
        let mut store = Store::new();
        store.last_id = u32::MAX;
        let now = Instant::now();
        let a = store.notify(request("a"), now, 0).notification.id;
        assert_eq!(a, 1);
        assert_eq!(store.notify(request("b"), now, 0).notification.id, 2);
    }

    #[test]
    fn replaces_keep_the_id() {
        let mut store = Store::new();
        let now = Instant::now();
        let id = store.notify(request("a"), now, 0).notification.id;
        let mut again = request("b");
        again.replaces_id = id;
        let posted = store.notify(again, now, 0);
        assert_eq!(posted.notification.id, id);
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.get(id).map(|n| n.summary.as_str()), Some("b"));
        // Un id che non esiste più dà una notifica nuova.
        let mut stale = request("c");
        stale.replaces_id = 999;
        assert_ne!(store.notify(stale, now, 0).notification.id, 999);
    }

    #[test]
    fn hostile_input_is_bounded() {
        let mut store = Store::new();
        let mut req = request(&"é".repeat(MAX_SUMMARY * 4));
        req.body = "x".repeat(MAX_BODY * 4);
        req.actions = (0..101).map(|i| i.to_string()).collect();
        let n = store.notify(req, Instant::now(), 0).notification;
        assert_eq!(n.summary.chars().count(), MAX_SUMMARY);
        assert_eq!(n.body.len(), MAX_BODY);
        assert_eq!(n.actions.len(), MAX_ACTIONS);

        let mut odd = request("odd");
        odd.actions = vec!["default".into(), "Apri".into(), "sola".into()];
        let n = store.notify(odd, Instant::now(), 0).notification;
        assert_eq!(action_keys(&n).collect::<Vec<_>>(), ["default"]);
    }

    #[test]
    fn history_drops_the_oldest() {
        let mut store = Store::new();
        let now = Instant::now();
        for i in 0..MAX_HISTORY {
            store.notify(request(&i.to_string()), now, 0);
        }
        let posted = store.notify(request("ultima"), now, 0);
        assert_eq!(posted.evicted, [1]);
        assert_eq!(store.list().len(), MAX_HISTORY);
    }

    #[test]
    fn only_explicit_timeouts_expire() {
        let mut store = Store::new();
        let now = Instant::now();
        let mut timed = request("a tempo");
        timed.expire_timeout = 500;
        let timed = store.notify(timed, now, 0).notification.id;
        let mut never = request("mai");
        never.expire_timeout = 0;
        store.notify(never, now, 0);
        store.notify(request("predefinita"), now, 0);
        store.notify(
            Request {
                expire_timeout: i32::MIN,
                ..request("negativa")
            },
            now,
            0,
        );

        assert_eq!(
            store.next_deadline(),
            Some(now + Duration::from_millis(500))
        );
        assert!(store.take_expired(now).is_empty());
        assert_eq!(
            store.take_expired(now + Duration::from_millis(500)),
            [timed]
        );
        assert_eq!(store.next_deadline(), None);
        assert_eq!(store.list().len(), 3);
    }
}
