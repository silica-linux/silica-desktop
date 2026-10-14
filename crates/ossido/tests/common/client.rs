//! Un client Wayland minimo per i test: una finestra xdg-shell con un colore pieno.

use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;

use silica_protocols::glass::client::silica_glass_manager_v1::SilicaGlassManagerV1;
use silica_protocols::glass::client::silica_glass_v1::{self, SilicaGlassV1};
use silica_protocols::window::client::silica_window_manager_v1::SilicaWindowManagerV1;
use silica_protocols::window::client::silica_window_v1::SilicaWindowV1;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{self, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_data_source::{self, WlDataSource};
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop, event_created_child,
};
use wayland_protocols::xdg::decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::{
    self, ZxdgToplevelDecorationV1,
};
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self, XdgToplevel};
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};

/// Dimensione e colore della finestra del client (ARGB8888 in memoria: B, G, R, A).
pub const WINDOW: (u32, u32) = (100, 60);
pub const RED_BGRA: [u8; 4] = [0, 0, 255, 255];
/// Posizione della prima finestra nella cascata di Ossido (handlers.rs).
pub const FIRST_WINDOW_AT: u32 = 32;

#[derive(Default)]
struct App {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    wm_base: Option<XdgWmBase>,
    decorations: Option<ZxdgDecorationManagerV1>,
    glass: Option<SilicaGlassManagerV1>,
    appearance: Option<SilicaWindowManagerV1>,
    seat: Option<WlSeat>,
    pointer: Option<WlPointer>,
    /// Il serial dell'ultima pressione di un pulsante sulla finestra.
    press_serial: Option<u32>,
    configured: bool,
    close_requested: bool,
    /// Modo delle decorazioni scelto da Ossido.
    decoration_mode: Option<zxdg_toplevel_decoration_v1::Mode>,
    data_manager: Option<WlDataDeviceManager>,
    data_device: Option<WlDataDevice>,
    /// Trascinamento in partenza da qui: la sorgente e i dati che manda.
    drag_source: Option<WlDataSource>,
    drag_payload: Vec<u8>,
    drag_finished: bool,
    drag_cancelled: bool,
    /// Trascinamento in arrivo: l'offerta con i suoi tipi, l'ingresso e il rilascio.
    offer: Option<WlDataOffer>,
    offer_mimes: Vec<String>,
    drag_entered: Option<u32>,
    dropped: bool,
}

