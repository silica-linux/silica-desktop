//! The clipboard history: `dev.frascella.Silica.Clipboard1` on
//! `/dev/frascella/Silica/Clipboard1`, for Faro's clipboard scope.
//!
//! The daemon watches the selection over `ext-data-control-v1`. Ossido offers that global only
//! to silica-daemon. Every new text, image or file list is read and kept, newest first. Copying
//! an entry back sets it as the selection from a data-control source of the daemon's own.
//!
//! Privacy:
//! - Selections marked as secret (the `x-kde-passwordManagerHint` type that password fields
//!   and managers add) are never read.
//! - The history lives in memory. Images go to `$XDG_RUNTIME_DIR` (tmpfs, gone at logout).
//! - It is bounded: [`MAX_ENTRIES`] entries, [`MAX_AGE`] old at most (PROVISIONAL, not
//!   measured). It can be cleared.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use event_listener::Event;
use serde::Serialize;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::{self, ExtDataControlOfferV1},
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};
use zbus::object_server::SignalEmitter;
use zbus::{Connection as Bus, interface};

pub const CLIPBOARD_PATH: &str = "/dev/frascella/Silica/Clipboard1";

/// How many entries the history keeps (PROVISIONAL).
pub const MAX_ENTRIES: usize = 50;
/// How long an entry stays (PROVISIONAL).
pub const MAX_AGE: Duration = Duration::from_secs(8 * 3600);
/// Largest text kept, in bytes.
const MAX_TEXT: usize = 1 << 20;
/// Largest image kept, in bytes.
const MAX_IMAGE: usize = 16 << 20;

/// The secret marker of password fields and managers.
const SECRET_HINT: &str = "x-kde-passwordManagerHint";
const TEXT_TYPES: &[&str] = &[
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "TEXT",
    "STRING",
];
const URI_LIST: &str = "text/uri-list";
const PNG: &str = "image/png";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Image,
    Files,
}

/// One entry of the history.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    pub id: u64,
    pub kind: Kind,
    /// The text, or the file list (`text/uri-list`, one URI per line).
    pub text: String,
    /// Where the image is (images only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<PathBuf>,
    /// Seconds since the epoch.
    pub time: u64,
    /// The raw data and its type, to put it back on the clipboard.
    #[serde(skip)]
    data: Vec<u8>,
    #[serde(skip)]
    mime: String,
}

/// The entries, newest first.
#[derive(Debug, Default)]
pub struct History {
    entries: VecDeque<Entry>,
    next: u64,
    dir: Option<PathBuf>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl History {
    /// A history whose images go to `dir` (`None`: images are not kept).
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            entries: VecDeque::new(),
            next: 1,
            dir,
        }
    }

    /// Adds a selection read from the clipboard; the same content moves to the top. Returns
    /// whether the history changed.
    pub fn push(&mut self, mime: &str, data: Vec<u8>) -> bool {
        let kind = if mime == PNG {
            Kind::Image
        } else if mime == URI_LIST {
            Kind::Files
        } else {
            Kind::Text
        };
        if kind != Kind::Image && String::from_utf8_lossy(&data).trim().is_empty() {
            return false;
        }
        if let Some(index) = self.entries.iter().position(|e| e.data == data) {
            if index == 0 {
                return false;
            }
            if let Some(mut entry) = self.entries.remove(index) {
                entry.time = now();
                self.entries.push_front(entry);
            }
            return true;
        }
        let id = self.next;
        self.next += 1;
        let image = match kind {
            Kind::Image => {
                let Some(dir) = &self.dir else {
                    return false;
                };
                let path = dir.join(format!("{id}.png"));
                if std::fs::create_dir_all(dir).is_err() || std::fs::write(&path, &data).is_err() {
                    return false;
                }
                Some(path)
            }
            _ => None,
        };
        let text = match kind {
            Kind::Image => String::new(),
            _ => String::from_utf8_lossy(&data).into_owned(),
        };
        self.entries.push_front(Entry {
            id,
            kind,
            text,
            image,
            time: now(),
            data,
            mime: mime.to_owned(),
        });
        self.prune();
        true
    }

    /// Drops the entries past the limits.
    pub fn prune(&mut self) {
        let oldest = now().saturating_sub(MAX_AGE.as_secs());
        while self
            .entries
            .back()
            .is_some_and(|e| self.entries.len() > MAX_ENTRIES || e.time < oldest)
        {
            if let Some(entry) = self.entries.pop_back() {
                forget(&entry);
            }
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn get(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn remove(&mut self, id: u64) -> bool {
        match self.entries.iter().position(|e| e.id == id) {
            Some(index) => {
                if let Some(entry) = self.entries.remove(index) {
                    forget(&entry);
                }
                true
            }
            None => false,
        }
    }

    pub fn clear(&mut self) {
        for entry in self.entries.drain(..) {
            forget(&entry);
        }
    }

    /// The entries as JSON, newest first (no raw data).
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.entries).unwrap_or_else(|_| "[]".to_owned())
    }
}

