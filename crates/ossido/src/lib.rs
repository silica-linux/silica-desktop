//! Ossido, il compositor Wayland di Silica.

pub mod backend;
mod capture;
pub mod commands;
mod control;
mod decorations;
mod dmabuf;
mod egl;
mod glass;
mod handlers;
mod input;
mod keys;
mod layers;
mod panorama;
mod perf;
mod render;
mod session_bus;
mod shell;
pub mod state;
mod system;
mod windows;
pub mod workspaces;

use std::io::Write;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use tracing::warn;

use crate::state::State;

/// Opzioni di avvio, lette dalla riga di comando in `main.rs`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Finestra dentro il compositor ospite (WSLg, una sessione Wayland o X11).
    #[default]
    Winit,
    /// Nessuna finestra: test automatici, CI, loop degli agenti.
    Headless,
}

#[derive(Debug)]
pub struct Options {
    pub backend: BackendKind,
    /// Dimensione dell'output headless, in pixel.
    pub size: (i32, i32),
    /// Fattore di scala intero dell'output (HiDPI).
    pub scale: i32,
    /// Porta del canale di controllo su 127.0.0.1; 0 sceglie una porta libera.
    pub control: Option<u16>,
    /// Comandi da avviare come client una volta aperto il socket Wayland.
    pub spawn: Vec<String>,
    /// Componenti della shell da avviare (`--spawn-shell`): vedono i protocolli della shell.
    pub spawn_shell: Vec<String>,
    /// Colore del desktop finché non c'è uno sfondo (RGBA 0–1, valori sRGB).
    pub background: [f32; 4],
    /// Moltiplicatore delle durate delle animazioni (0: nessuna animazione).
    pub animation_scale: f64,
    /// Qualità del vetro (`--glass auto|full|frosted|opaque`).
    pub glass: state::GlassSetting,
    /// Avvia un bus di sessione privato con `silica-daemon` (`--session-bus`).
    pub session_bus: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            backend: BackendKind::Winit,
            size: (1280, 800),
            scale: 1,
            control: None,
            spawn: Vec::new(),
            spawn_shell: Vec::new(),
            background: state::CLEAR_COLOR,
            animation_scale: 1.0,
            glass: state::GlassSetting::Auto,
            session_bus: false,
        }
    }
}

pub fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<'static, _> = EventLoop::try_new()?;
    let display: Display<State> = Display::new()?;
    let mut ossido = match options.backend {
        BackendKind::Winit => backend::init_winit(&mut event_loop, display, options.size)?,
        BackendKind::Headless => backend::init_headless(&mut event_loop, display, options.size)?,
    };

    ossido.state.clear_color = options.background;
    ossido.state.animation_speed = options.animation_scale;
    ossido.state.glass_setting = options.glass;
    if options.scale != 1 {
        ossido.state.output.change_current_state(
            None,
            None,
            Some(smithay::output::Scale::Integer(options.scale)),
            None,
        );
    }

    // Le righe su stdout sono l'interfaccia per script e test: una per variabile.
    let socket = ossido.state.socket_name.clone();
    println!("WAYLAND_DISPLAY={}", socket.to_string_lossy());
    // Vive fino alla fine di `run`: chiude il bus e il daemon all'uscita.
    let _session_bus = if options.session_bus {
        let bus = session_bus::SessionBus::start()?;
        println!("DBUS_SESSION_BUS_ADDRESS={}", bus.address);
        ossido.state.session_bus = Some(bus.address.clone());
        Some(bus)
    } else {
        None
    };
    ossido.state.system = system::SystemLink::start(ossido.state.session_bus.clone());
    if let Some(port) = options.control {
        let addr = control::listen(&event_loop.handle(), port)?;
        println!("OSSIDO_CONTROL={addr}");
    }
    std::io::stdout().flush()?;
    for command in &options.spawn {
        if let Err(err) = ossido.state.launch(command) {
            warn!("{err}");
        }
    }
    for command in &options.spawn_shell {
        if let Err(err) = ossido.state.launch_with(command, true) {
            warn!("{err}");
        }
    }

    event_loop.run(None, &mut ossido, |ossido| {
        ossido.state.space.refresh();
        ossido.state.forget_dead_windows();
        ossido.state.popups.cleanup();
        if let Err(err) = ossido.display.flush_clients() {
            warn!("flush dei client fallito: {err}");
        }
    })?;
    Ok(())
}
