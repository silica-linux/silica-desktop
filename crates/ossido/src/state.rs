//! Stato del compositor: protocolli Wayland, spazio delle finestre, seat.

use std::ffi::OsString;
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType, layer_map_for_output};
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, LoopSignal, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::wlr_layer::{Layer, WlrLayerShellState};
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::viewporter::ViewporterState;
use tracing::{info, warn};

use silica_control::Shortcut;

use crate::backend::Backend;

/// Colore di fondo del desktop finché non c'è uno sfondo (RGBA lineare).
pub const CLEAR_COLOR: [f32; 4] = [0.11, 0.12, 0.16, 1.0];

/// Dati del ciclo di eventi: lo stato e il `Display` che lo serve.
pub struct Ossido {
    pub state: State,
    pub display: Display<State>,
}

pub struct State {
    pub start_time: Instant,
    pub socket_name: OsString,
    /// Indirizzo del bus di sessione privato (`--session-bus`), passato ai client.
    pub session_bus: Option<String>,
    /// I comandi `system.*` verso silica-daemon.
    pub system: crate::system::SystemLink,
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Ossido>,
    pub loop_signal: LoopSignal,

    pub space: Space<Window>,
    pub popups: PopupManager,
    pub output: Output,
    pub backend: Backend,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<State>,
    pub data_device_state: DataDeviceState,
    pub layer_shell_state: WlrLayerShellState,
    /// wp_viewporter: superfici scalate o ritagliate (iced lo richiede per l'HiDPI).
    pub viewporter_state: ViewporterState,
    /// linux-dmabuf: buffer dalla GPU dei client (solo winit, vedi `dmabuf.rs`).
    pub dmabuf_state: DmabufState,
    pub dmabuf_global: Option<DmabufGlobal>,
    pub xdg_decoration_state: XdgDecorationState,
    pub glass_manager_state: crate::glass::GlassManagerState,
    /// Le finestre per la shell (`ext-foreign-toplevel-list`, solo ai suoi componenti).
    pub foreign_toplevel_list: smithay::wayland::foreign_toplevel_list::ForeignToplevelListState,
    pub shell_manager_state: crate::shell::ShellManagerState,
    /// I componenti della shell collegati a `silica_shell_manager_v1`.
    pub shell_managers:
        Vec<silica_protocols::shell::server::silica_shell_manager_v1::SilicaShellManagerV1>,
    /// L'ultima finestra con il focus annunciata alla shell.
    pub announced_focus: Option<String>,
    pub seat: Seat<State>,

    /// Numero di finestre aperte finora, per disporle a cascata.
    pub mapped: u32,
    /// Ultimo id assegnato a una finestra.
    pub next_window_id: u64,
    pub shortcuts: Vec<(Shortcut, &'static str)>,
    /// Keycode per i tasti sintetici (`input.key`, `input.type`).
    pub keys: crate::keys::KeyLookup,
    /// Ritmo dei frame, avvio e memoria dei client (`perf.*`).
    pub perf: crate::perf::Perf,
    /// Colore del desktop finché non c'è uno sfondo (`--background`).
    pub clear_color: [f32; 4],
    /// Finestre ridotte, con la posizione a cui tornano.
    pub minimized: Vec<crate::windows::Minimized>,
    /// Gli spazi di lavoro e lo scorrimento in corso (`workspaces.rs`).
    pub workspaces: crate::workspaces::Workspaces,
    /// Panorama aperto o in chiusura (`panorama.rs`).
    pub panorama: Option<crate::panorama::Panorama>,
    /// Il clic che ha aperto o usato Panorama: il rilascio non va al client.
    pub panorama_press: bool,
    /// Finestra con il puntatore sul gruppo dei pulsanti della barra.
    pub titlebar_hover: Option<Window>,
    /// Pulsante della barra premuto: l'azione parte al rilascio sullo stesso pulsante.
    pub titlebar_press: Option<(Window, ossido_render::titlebar::Part)>,
    /// Finestra trascinata per la barra, con il punto di presa relativo alla cornice.
    pub moving: Option<(Window, Point<f64, Logical>)>,
    /// Moltiplicatore delle durate delle animazioni; 0 le spegne (`--animation-scale`,
    /// `ui.animation_scale`).
    pub animation_speed: f64,
    /// Finestre chiuse che stanno svanendo.
    pub closing: Vec<crate::render::Closing>,
    /// Qualità del vetro scelta (`--glass`, `ui.glass_quality`).
    pub glass_setting: GlassSetting,
    /// Il renderer gira su un rasterizzatore software (llvmpipe): il vetro automatico è
    /// smerigliato.
    pub software_renderer: bool,
}

/// Qualità del vetro chiesta: una fissa o la scelta automatica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlassSetting {
    #[default]
    Auto,
    Fixed(ossido_render::glass::GlassQuality),
}

impl GlassSetting {
    pub fn parse(text: &str) -> Option<Self> {
        use ossido_render::glass::GlassQuality;
        Some(match text {
            "auto" => GlassSetting::Auto,
            "full" => GlassSetting::Fixed(GlassQuality::Full),
            "frosted" => GlassSetting::Fixed(GlassQuality::Frosted),
            "opaque" => GlassSetting::Fixed(GlassQuality::Opaque),
            _ => return None,
        })
    }
}

