//! Backend di Ossido: winit (annidato, per lo sviluppo) e headless (test e CI).

use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use std::time::{Duration, Instant};

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::pixman;
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Rectangle, Transform};
use tracing::{info, warn};

use crate::capture::{Capture, capture, capture_elements};
use crate::render::{RenderContext, output_elements};
use crate::state::{Ossido, State};

pub enum Backend {
    Winit(Box<WinitBackend>),
    /// Nessuna finestra: si disegna solo quando serve una cattura, con GLES su un device
    /// EGL (la GPU o llvmpipe) se c'è, altrimenti con pixman.
    Headless(Headless),
}

pub enum Headless {
    Gles(Box<GlesRenderer>),
    Pixman(PixmanRenderer),
}

/// Il renderer headless, da `OSSIDO_HEADLESS_RENDERER`: GLES su llvmpipe (predefinito),
/// `gpu` per GLES sulla GPU, `pixman` senza GLES. Se EGL manca, pixman.
fn headless_renderer() -> Result<Headless, Box<dyn std::error::Error>> {
    let choice = std::env::var("OSSIDO_HEADLESS_RENDERER").unwrap_or_default();
    if choice != "pixman" {
        match crate::egl::headless_renderer(choice == "gpu") {
            Ok(renderer) => return Ok(Headless::Gles(Box::new(renderer))),
            Err(err) => warn!("headless senza GLES, uso pixman: {err}"),
        }
    }
    Ok(Headless::Pixman(PixmanRenderer::new()?))
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
    let renderer = headless_renderer()?;
    let mode = Mode {
        size: (width, height).into(),
        refresh: NESTED_REFRESH_MHZ,
    };
    let output = new_output("headless", "Ossido headless", mode, Transform::Normal);
    let mut state = State::new(
        &mut display,
        event_loop.handle(),
        event_loop.get_signal(),
        output,
        Backend::Headless(renderer),
    )?;
    event_loop
        .handle()
        .insert_source(Timer::from_duration(HEADLESS_FRAME), |_, _, data| {
            data.state.advance_spaces(std::time::Instant::now());
            data.state.send_frames();
            TimeoutAction::ToDuration(HEADLESS_FRAME)
        })?;
    if let Backend::Headless(Headless::Gles(renderer)) = &state.backend {
        state.software_renderer = is_software(renderer);
    }
    info!("backend headless pronto ({width}x{height})");
    Ok(Ossido { state, display })
}