fn forget(entry: &Entry) {
    if let Some(image) = &entry.image {
        let _ = std::fs::remove_file(image);
    }
}

/// Which type to read from an offer, or none (secret, or nothing we keep).
pub fn chosen_type(types: &[String]) -> Option<&'static str> {
    if types.iter().any(|t| t == SECRET_HINT) {
        return None;
    }
    let has = |wanted: &str| types.iter().any(|t| t == wanted);
    if has(URI_LIST) {
        return Some(URI_LIST);
    }
    if let Some(text) = TEXT_TYPES.iter().find(|t| has(t)) {
        return Some(text);
    }
    has(PNG).then_some(PNG)
}

#[derive(Clone)]
struct Shared {
    history: Arc<Mutex<History>>,
    changed: Arc<Event>,
}

impl Shared {
    fn history(&self) -> MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The service on the session bus, and the watcher of the selection.
pub struct ClipboardService {
    connection: Bus,
    shared: Shared,
}

impl ClipboardService {
    /// Publishes the interface and starts watching the selection (if Wayland and the
    /// data-control global are there: otherwise the history stays empty).
    pub async fn serve(connection: Bus) -> zbus::Result<Self> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|dir| PathBuf::from(dir).join("silica").join("clipboard"));
        let shared = Shared {
            history: Arc::new(Mutex::new(History::new(dir))),
            changed: Arc::new(Event::new()),
        };
        connection
            .object_server()
            .at(
                CLIPBOARD_PATH,
                Clipboard {
                    shared: shared.clone(),
                },
            )
            .await?;
        let watcher = shared.clone();
        let _ = std::thread::Builder::new()
            .name("clipboard".to_owned())
            .spawn(move || {
                if let Err(err) = watch(watcher) {
                    eprintln!("silica-daemon: no clipboard history: {err}");
                }
            });
        Ok(Self { connection, shared })
    }

    /// Emits `Changed` on every change of the history.
    pub async fn run(&self) -> zbus::Result<()> {
        loop {
            let listener = self.shared.changed.listen();
            listener.await;
            let emitter = SignalEmitter::new(&self.connection, CLIPBOARD_PATH)?;
            Clipboard::changed(&emitter).await?;
        }
    }
}

struct Clipboard {
    shared: Shared,
}

#[interface(name = "dev.frascella.Silica.Clipboard1")]
impl Clipboard {
    /// The entries as JSON, newest first: `[{id, kind, text, image?, time}]`.
    fn history(&self) -> String {
        let mut history = self.shared.history();
        history.prune();
        history.to_json()
    }

    /// Puts an entry back on the clipboard (it moves to the top). False if it is gone.
    fn copy(&self, id: u64) -> bool {
        let Some(entry) = self.shared.history().get(id).cloned() else {
            return false;
        };
        std::thread::Builder::new()
            .name("clipboard-source".to_owned())
            .spawn(move || {
                if let Err(err) = offer(&entry) {
                    eprintln!("silica-daemon: clipboard: {err}");
                }
            })
            .is_ok()
    }

    /// Puts a PNG file on the clipboard as an image (a screenshot sent to the clipboard).
    /// False if it is not a readable PNG.
    fn copy_image(&self, path: &str) -> bool {
        let Some(data) = read_png(Path::new(path)) else {
            return false;
        };
        let entry = Entry {
            id: 0,
            kind: Kind::Image,
            text: String::new(),
            image: None,
            time: now(),
            data,
            mime: PNG.to_owned(),
        };
        std::thread::Builder::new()
            .name("clipboard-source".to_owned())
            .spawn(move || {
                if let Err(err) = offer(&entry) {
                    eprintln!("silica-daemon: clipboard: {err}");
                }
            })
            .is_ok()
    }