impl State {
    /// La qualità del vetro in uso.
    pub fn glass_quality(&self) -> ossido_render::glass::GlassQuality {
        use ossido_render::glass::GlassQuality;
        match self.glass_setting {
            GlassSetting::Fixed(quality) => quality,
            GlassSetting::Auto if self.software_renderer => GlassQuality::Frosted,
            GlassSetting::Auto => GlassQuality::Full,
        }
    }
}

impl State {
    /// Crea lo stato, apre il socket Wayland e registra il `Display` nel ciclo.
    pub fn new(
        display: &mut Display<State>,
        loop_handle: LoopHandle<'static, Ossido>,
        loop_signal: LoopSignal,
        output: Output,
        backend: Backend,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let dh = display.handle();
        let compositor_state = CompositorState::new::<State>(&dh);
        let xdg_shell_state = XdgShellState::new::<State>(&dh);
        let shm_state = ShmState::new::<State>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<State>(&dh);
        let data_device_state = DataDeviceState::new::<State>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<State>(&dh);
        let viewporter_state = ViewporterState::new::<State>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<State>(&dh);
        let glass_manager_state = crate::glass::GlassManagerState::new(&dh);
        let foreign_toplevel_list = crate::shell::toplevel_list(&dh);
        let shell_manager_state = crate::shell::ShellManagerState::new(&dh);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "seat0");
        seat.add_keyboard(Default::default(), 200, 25)?;
        seat.add_pointer();

        let _global = output.create_global::<State>(&dh);
        let mut space = Space::default();
        space.map_output(&output, (0, 0));

        let socket = ListeningSocketSource::new_auto()?;
        let socket_name = socket.socket_name().to_os_string();
        loop_handle.insert_source(socket, |stream, _, data| {
            // Il processo dall'altra parte del socket dice se è un componente della shell:
            // si legge qui, una volta, perché i filtri dei global non possono interrogare il
            // display mentre è bloccato dal dispatch.
            let shell = crate::shell::stream_is_shell(&stream);
            if let Err(err) = data.state.display_handle.insert_client(
                stream,
                Arc::new(ClientState {
                    shell,
                    ..ClientState::default()
                }),
            ) {
                warn!("client Wayland rifiutato: {err}");
            }
        })?;

        let poll_fd = display.backend().poll_fd().try_clone_to_owned()?;
        loop_handle.insert_source(
            Generic::new(poll_fd, Interest::READ, Mode::Level),
            |_, _, data| {
                if let Err(err) = data.display.dispatch_clients(&mut data.state) {
                    warn!("dispatch dei client fallito: {err}");
                }
                Ok(PostAction::Continue)
            },
        )?;
        info!("socket Wayland: {}", socket_name.to_string_lossy());

        Ok(Self {
            start_time: Instant::now(),
            socket_name,
            session_bus: None,
            system: crate::system::SystemLink::disconnected(),
            display_handle: dh,
            loop_handle,
            loop_signal,
            space,
            popups: PopupManager::default(),
            output,
            backend,
            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            layer_shell_state,
            viewporter_state,
            dmabuf_state: DmabufState::new(),
            dmabuf_global: None,
            xdg_decoration_state,
            glass_manager_state,
            foreign_toplevel_list,
            shell_manager_state,
            shell_managers: Vec::new(),
            announced_focus: None,
            seat,
            mapped: 0,
            next_window_id: 0,
            shortcuts: crate::commands::shortcuts(),
            keys: crate::keys::KeyLookup::new()?,
            perf: crate::perf::Perf::default(),
            clear_color: CLEAR_COLOR,
            minimized: Vec::new(),
            workspaces: crate::workspaces::Workspaces::default(),
            panorama: None,
            panorama_press: false,
            titlebar_hover: None,
            titlebar_press: None,
            moving: None,
            animation_speed: 1.0,
            closing: Vec::new(),
            glass_setting: GlassSetting::Auto,
            software_renderer: false,
        })
    }

    /// La superficie sotto un punto del desktop, con la sua origine: prima i layer
    /// sopra le finestre, poi le finestre, poi i layer sotto.
    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        let in_layer = |levels: &[Layer]| {
            self.layer_under(levels, pos).and_then(|(layer, location)| {
                layer
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(surface, offset)| (surface, (offset + location).to_f64()))
            })
        };
        in_layer(&[Layer::Overlay, Layer::Top])
            .or_else(|| {
                self.space
                    .element_under(pos)
                    .and_then(|(window, location)| {
                        window
                            .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                            .map(|(surface, offset)| (surface, (offset + location).to_f64()))
                    })
            })
            .or_else(|| in_layer(&[Layer::Bottom, Layer::Background]))
    }

    /// Manda i frame callback a tutte le finestre visibili.
    pub fn send_frames(&self) {
        let output = &self.output;
        let now = self.start_time.elapsed();
        for window in self.space.elements() {
            window.send_frame(output, now, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
        for layer in layer_map_for_output(output).layers() {
            layer.send_frame(output, now, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    /// Componente della shell (`shell.rs`): vede i protocolli della shell.
    pub shell: bool,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
