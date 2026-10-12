//! Backend di Ossido: winit (annidato, per lo sviluppo) e headless (test e CI).

use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use std::time::Duration;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::pixman;
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Rectangle, Transform};
use tracing::{info, warn};

use crate::capture::{Capture, capture};
use crate::state::{CLEAR_COLOR, Ossido, State};

pub enum Backend {
    Winit(Box<WinitBackend>),
    /// Nessuna finestra né GPU: si disegna con pixman solo quando serve una cattura.
    Headless(PixmanRenderer),
}

pub struct WinitBackend {
    pub graphics: WinitGraphicsBackend<GlesRenderer>,
    pub damage_tracker: OutputDamageTracker,
}

/// Frequenza dichiarata per gli output, in millihertz.
const NESTED_REFRESH_MHZ: i32 = 60_000;
/// Intervallo dei frame callback in headless.
const HEADLESS_FRAME: Duration = Duration::from_micros(16_667);

fn new_output(name: &str, model: &str, mode: Mode, transform: Transform) -> Output {
    let output = Output::new(
        name.to_owned(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Silica".to_owned(),
            model: model.to_owned(),
        },
    );
    output.change_current_state(Some(mode), Some(transform), None, Some((0, 0).into()));
    output.set_preferred(mode);
    output
}

pub fn init_headless(
    event_loop: &mut EventLoop<'static, Ossido>,
    mut display: Display<State>,
    (width, height): (i32, i32),
) -> Result<Ossido, Box<dyn std::error::Error>> {
    let renderer = PixmanRenderer::new()?;
    let mode = Mode {
        size: (width, height).into(),
        refresh: NESTED_REFRESH_MHZ,
    };
    let output = new_output("headless", "Ossido headless", mode, Transform::Normal);
    let state = State::new(
        &mut display,
        event_loop.handle(),
        event_loop.get_signal(),
        output,
        Backend::Headless(renderer),
    )?;
    event_loop
        .handle()
        .insert_source(Timer::from_duration(HEADLESS_FRAME), |_, _, data| {
            data.state.send_frames();
            TimeoutAction::ToDuration(HEADLESS_FRAME)
        })?;
    info!("backend headless pronto ({width}x{height})");
    Ok(Ossido { state, display })
}

pub fn init_winit(
    event_loop: &mut EventLoop<'static, Ossido>,
    mut display: Display<State>,
) -> Result<Ossido, Box<dyn std::error::Error>> {
    let (graphics, winit_loop) = winit::init::<GlesRenderer>()?;
    let mode = Mode {
        size: graphics.window_size(),
        refresh: NESTED_REFRESH_MHZ,
    };
    let output = new_output("winit", "Ossido annidato", mode, Transform::Flipped180);
    let damage_tracker = OutputDamageTracker::from_output(&output);

    let state = State::new(
        &mut display,
        event_loop.handle(),
        event_loop.get_signal(),
        output,
        Backend::Winit(Box::new(WinitBackend {
            graphics,
            damage_tracker,
        })),
    )?;

    event_loop
        .handle()
        .insert_source(winit_loop, |event, _, data| match event {
            WinitEvent::Resized { size, .. } => {
                let mode = Mode {
                    size,
                    refresh: NESTED_REFRESH_MHZ,
                };
                data.state
                    .output
                    .change_current_state(Some(mode), None, None, None);
                data.state.output.set_preferred(mode);
            }
            WinitEvent::Input(event) => data.state.process_input_event(event),
            WinitEvent::Redraw => data.state.render_winit(),
            WinitEvent::CloseRequested => data.state.loop_signal.stop(),
            WinitEvent::Focus(_) => {}
        })?;
    info!("backend winit pronto");
    Ok(Ossido { state, display })
}

impl State {
    /// Disegna l'output corrente fuori schermo e lo restituisce in RGBA.
    pub fn screenshot(&mut self) -> Result<Capture, String> {
        match &mut self.backend {
            Backend::Winit(backend) => {
                capture::<_, GlesTexture>(backend.graphics.renderer(), &self.output, &self.space)
            }
            Backend::Headless(renderer) => {
                capture::<_, pixman::Image<'static, 'static>>(renderer, &self.output, &self.space)
            }
        }
    }

    fn render_winit(&mut self) {
        let Backend::Winit(backend) = &mut self.backend else {
            return;
        };
        let size = backend.graphics.window_size();
        let damage = Rectangle::from_size(size);
        let rendered = match backend.graphics.bind() {
            Ok((renderer, mut framebuffer)) => smithay::desktop::space::render_output::<
                _,
                WaylandSurfaceRenderElement<GlesRenderer>,
                _,
                _,
            >(
                &self.output,
                renderer,
                &mut framebuffer,
                1.0,
                0,
                [&self.space],
                &[],
                &mut backend.damage_tracker,
                CLEAR_COLOR,
            )
            .map(|_| ())
            .map_err(|err| format!("{err:?}")),
            Err(err) => Err(format!("{err:?}")),
        };
        match rendered {
            Ok(()) => {
                if let Err(err) = backend.graphics.submit(Some(&[damage])) {
                    warn!("submit del frame fallito: {err:?}");
                }
            }
            Err(err) => warn!("render fallito: {err}"),
        }
        backend.graphics.window().request_redraw();
        self.send_frames();
    }
}