pub fn init_winit(
    event_loop: &mut EventLoop<'static, Ossido>,
    mut display: Display<State>,
    (width, height): (i32, i32),
) -> Result<Ossido, Box<dyn std::error::Error>> {
    // La finestra annidata ha la dimensione chiesta in pixel fisici, come l'output headless.
    let attributes = smithay::reexports::winit::window::Window::default_attributes()
        .with_inner_size(smithay::reexports::winit::dpi::PhysicalSize::new(
            width.max(1).unsigned_abs(),
            height.max(1).unsigned_abs(),
        ))
        .with_title("Ossido")
        .with_visible(true);
    let (graphics, winit_loop) = winit::init_from_attributes::<GlesRenderer>(attributes)?;
    let mode = Mode {
        size: graphics.window_size(),
        refresh: NESTED_REFRESH_MHZ,
    };
    let output = new_output("winit", "Ossido annidato", mode, Transform::Flipped180);
    let damage_tracker = OutputDamageTracker::from_output(&output);

    let mut state = State::new(
        &mut display,
        event_loop.handle(),
        event_loop.get_signal(),
        output,
        Backend::Winit(Box::new(WinitBackend {
            graphics,
            damage_tracker,
        })),
    )?;
    if let Backend::Winit(backend) = &mut state.backend {
        state.software_renderer = is_software(backend.graphics.renderer());
        let dh = display.handle();
        state.dmabuf_global = Some(crate::dmabuf::create_global(
            &mut state.dmabuf_state,
            &dh,
            backend.graphics.renderer(),
        ));
    }

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
        let glass_quality = self.glass_quality();
        self.advance_spaces(Instant::now());
        let space_shift = self.space_shift();
        match &mut self.backend {
            Backend::Winit(backend) => {
                let renderer = backend.graphics.renderer();
                let context = RenderContext {
                    titlebar_hover: self.titlebar_hover.as_ref(),
                    clear_color: self.clear_color,
                    animation_speed: self.animation_speed,
                    closing: &self.closing,
                    now: Instant::now(),
                    glass_quality,
                    space_shift,
                };
                let elements = output_elements(renderer, &self.space, &self.output, &context);
                capture_elements::<_, GlesTexture, _>(
                    renderer,
                    &self.output,
                    &elements,
                    self.clear_color,
                )
            }
            Backend::Headless(Headless::Gles(renderer)) => {
                let context = RenderContext {
                    titlebar_hover: self.titlebar_hover.as_ref(),
                    clear_color: self.clear_color,
                    animation_speed: self.animation_speed,
                    closing: &self.closing,
                    now: Instant::now(),
                    glass_quality,
                    space_shift,
                };
                let elements = output_elements(renderer, &self.space, &self.output, &context);
                capture_elements::<_, GlesTexture, _>(
                    renderer.as_mut(),
                    &self.output,
                    &elements,
                    self.clear_color,
                )
            }
            Backend::Headless(Headless::Pixman(renderer)) => {
                capture::<_, pixman::Image<'static, 'static>>(
                    renderer,
                    &self.output,
                    &self.space,
                    self.clear_color,
                )
            }
        }
    }

    fn render_winit(&mut self) {
        let now = Instant::now();
        let speed = self.animation_speed;
        let glass_quality = self.glass_quality();
        self.closing.retain(|closing| !closing.finished(now, speed));
        self.advance_spaces(now);
        let space_shift = self.space_shift();
        let Backend::Winit(backend) = &mut self.backend else {
            return;
        };
        let size = backend.graphics.window_size();
        let damage = Rectangle::from_size(size);
        let clear_color = self.clear_color;
        let context = RenderContext {
            titlebar_hover: self.titlebar_hover.as_ref(),
            clear_color,
            animation_speed: speed,
            closing: &self.closing,
            now,
            glass_quality,
            space_shift,
        };
        let elements = output_elements(
            backend.graphics.renderer(),
            &self.space,
            &self.output,
            &context,
        );
        let rendered = match backend.graphics.bind() {
            Ok((renderer, mut framebuffer)) => backend
                .damage_tracker
                .render_output(renderer, &mut framebuffer, 0, &elements, clear_color)
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
        self.perf.rendered(now.elapsed());
        self.send_frames();
    }
}

impl State {
    /// L'istantanea della finestra che si chiude, per farla svanire (solo con GLES).
    pub fn start_closing(&mut self, window: &smithay::desktop::Window) {
        if self.animation_speed <= 0.0 {
            return;
        }
        let renderer = match &mut self.backend {
            Backend::Winit(backend) => backend.graphics.renderer(),
            Backend::Headless(Headless::Gles(renderer)) => renderer.as_mut(),
            Backend::Headless(Headless::Pixman(_)) => return,
        };
        let now = Instant::now();
        let speed = self.animation_speed;
        self.closing.retain(|closing| !closing.finished(now, speed));
        if let Some(closing) =
            crate::render::snapshot_window(renderer, &self.space, &self.output, window)
        {
            self.closing.push(closing);
        }
    }
}

/// Il renderer gira su un rasterizzatore software (EGL_MESA_device_software).
fn is_software(renderer: &GlesRenderer) -> bool {
    smithay::backend::egl::EGLDevice::device_for_display(renderer.egl_context().display())
        .is_ok_and(|device| device.is_software())
}
