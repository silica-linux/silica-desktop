//! Un client Wayland minimo per i test: una finestra xdg-shell con un colore pieno.

use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;

use silica_protocols::glass::client::silica_glass_manager_v1::SilicaGlassManagerV1;
use silica_protocols::glass::client::silica_glass_v1::{self, SilicaGlassV1};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
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
    configured: bool,
    close_requested: bool,
    /// Modo delle decorazioni scelto da Ossido.
    decoration_mode: Option<zxdg_toplevel_decoration_v1::Mode>,
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
            name, interface, ..
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => state.compositor = Some(registry.bind(name, 4, qh, ())),
                "wl_shm" => state.shm = Some(registry.bind(name, 1, qh, ())),
                "xdg_wm_base" => state.wm_base = Some(registry.bind(name, 1, qh, ())),
                "silica_glass_manager_v1" => state.glass = Some(registry.bind(name, 1, qh, ())),
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

delegate_noop!(App: ignore WlCompositor);
delegate_noop!(App: ignore WlSurface);
delegate_noop!(App: ignore WlBuffer);
delegate_noop!(App: WlShmPool);
delegate_noop!(App: ignore ZxdgDecorationManagerV1);
delegate_noop!(App: ignore SilicaGlassManagerV1);
delegate_noop!(App: ignore SilicaGlassV1);

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
    buffer: WlBuffer,
}

/// Una regione di vetro per `TestWindow::set_glass`: x, y, larghezza, altezza, raggio,
/// materiale.
pub type Region = (i32, i32, i32, i32, f64, silica_glass_v1::Material);

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
