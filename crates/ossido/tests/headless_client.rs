//! Un vero client Wayland apre una finestra xdg-shell su Ossido headless,
//! e la finestra compare nello screenshot del canale di controllo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

use common::{Headless, WIDTH, assert_clear_color, decode};
use silica_control::ScreenshotParams;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};

/// Dimensione e colore della finestra del client (ARGB8888 in memoria: B, G, R, A).
const WINDOW: (u32, u32) = (100, 60);
const RED_BGRA: [u8; 4] = [0, 0, 255, 255];
/// Posizione della prima finestra nella cascata di Ossido (handlers.rs).
const FIRST_WINDOW_AT: u32 = 32;

#[derive(Default)]
struct App {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    wm_base: Option<XdgWmBase>,
    configured: bool,
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
delegate_noop!(App: ignore XdgToplevel);
delegate_noop!(App: WlShmPool);

#[test]
fn xdg_toplevel_appears_in_screenshot() {
    let ossido = Headless::start("client");
    let conn = Connection::from_socket(UnixStream::connect(&ossido.wayland).unwrap()).unwrap();
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
    toplevel.set_title("test".to_owned());
    surface.commit();
    while !app.configured {
        queue.blocking_dispatch(&mut app).unwrap();
    }

    let (w, h) = WINDOW;
    let stride = w * 4;
    let mut file = tempfile::tempfile().unwrap();
    let pixels: Vec<u8> = RED_BGRA.repeat((w * h) as usize);
    file.write_all(&pixels).unwrap();
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

    let path = ossido.dir.join("client.png");
    ossido
        .client()
        .screenshot(&ScreenshotParams {
            path: Some(path.to_string_lossy().into_owned()),
        })
        .unwrap();
    let (_, shot) = decode(&std::fs::read(&path).unwrap());

    let pixel = |x: u32, y: u32| {
        let offset = ((y * WIDTH + x) * 4) as usize;
        shot[offset..offset + 4].to_vec()
    };
    let at = FIRST_WINDOW_AT;
    for (x, y) in [(at, at), (at + w / 2, at + h / 2), (at + w - 1, at + h - 1)] {
        assert_eq!(
            pixel(x, y),
            [255, 0, 0, 255],
            "finestra assente in ({x}, {y})"
        );
    }
    assert_clear_color(&shot, at - 1, at);
    assert_clear_color(&shot, at + w, at + h);
}
