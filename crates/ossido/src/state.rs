//! Stato del compositor: protocolli Wayland, spazio delle finestre, seat.

use std::ffi::OsString;
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType};
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, LoopSignal, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use tracing::{info, warn};

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
    pub seat: Seat<State>,

    /// Numero di finestre aperte finora, per disporle a cascata.
    pub mapped: u32,
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
            if let Err(err) = data
                .state
                .display_handle
                .insert_client(stream, Arc::new(ClientState::default()))
            {
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
            seat,
            mapped: 0,
        })
    }

    /// La superficie sotto un punto del desktop, con la sua origine.
    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space
            .element_under(pos)
            .and_then(|(window, location)| {
                window
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(surface, offset)| (surface, (offset + location).to_f64()))
            })
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
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