    fn remove(&self, id: u64) -> bool {
        let removed = self.shared.history().remove(id);
        if removed {
            self.shared.changed.notify(usize::MAX);
        }
        removed
    }

    fn clear(&self) {
        self.shared.history().clear();
        self.shared.changed.notify(usize::MAX);
    }

    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// Puts a list of files (`file://` URIs) on the clipboard, as a copy in a file manager does:
/// the share service's `copy` target. False if the list is empty or the source could not start.
pub fn copy_files(uris: &[String]) -> bool {
    if uris.is_empty() {
        return false;
    }
    let text = uris.join("\r\n");
    let entry = Entry {
        id: 0,
        kind: Kind::Files,
        text: text.clone(),
        image: None,
        time: now(),
        data: text.into_bytes(),
        mime: URI_LIST.to_owned(),
    };
    std::thread::Builder::new()
        .name("clipboard-source".to_owned())
        .spawn(move || {
            if let Err(err) = offer(&entry) {
                eprintln!("silica-daemon: clipboard: {err}");
            }
        })
        .is_ok()
}

/// Largest PNG [`Clipboard::copy_image`] reads (a full 8K screen fits).
const MAX_COPIED_IMAGE: u64 = 128 << 20;

/// The bytes of a PNG file, if it is one and not too large.
pub fn read_png(path: &Path) -> Option<Vec<u8>> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_COPIED_IMAGE {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    data.starts_with(SIGNATURE).then_some(data)
}

type Error = Box<dyn std::error::Error + Send + Sync>;

/// The Wayland side of the watcher: the offers being described and the current selection.
struct Watcher {
    shared: Shared,
    types: HashMap<wayland_client::backend::ObjectId, Vec<String>>,
    pending: Vec<ExtDataControlOfferV1>,
}

fn watch(shared: Shared) -> Result<(), Error> {
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Watcher>(&connection)?;
    let qh = queue.handle();
    let manager: ExtDataControlManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=9, ())?;
    let _device = manager.get_data_device(&seat, &qh, ());
    let mut watcher = Watcher {
        shared,
        types: HashMap::new(),
        pending: Vec::new(),
    };
    loop {
        queue.blocking_dispatch(&mut watcher)?;
        for offer in std::mem::take(&mut watcher.pending) {
            let types = watcher.types.remove(&offer.id()).unwrap_or_default();
            if let Some(mime) = chosen_type(&types) {
                let (mut reader, writer) = std::io::pipe()?;
                offer.receive(mime.to_owned(), writer.as_fd());
                connection.flush()?;
                drop(writer);
                let shared = watcher.shared.clone();
                // Read off the Wayland thread: the source may take its time.
                let _ = std::thread::Builder::new()
                    .name("clipboard-read".to_owned())
                    .spawn(move || {
                        let limit = if mime == PNG { MAX_IMAGE } else { MAX_TEXT };
                        let mut data = Vec::new();
                        let read = (&mut reader).take(limit as u64 + 1).read_to_end(&mut data);
                        if read.is_ok() && data.len() <= limit && shared.history().push(mime, data)
                        {
                            shared.changed.notify(usize::MAX);
                        }
                    });
            }
            offer.destroy();
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Watcher {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Watcher {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlManagerV1, ()> for Watcher {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: <ExtDataControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::DataOffer { id } => {
                state.types.insert(id.id(), Vec::new());
            }
            ext_data_control_device_v1::Event::Selection { id: Some(offer) } => {
                state.pending.push(offer);
            }
            _ => {}
        }
    }

    event_created_child!(Watcher, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        offer: &ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            state.types.entry(offer.id()).or_default().push(mime_type);
        }
    }
}

/// The source that puts an entry back on the clipboard, until another selection replaces
/// it.
struct Source {
    data: Vec<u8>,
    done: bool,
}

fn offer(entry: &Entry) -> Result<(), Error> {
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Source>(&connection)?;
    let qh = queue.handle();
    let manager: ExtDataControlManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=9, ())?;
    let device = manager.get_data_device(&seat, &qh, ());
    let source = manager.create_data_source(&qh, ());
    let types: &[&str] = match entry.kind {
        Kind::Text => TEXT_TYPES,
        Kind::Files => &[URI_LIST],
        Kind::Image => &[PNG],
    };
    for mime in types {
        source.offer((*mime).to_owned());
    }
    if !types.contains(&entry.mime.as_str()) {
        source.offer(entry.mime.clone());
    }
    if entry.kind == Kind::Files {
        for mime in TEXT_TYPES {
            source.offer((*mime).to_owned());
        }
    }
    device.set_selection(Some(&source));
    let mut state = Source {
        data: entry.data.clone(),
        done: false,
    };
    while !state.done {
        queue.blocking_dispatch(&mut state)?;
    }
    source.destroy();
    device.destroy();
    connection.flush()?;
    Ok(())
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Source {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Source {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlManagerV1, ()> for Source {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: <ExtDataControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for Source {
    fn event(
        _: &mut Self,
        _: &ExtDataControlDeviceV1,
        _: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(Source, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for Source {
    fn event(
        _: &mut Self,
        _: &ExtDataControlOfferV1,
        _: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlSourceV1, ()> for Source {
    fn event(
        state: &mut Self,
        _: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_source_v1::Event::Send { fd, .. } => {
                let mut file = std::fs::File::from(fd);
                let _ = file.write_all(&state.data);
            }
            ext_data_control_source_v1::Event::Cancelled => state.done = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_duplicates_move_up_and_limits_hold() {
        let mut history = History::new(None);
        assert!(history.push("text/plain", b"one".to_vec()));
        assert!(history.push("text/plain", b"two".to_vec()));
        assert!(!history.push("text/plain", b"two".to_vec()));
        assert!(history.push("text/plain", b"one".to_vec()));
        let texts: Vec<_> = history.entries().map(|e| e.text.clone()).collect();
        assert_eq!(texts, ["one", "two"]);
        // Blank text and images without a folder are not kept.
        assert!(!history.push("text/plain", b"  \n".to_vec()));
        assert!(!history.push(PNG, vec![1, 2, 3]));
        for i in 0..(MAX_ENTRIES + 5) {
            history.push("text/plain", format!("item {i}").into_bytes());
        }
        assert_eq!(history.entries().count(), MAX_ENTRIES);
        let first = history.entries().next().map(|e| e.id);
        assert!(first.is_some_and(|id| history.remove(id)));
        history.clear();
        assert_eq!(history.entries().count(), 0);
    }

    #[test]
    fn secrets_are_never_read_and_files_win_over_text() {
        let types = |list: &[&str]| list.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>();
        assert_eq!(chosen_type(&types(&["text/plain", SECRET_HINT])), None);
        assert_eq!(
            chosen_type(&types(&["text/plain", "text/plain;charset=utf-8"])),
            Some("text/plain;charset=utf-8")
        );
        assert_eq!(
            chosen_type(&types(&["text/plain", URI_LIST])),
            Some(URI_LIST)
        );
        assert_eq!(chosen_type(&types(&[PNG])), Some(PNG));
        assert_eq!(chosen_type(&types(&["application/x-other"])), None);
    }

    #[test]
    fn only_png_files_are_copied_as_images() {
        let dir = std::env::temp_dir().join(format!("silica-copy-image-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("shot.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nrest").unwrap();
        let text = dir.join("shot.txt");
        std::fs::write(&text, b"hello").unwrap();
        assert_eq!(
            read_png(&png).as_deref(),
            Some(&b"\x89PNG\r\n\x1a\nrest"[..])
        );
        assert_eq!(read_png(&text), None);
        assert_eq!(read_png(&dir), None);
        assert_eq!(read_png(&dir.join("missing.png")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_has_no_raw_data() {
        let mut history = History::new(None);
        history.push(URI_LIST, b"file:///home/a.txt\n".to_vec());
        let json = history.to_json();
        assert!(json.contains("\"kind\":\"files\""));
        assert!(!json.contains("data"));
    }
}
