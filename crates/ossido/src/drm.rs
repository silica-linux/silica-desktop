//! The real-session backend: Ossido on a TTY (or in a virtual machine) without a host
//! compositor. libseat opens the devices and switches VT, udev finds the GPU and its
//! connectors, DRM/KMS scans out GBM buffers drawn by the same GLES pipeline as the other
//! backends (`ossido-render`: glass, shadows, corners), libinput feeds the input paths.
//!
//! One output for now: the first connected connector with its preferred mode (`State` has
//! one `Output`). When connectors change (hotplug), the output moves to the first connected
//! one; with none connected nothing is drawn until one comes back.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent, GbmBufferedSurface};
use smithay::backend::egl::EGLDevice;
use smithay::backend::input::InputEvent;
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, ImportMem};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{UdevBackend, UdevEvent, all_gpus, primary_gpu};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::drm::control::{Device as ControlDevice, ModeTypeFlags, connector, crtc};
use smithay::reexports::input::Libinput;
use smithay::reexports::rustix::fs::OFlags;
use smithay::reexports::wayland_server::Display;
use smithay::utils::{DeviceFd, Transform};
use tracing::{info, warn};

use crate::backend::Backend;
use crate::state::{Ossido, State};

/// When no page flip is pending, the interval at which the loop checks for a new frame
/// (animations, frame callbacks): a frame with no damage is not queued and brings no vblank.
const IDLE_FRAME: Duration = Duration::from_micros(16_667);

pub struct DrmBackend {
    pub renderer: GlesRenderer,
    session: LibSeatSession,
    drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    /// The `dev_t` of the GPU, to recognise its udev events.
    device_id: u64,
    libinput: Libinput,
    surface: Option<OutputSurface>,
    /// A frame is queued for scanout and waits for its vblank.
    pending: bool,
    /// The default cursor, drawn when no client sets one and there is no theme.
    cursor: Option<GlesTexture>,
    /// The frames of the theme already uploaded, by name, scale (×100) and frame, with their
    /// hotspot and the element id the damage tracker follows.
    cursors: std::collections::HashMap<
        crate::cursor_theme::FrameKey,
        (
            GlesTexture,
            (i32, i32),
            smithay::backend::renderer::element::Id,
        ),
    >,
}

/// The connector driven by the output, with its swapchain.
struct OutputSurface {
    connector: connector::Handle,
    crtc: crtc::Handle,
    mode: smithay::reexports::drm::control::Mode,
    gbm: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    damage_tracker: OutputDamageTracker,
}

/// A connected connector with the crtc that can drive it and the mode to use.
#[derive(Debug, Clone, PartialEq)]
struct Choice {
    connector: connector::Handle,
    crtc: crtc::Handle,
    mode: smithay::reexports::drm::control::Mode,
    name: String,
    size_mm: (i32, i32),
}

/// The GPU of the seat: the primary one (the boot VGA device), otherwise the first one.
fn gpu_path(seat: &str) -> Result<PathBuf, String> {
    if let Ok(Some(path)) = primary_gpu(seat) {
        return Ok(path);
    }
    all_gpus(seat)
        .map_err(|e| format!("udev: {e}"))?
        .into_iter()
        .next()
        .ok_or_else(|| format!("no GPU on seat {seat}"))
}

/// The preferred mode: the one flagged PREFERRED, otherwise the first (the largest).
fn preferred_mode(
    modes: &[smithay::reexports::drm::control::Mode],
) -> Option<smithay::reexports::drm::control::Mode> {
    modes
        .iter()
        .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| modes.first())
        .copied()
}

/// The first connected connector with a mode and a usable crtc.
fn choose(drm: &DrmDevice) -> Option<Choice> {
    let device = drm.device_fd();
    let resources = device.resource_handles().ok()?;
    for &handle in resources.connectors() {
        let Ok(info) = device.get_connector(handle, false) else {
            continue;
        };
        if info.state() != connector::State::Connected {
            continue;
        }
        let Some(mode) = preferred_mode(info.modes()) else {
            continue;
        };
        let crtc = info
            .encoders()
            .iter()
            .filter_map(|&encoder| device.get_encoder(encoder).ok())
            .flat_map(|encoder| resources.filter_crtcs(encoder.possible_crtcs()))
            .find(|crtc| drm.crtcs().contains(crtc));
        let Some(crtc) = crtc else {
            continue;
        };
        let (w, h) = info.size().unwrap_or((0, 0));
        return Some(Choice {
            connector: handle,
            crtc,
            mode,
            name: format!("{}-{}", info.interface().as_str(), info.interface_id()),
            size_mm: (
                i32::try_from(w).unwrap_or_default(),
                i32::try_from(h).unwrap_or_default(),
            ),
        });
    }
    None
}