impl Dispatch<WlRegistry, ()> for App {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => state.compositor = Some(registry.bind(name, 4, qh, ())),
                "wl_shm" => state.shm = Some(registry.bind(name, 1, qh, ())),
                "xdg_wm_base" => state.wm_base = Some(registry.bind(name, 1, qh, ())),
                "silica_glass_manager_v1" => {
                    let version = GLASS_VERSION.with(std::cell::Cell::get).min(version);
                    state.glass = Some(registry.bind(name, version, qh, ()));
                }
                "wl_seat" => state.seat = Some(registry.bind(name, 1, qh, ())),
                "wl_data_device_manager" => {
                    state.data_manager = Some(registry.bind(name, 3.min(version), qh, ()));
                }
                "silica_window_manager_v1" => {
                    state.appearance = Some(registry.bind(name, 1, qh, ()));
                }
                "zxdg_decoration_manager_v1" => {
                    state.decorations = Some(registry.bind(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<XdgWmBase, ()> for App {
    fn event(
        _: &mut Self,
        wm_base: &XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<XdgSurface, ()> for App {
    fn event(
        state: &mut Self,
        surface: &XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<WlShm, ()> for App {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for App {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: wayland_client::WEnum::Value(caps),
        } = event
            && caps.contains(wl_seat::Capability::Pointer)
            && state.pointer.is_none()
        {
            state.pointer = Some(seat.get_pointer(qh, ()));
        }
    }
}

impl Dispatch<WlPointer, ()> for App {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button {
            serial,
            state: wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed),
            ..
        } = event
        {
            state.press_serial = Some(serial);
        }
    }
}

delegate_noop!(App: ignore WlDataDeviceManager);

impl Dispatch<WlDataDevice, ()> for App {
    fn event(
        state: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::DataOffer { id } => {
                state.offer = Some(id);
                state.offer_mimes.clear();
            }
            wl_data_device::Event::Enter { serial, id, .. } => {
                state.drag_entered = Some(serial);
                if id.is_none() {
                    state.offer = None;
                }
            }
            wl_data_device::Event::Leave => state.drag_entered = None,
            wl_data_device::Event::Drop => state.dropped = true,
            _ => {}
        }
    }

    event_created_child!(App, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);
}

impl Dispatch<WlDataOffer, ()> for App {
    fn event(
        state: &mut Self,
        _: &WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            state.offer_mimes.push(mime_type);
        }
    }
}

impl Dispatch<WlDataSource, ()> for App {
    fn event(
        state: &mut Self,
        _: &WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { fd, .. } => {
                let mut file = std::fs::File::from(fd);
                file.write_all(&state.drag_payload).unwrap();
            }
            wl_data_source::Event::DndFinished => state.drag_finished = true,
            wl_data_source::Event::Cancelled => state.drag_cancelled = true,
            _ => {}
        }
    }
}

delegate_noop!(App: ignore WlCompositor);
delegate_noop!(App: ignore WlSurface);
delegate_noop!(App: ignore WlBuffer);
delegate_noop!(App: WlShmPool);
delegate_noop!(App: ignore ZxdgDecorationManagerV1);
delegate_noop!(App: ignore SilicaGlassManagerV1);
delegate_noop!(App: ignore SilicaGlassV1);
delegate_noop!(App: ignore SilicaWindowManagerV1);
delegate_noop!(App: ignore SilicaWindowV1);

impl Dispatch<ZxdgToplevelDecorationV1, ()> for App {
    fn event(
        state: &mut Self,
        _: &ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_toplevel_decoration_v1::Event::Configure {
            mode: wayland_client::WEnum::Value(mode),
        } = event
        {
            state.decoration_mode = Some(mode);
        }
    }
}

impl Dispatch<XdgToplevel, ()> for App {
    fn event(
        state: &mut Self,
        _: &XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_toplevel::Event::Close = event {
            state.close_requested = true;
        }
    }
}

/// Una finestra aperta su Ossido; resta viva finché esiste questo valore.
pub struct TestWindow {
    queue: EventQueue<App>,
    app: App,
    surface: WlSurface,
    _toplevel: XdgToplevel,
    _decoration: Option<ZxdgToplevelDecorationV1>,
    glass: Option<SilicaGlassV1>,
    appearance: Option<SilicaWindowV1>,
    buffer: WlBuffer,
}

/// Una regione di vetro per `TestWindow::set_glass`: x, y, larghezza, altezza, raggio,
/// materiale.
pub type Region = (i32, i32, i32, i32, f64, silica_glass_v1::Material);

thread_local! {
    /// Versione di silica-glass-v1 che le finestre di questo thread (un test) chiedono.
    pub static GLASS_VERSION: std::cell::Cell<u32> = const { std::cell::Cell::new(4) };
    /// L'app_id delle finestre che questo thread apre (nessuno se vuoto).
    pub static APP_ID: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

impl TestWindow {
    /// Apre una finestra `WINDOW` piena di `RED_BGRA` e aspetta che Ossido la riceva.
    pub fn open(socket: &Path, title: &str) -> Self {
        Self::open_sized(socket, title, WINDOW)
    }

    /// Come `open`, con una dimensione a scelta (in pixel, a scala 1).
    pub fn open_sized(socket: &Path, title: &str, size: (u32, u32)) -> Self {
        Self::open_with(socket, title, size, false)
    }

    /// Una finestra che chiede le decorazioni a Ossido (`xdg-decoration`).
    pub fn open_decorated(socket: &Path, title: &str, size: (u32, u32)) -> Self {
        Self::open_with(socket, title, size, true)
    }

    /// Una finestra con pixel a scelta (BGRA premoltiplicato, righe dall'alto).
    #[allow(dead_code)]
    pub fn open_pixels(socket: &Path, title: &str, size: (u32, u32), pixels: &[u8]) -> Self {
        Self::open_full(socket, title, size, false, pixels)
    }

    fn open_with(socket: &Path, title: &str, (w, h): (u32, u32), decorated: bool) -> Self {
        let pixels = RED_BGRA.repeat((w * h) as usize);
        Self::open_full(socket, title, (w, h), decorated, &pixels)
    }

    fn open_full(
        socket: &Path,
        title: &str,
        (w, h): (u32, u32),
        decorated: bool,
        pixels: &[u8],
    ) -> Self {
        let conn = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        let mut app = App::default();
        conn.display().get_registry(&qh, ());
        queue.roundtrip(&mut app).unwrap();

        let compositor = app.compositor.clone().expect("wl_compositor assente");
        let shm = app.shm.clone().expect("wl_shm assente");
        let wm_base = app.wm_base.clone().expect("xdg_wm_base assente");

        let surface = compositor.create_surface(&qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());
        toplevel.set_title(title.to_owned());
        let app_id = APP_ID.with(|id| id.borrow().clone());
        if !app_id.is_empty() {
            toplevel.set_app_id(app_id);
        }
        let decoration = decorated.then(|| {
            app.decorations
                .clone()
                .expect("zxdg_decoration_manager_v1 assente")
                .get_toplevel_decoration(&toplevel, &qh, ())
        });
        surface.commit();
        while !app.configured {
            queue.blocking_dispatch(&mut app).unwrap();
        }

        let stride = w * 4;
        let mut file = tempfile::tempfile().unwrap();
        assert_eq!(pixels.len(), (w * h * 4) as usize, "pixel della finestra");
        file.write_all(pixels).unwrap();
        let pool = shm.create_pool(file.as_fd(), (stride * h) as i32, &qh, ());
        let buffer = pool.create_buffer(
            0,
            w as i32,
            h as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            &qh,
            (),
        );
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.commit();
        queue.roundtrip(&mut app).unwrap();
        Self {
            queue,
            app,
            surface,
            _toplevel: toplevel,
            _decoration: decoration,
            glass: None,
            appearance: None,
            buffer,
        }
    }

    /// Ricommitta lo stesso buffer `count` volte, ogni `every`: frame a ritmo noto.
    #[allow(dead_code)]
    pub fn commit_frames(&mut self, count: u32, every: std::time::Duration) {
        let (w, h) = WINDOW;
        for _ in 0..count {
            std::thread::sleep(every);
            self.surface.attach(Some(&self.buffer), 0, 0);
            self.surface.damage_buffer(0, 0, w as i32, h as i32);
            self.surface.commit();
            self.queue.flush().unwrap();
        }
        self.queue.roundtrip(&mut self.app).unwrap();
    }

    /// Dichiara le regioni di vetro e fa il commit. Errore se Ossido ha rifiutato le
    /// richieste (errore di protocollo: il client viene disconnesso).
    #[allow(dead_code)]
    pub fn set_glass(&mut self, regions: &[Region]) -> Result<(), String> {
        let qh = self.queue.handle();
        let manager = self
            .app
            .glass
            .clone()
            .expect("silica_glass_manager_v1 assente");
        let glass = self
            .glass
            .get_or_insert_with(|| manager.get_glass(&self.surface, &qh, ()));
        glass.clear();
        for &(x, y, w, h, radius, material) in regions {
            glass.add_region(x, y, w, h, radius, material);
        }
        self.surface.commit();
        self.queue
            .roundtrip(&mut self.app)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Come `set_glass`, con un'opacità per regione (`add_translucent_region`, versione 3).
    #[allow(dead_code)]
    pub fn set_translucent_glass(&mut self, regions: &[(Region, f64)]) -> Result<(), String> {
        let qh = self.queue.handle();
        let manager = self
            .app
            .glass
            .clone()
            .expect("silica_glass_manager_v1 assente");
        let glass = self
            .glass
            .get_or_insert_with(|| manager.get_glass(&self.surface, &qh, ()));
        glass.clear();
        for &((x, y, w, h, radius, material), opacity) in regions {
            glass.add_translucent_region(x, y, w, h, radius, material, opacity);
        }
        self.surface.commit();
        self.queue
            .roundtrip(&mut self.app)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Richieste di vetro libere (versione 4: `separate`, `add_tip`) dopo un `clear`, poi
    /// il commit.
    #[allow(dead_code)]
    pub fn set_glass_with(
        &mut self,
        requests: impl FnOnce(&silica_protocols::glass::client::silica_glass_v1::SilicaGlassV1),
    ) -> Result<(), String> {
        let qh = self.queue.handle();
        let manager = self
            .app
            .glass
            .clone()
            .expect("silica_glass_manager_v1 assente");
        let glass = self
            .glass
            .get_or_insert_with(|| manager.get_glass(&self.surface, &qh, ()));
        glass.clear();
        requests(glass);
        self.surface.commit();
        self.queue
            .roundtrip(&mut self.app)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Contenuto a tutta finestra (`silica-window-v1`) con una barra degli strumenti alta
    /// `toolbar` (0: torna alla barra di Ossido), poi il commit.
    #[allow(dead_code)]
    pub fn set_full_size_content(&mut self, toolbar: u32) {
        let qh = self.queue.handle();
        let manager = self
            .app
            .appearance
            .clone()
            .expect("silica_window_manager_v1 assente");
        let window = self
            .appearance
            .get_or_insert_with(|| manager.get_window(&self.surface, &qh, ()));
        window.set_full_size_content(toolbar);
        self.surface.commit();
        self.queue.roundtrip(&mut self.app).unwrap();
    }

    /// Chiede a Ossido di spostare la finestra (`xdg_toplevel.move`) con il serial dell'ultima
    /// pressione ricevuta, come fa l'embedder dalla barra degli strumenti dell'app.
    #[allow(dead_code)]
    pub fn start_move(&mut self) {
        self.queue.roundtrip(&mut self.app).unwrap();
        let seat = self.app.seat.clone().expect("wl_seat assente");
        let serial = self.app.press_serial.expect("nessuna pressione ricevuta");
        self._toplevel._move(&seat, serial);
        self.queue.roundtrip(&mut self.app).unwrap();
    }

    fn data_device(&mut self) -> WlDataDevice {
        self.queue.roundtrip(&mut self.app).unwrap();
        let qh = self.queue.handle();
        if self.app.data_device.is_none() {
            let manager = self
                .app
                .data_manager
                .clone()
                .expect("data_device_manager assente");
            let seat = self.app.seat.clone().expect("wl_seat assente");
            self.app.data_device = Some(manager.get_data_device(&seat, &qh, ()));
            self.queue.roundtrip(&mut self.app).unwrap();
        }
        self.app.data_device.clone().unwrap()
    }

    /// Si prepara a ricevere trascinamenti (crea il wl_data_device).
    #[allow(dead_code)]
    pub fn accept_drops(&mut self) {
        self.data_device();
    }

    /// Comincia un trascinamento (`start_drag`) con il serial dell'ultima pressione: offre
    /// `mime` con `payload`, e un'icona `side` × `side` di un colore pieno (BGRA).
    #[allow(dead_code)]
    pub fn start_drag(&mut self, mime: &str, payload: &[u8], side: u32, color: [u8; 4]) {
        let device = self.data_device();
        let qh = self.queue.handle();
        let manager = self.app.data_manager.clone().unwrap();
        let source = manager.create_data_source(&qh, ());
        source.offer(mime.to_owned());
        source.set_actions(
            wl_data_device_manager::DndAction::Copy | wl_data_device_manager::DndAction::Move,
        );
        self.app.drag_payload = payload.to_vec();
        let compositor = self.app.compositor.clone().unwrap();
        let shm = self.app.shm.clone().unwrap();
        let icon = compositor.create_surface(&qh, ());
        let stride = side * 4;
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&color.repeat((side * side) as usize))
            .unwrap();
        let pool = shm.create_pool(file.as_fd(), (stride * side) as i32, &qh, ());
        let buffer = pool.create_buffer(
            0,
            side as i32,
            side as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            &qh,
            (),
        );
        let serial = self.app.press_serial.expect("nessuna pressione ricevuta");
        device.start_drag(Some(&source), &self.surface, Some(&icon), serial);
        icon.attach(Some(&buffer), 0, 0);
        icon.damage_buffer(0, 0, side as i32, side as i32);
        icon.commit();
        self.app.drag_source = Some(source);
        self.queue.roundtrip(&mut self.app).unwrap();
    }

    /// Legge gli eventi (per la sorgente: manda i dati chiesti) e dice se il trascinamento è
    /// finito (true) o annullato (false); None se è ancora in corso.
    #[allow(dead_code)]
    pub fn drag_outcome(&mut self) -> Option<bool> {
        self.queue.roundtrip(&mut self.app).unwrap();
        if self.app.drag_finished {
            Some(true)
        } else if self.app.drag_cancelled {
            Some(false)
        } else {
            None
        }
    }

    /// Il trascinamento sopra questa finestra: i tipi offerti (None se non è entrato).
    #[allow(dead_code)]
    pub fn drag_over(&mut self) -> Option<Vec<String>> {
        self.queue.roundtrip(&mut self.app).unwrap();
        self.app.drag_entered?;
        Some(self.app.offer_mimes.clone())
    }

    /// Accetta `mime` con la copia, come fa un'app sopra una destinazione valida.
    #[allow(dead_code)]
    pub fn accept_offer(&mut self, mime: &str) {
        let serial = self.app.drag_entered.expect("nessun trascinamento");
        let offer = self.app.offer.clone().expect("nessuna offerta");
        offer.accept(serial, Some(mime.to_owned()));
        offer.set_actions(
            wl_data_device_manager::DndAction::Copy,
            wl_data_device_manager::DndAction::Copy,
        );
        self.queue.roundtrip(&mut self.app).unwrap();
    }

    /// Dopo il rilascio: chiede i dati in `mime` (la sorgente li scrive dal suo lato con
    /// `pump`, chiamata in mezzo) e chiude l'offerta.
    #[allow(dead_code)]
    pub fn receive_drop(&mut self, mime: &str, pump: impl FnOnce()) -> Vec<u8> {
        self.queue.roundtrip(&mut self.app).unwrap();
        assert!(self.app.dropped, "nessun rilascio");
        let offer = self.app.offer.clone().expect("nessuna offerta");
        let (mut reader, writer) = std::io::pipe().unwrap();
        offer.receive(mime.to_owned(), std::os::fd::AsFd::as_fd(&writer));
        self.queue.roundtrip(&mut self.app).unwrap();
        drop(writer);
        pump();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut data).unwrap();
        offer.finish();
        self.queue.roundtrip(&mut self.app).unwrap();
        data
    }

    /// Distrugge il toplevel (la finestra si chiude) lasciando viva la superficie.
    #[allow(dead_code)]
    pub fn destroy_toplevel(&mut self) {
        self._toplevel.destroy();
        self.queue.roundtrip(&mut self.app).unwrap();
    }

    /// Il modo delle decorazioni che Ossido ha scelto, dopo aver letto gli eventi.
    #[allow(dead_code)]
    pub fn decoration_mode(&mut self) -> Option<zxdg_toplevel_decoration_v1::Mode> {
        self.queue.roundtrip(&mut self.app).unwrap();
        self.app.decoration_mode
    }

    /// Legge gli eventi in arrivo e dice se Ossido ha chiesto la chiusura.
    pub fn close_requested(&mut self) -> bool {
        self.queue.roundtrip(&mut self.app).unwrap();
        self.app.close_requested
    }
}