fn create_surface(
    drm: &mut DrmDevice,
    gbm: &GbmDevice<DrmDeviceFd>,
    renderer: &GlesRenderer,
    output: &Output,
    choice: &Choice,
) -> Result<OutputSurface, String> {
    let surface = drm
        .create_surface(choice.crtc, choice.mode, &[choice.connector])
        .map_err(|e| format!("DRM surface on {}: {e}", choice.name))?;
    let allocator = GbmAllocator::new(
        gbm.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );
    let formats = renderer.egl_context().dmabuf_render_formats().clone();
    let gbm = GbmBufferedSurface::new(
        surface,
        allocator,
        &[Fourcc::Argb8888, Fourcc::Xrgb8888],
        formats,
    )
    .map_err(|e| format!("GBM swapchain on {}: {e}", choice.name))?;
    Ok(OutputSurface {
        connector: choice.connector,
        crtc: choice.crtc,
        mode: choice.mode,
        gbm,
        damage_tracker: OutputDamageTracker::from_output(output),
    })
}

pub fn init_drm(
    event_loop: &mut EventLoop<'static, Ossido>,
    mut display: Display<State>,
) -> Result<Ossido, Box<dyn std::error::Error>> {
    let (mut session, session_events) =
        LibSeatSession::new().map_err(|e| format!("libseat session: {e}"))?;
    let seat = session.seat();
    let path = gpu_path(&seat)?;
    let fd = session
        .open(
            &path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (mut drm, drm_events) =
        DrmDevice::new(fd.clone(), true).map_err(|e| format!("DRM device: {e}"))?;
    let gbm = GbmDevice::new(fd).map_err(|e| format!("GBM device: {e}"))?;
    let renderer = crate::egl::gbm_renderer(gbm.clone())?;
    let software = EGLDevice::device_for_display(renderer.egl_context().display())
        .is_ok_and(|device| device.is_software());
    info!(
        "DRM on {} (seat {seat}, software rendering: {software})",
        path.display()
    );

    let choice = choose(&drm);
    let (mode, name, size_mm) = match &choice {
        Some(choice) => (Mode::from(choice.mode), choice.name.clone(), choice.size_mm),
        None => {
            warn!("no connected output yet");
            (
                Mode {
                    size: (1280, 800).into(),
                    refresh: 60_000,
                },
                "drm".to_owned(),
                (0, 0),
            )
        }
    };
    let output = Output::new(
        name.clone(),
        PhysicalProperties {
            size: size_mm.into(),
            subpixel: Subpixel::Unknown,
            make: "Silica".to_owned(),
            model: name,
        },
    );
    output.change_current_state(
        Some(mode),
        Some(Transform::Normal),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
    let surface = match &choice {
        Some(choice) => Some(create_surface(&mut drm, &gbm, &renderer, &output, choice)?),
        None => None,
    };

    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    if libinput.udev_assign_seat(&seat).is_err() {
        return Err(format!("libinput: cannot assign seat {seat}").into());
    }
    let device_id = drm.device_id();
    let backend = DrmBackend {
        renderer,
        session,
        drm,
        gbm,
        device_id,
        libinput: libinput.clone(),
        surface,
        pending: false,
        cursor: None,
        cursors: std::collections::HashMap::new(),
    };
    let mut state = State::new(
        &mut display,
        event_loop.handle(),
        event_loop.get_signal(),
        output,
        Backend::Drm(Box::new(backend)),
    )?;
    state.software_renderer = software;
    if let Backend::Drm(backend) = &state.backend {
        let dh = display.handle();
        state.dmabuf_global = Some(crate::dmabuf::create_global(
            &mut state.dmabuf_state,
            &dh,
            &backend.renderer,
        ));
    }

    let handle = event_loop.handle();
    handle.insert_source(session_events, |event, _, data| match event {
        SessionEvent::PauseSession => data.state.drm_pause(),
        SessionEvent::ActivateSession => data.state.drm_activate(),
    })?;
    handle.insert_source(drm_events, |event, _, data| match event {
        DrmEvent::VBlank(crtc) => data.state.drm_vblank(crtc),
        DrmEvent::Error(err) => warn!("DRM: {err}"),
    })?;
    let udev = UdevBackend::new(&seat).map_err(|e| format!("udev: {e}"))?;
    handle.insert_source(udev, |event, _, data| {
        if let UdevEvent::Changed { device_id } = event {
            data.state.drm_changed(device_id);
        }
    })?;
    handle.insert_source(LibinputInputBackend::new(libinput), |event, _, data| {
        if let InputEvent::DeviceAdded { device } = &event {
            info!("input device: {}", device.name());
        }
        data.state.process_input_event(event);
    })?;
    handle.insert_source(Timer::from_duration(IDLE_FRAME), |_, _, data| {
        data.state.render_drm();
        TimeoutAction::ToDuration(IDLE_FRAME)
    })?;
    info!("drm backend ready");
    Ok(Ossido { state, display })
}

impl State {
    fn drm_backend(&mut self) -> Option<&mut DrmBackend> {
        match &mut self.backend {
            Backend::Drm(backend) => Some(backend),
            _ => None,
        }
    }

    /// The session goes away (VT switch): stop drawing and reading input.
    fn drm_pause(&mut self) {
        let Some(backend) = self.drm_backend() else {
            return;
        };
        info!("session paused");
        backend.libinput.suspend();
        backend.drm.pause();
        backend.pending = false;
    }

    /// The session is back: the device state may have changed, so the buffers start over.
    fn drm_activate(&mut self) {
        let Some(backend) = self.drm_backend() else {
            return;
        };
        info!("session active");
        if backend.libinput.resume().is_err() {
            warn!("libinput: resume failed");
        }
        if let Err(err) = backend.drm.activate(false) {
            warn!("DRM: activate failed: {err}");
        }
        if let Some(surface) = &mut backend.surface {
            surface.gbm.reset_buffers();
        }
        backend.pending = false;
        self.drm_changed_inner();
        self.render_drm();
    }

    /// The previous frame is on screen: the next one can be queued.
    fn drm_vblank(&mut self, crtc: crtc::Handle) {
        let Some(backend) = self.drm_backend() else {
            return;
        };
        let Some(surface) = &mut backend.surface else {
            return;
        };
        if surface.crtc != crtc {
            return;
        }
        if let Err(err) = surface.gbm.frame_submitted() {
            warn!("DRM: frame submitted: {err}");
        }
        backend.pending = false;
        self.render_drm();
    }

    /// A udev event: if it is our GPU, its connectors may have changed.
    fn drm_changed(&mut self, device_id: u64) {
        if self.drm_backend().is_some_and(|b| b.device_id == device_id) {
            self.drm_changed_inner();
        }
    }

    /// Follows the connectors: the output moves to the first connected one.
    fn drm_changed_inner(&mut self) {
        let output = self.output.clone();
        let Some(backend) = self.drm_backend() else {
            return;
        };
        let choice = choose(&backend.drm);
        let current = backend
            .surface
            .as_ref()
            .map(|s| (s.connector, s.crtc, s.mode));
        if choice.as_ref().map(|c| (c.connector, c.crtc, c.mode)) == current {
            return;
        }
        backend.surface = None;
        backend.pending = false;
        let Some(choice) = choice else {
            info!("no connected output");
            return;
        };
        info!(
            "output {} {}x{}",
            choice.name,
            choice.mode.size().0,
            choice.mode.size().1
        );
        let mode = Mode::from(choice.mode);
        output.change_current_state(Some(mode), None, None, None);
        output.set_preferred(mode);
        match create_surface(
            &mut backend.drm,
            &backend.gbm,
            &backend.renderer,
            &output,
            &choice,
        ) {
            Ok(surface) => backend.surface = Some(surface),
            Err(err) => warn!("{err}"),
        }
    }

    /// Switches to another virtual terminal.
    pub fn drm_change_vt(&mut self, vt: i32) {
        if let Some(backend) = self.drm_backend()
            && let Err(err) = backend.session.change_vt(vt)
        {
            warn!("VT {vt}: {err}");
        }
    }

    /// Draws a frame and queues it for scanout, unless one is already waiting for its
    /// vblank or the session is not active.
    pub fn render_drm(&mut self) {
        let now = Instant::now();
        let ready = self.drm_backend().is_some_and(|backend| {
            !backend.pending && backend.surface.is_some() && backend.session.is_active()
        });
        if !ready {
            // Clients still get their frame callbacks, at the idle rate.
            self.send_frames();
            return;
        }
        // The same frame preparation as the winit backend (`render_winit`).
        let speed = self.animation_speed;
        let glass_quality = self.glass_quality();
        self.closing.retain(|closing| !closing.finished(now, speed));
        self.genies.retain(|genie| !genie.finished(now, speed));
        self.advance_spaces(now);
        self.advance_panorama(now);
        let space_shift = self.space_shift();
        let panorama = self.panorama_view();
        let spaces = (self.workspaces.count(), self.workspaces.active);
        let buttons = self.buttons_view();
        let alt = self.alt_held();
        let dark = self.dark();
        let drag_icon = self.drag_icon_view();
        let window_veil = self.window_veil_view();
        let pointer = self
            .seat
            .get_pointer()
            .map(|pointer| pointer.current_location());
        let cursor_status = self.effective_cursor();
        let scale = self.output.current_scale().fractional_scale();
        let themed = match &cursor_status {
            smithay::input::pointer::CursorImageStatus::Named(icon) => {
                self.cursor_theme.image(*icon, scale, now)
            }
            _ => None,
        };
        let Backend::Drm(backend) = &mut self.backend else {
            return;
        };
        let clear_color = self.clear_color;
        let context = crate::render::RenderContext {
            buttons,
            alt,
            clear_color,
            animation_speed: speed,
            closing: &self.closing,
            genies: &self.genies,
            now,
            glass_quality,
            space_shift,
            panorama,
            spaces,
            dark,
            drag_icon,
            window_veil,
        };
        let mut elements = crate::render::output_elements(
            &mut backend.renderer,
            &self.space,
            &self.output,
            &context,
        );
        if backend.cursor.is_none() {
            backend.cursor = default_cursor(&mut backend.renderer);
        }
        if let Some((key, image)) = &themed
            && !backend.cursors.contains_key(key)
        {
            match backend.renderer.import_memory(
                &image.rgba,
                Fourcc::Abgr8888,
                (image.width as i32, image.height as i32).into(),
                false,
            ) {
                Ok(texture) => {
                    backend.cursors.insert(
                        key.clone(),
                        (
                            texture,
                            image.hotspot,
                            smithay::backend::renderer::element::Id::new(),
                        ),
                    );
                }
                Err(err) => warn!("cursor {}: {err}", key.0),
            }
        }
        if let Some(location) = pointer {
            let origin = self
                .space
                .output_geometry(&self.output)
                .map(|g| g.loc)
                .unwrap_or_default();
            let named = themed
                .as_ref()
                .and_then(|(key, _)| backend.cursors.get(key))
                .map(|(texture, hotspot, id)| (texture, *hotspot, Some(id.clone())))
                .or(backend
                    .cursor
                    .as_ref()
                    .map(|texture| (texture, (0, 0), None)));
            let cursor = crate::cursor::elements(
                &mut backend.renderer,
                &cursor_status,
                named,
                location - origin.to_f64(),
                scale,
            );
            elements.splice(0..0, cursor);
        }
        let Some(surface) = &mut backend.surface else {
            return;
        };
        let queued = (|| -> Result<bool, String> {
            let (mut dmabuf, age) = surface
                .gbm
                .next_buffer()
                .map_err(|e| format!("next buffer: {e}"))?;
            let mut framebuffer = backend
                .renderer
                .bind(&mut dmabuf)
                .map_err(|e| format!("bind: {e}"))?;
            let result = surface
                .damage_tracker
                .render_output(
                    &mut backend.renderer,
                    &mut framebuffer,
                    usize::from(age),
                    &elements,
                    clear_color,
                )
                .map_err(|e| format!("render: {e:?}"))?;
            let Some(damage) = result.damage.cloned() else {
                return Ok(false);
            };
            surface
                .gbm
                .queue_buffer(Some(result.sync), Some(damage), ())
                .map_err(|e| format!("queue: {e}"))?;
            Ok(true)
        })();
        match queued {
            Ok(queued) => backend.pending = queued,
            Err(err) => warn!("DRM frame: {err}"),
        }
        self.perf.rendered(now.elapsed());
        self.send_frames();
    }
}

/// The default cursor as a texture, generated in code (see `cursor.rs`).
fn default_cursor(renderer: &mut GlesRenderer) -> Option<GlesTexture> {
    let (w, h, pixels) = crate::cursor::arrow();
    match renderer.import_memory(&pixels, Fourcc::Abgr8888, (w, h).into(), false) {
        Ok(texture) => Some(texture),
        Err(err) => {
            warn!("default cursor: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::preferred_mode;

    #[test]
    fn no_modes_no_preferred_mode() {
        assert_eq!(preferred_mode(&[]), None);
    }
}
