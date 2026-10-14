//! Wayland protocol handlers: compositor, xdg-shell, shm, seat, output.

use std::os::fd::OwnedFd;

use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{PopupKind, Window};
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::{Logical, Point, Serial};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    BufferAssignment, CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes,
    get_parent, is_sync_subsurface, with_states,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
    set_data_device_focus,
};
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    delegate_viewporter, delegate_xdg_shell,
};
use tracing::{info, warn};

use crate::state::{ClientState, State};

/// Distance between a window and the next one in the cascade, in logical pixels.
const CASCADE_STEP: i32 = 32;
const CASCADE_SLOTS: u32 = 8;

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        match client.get_data::<ClientState>() {
            Some(data) => &data.compositor_state,
            // Every client is inserted with ClientState (state.rs): it does not happen.
            None => &FALLBACK_CLIENT_STATE,
        }
    }

    fn commit(&mut self, surface: &WlSurface) {
        // It must be read before the buffer handler, which consumes the buffer just assigned.
        let new_buffer = with_states(surface, |states| {
            matches!(
                states
                    .cached_state
                    .get::<SurfaceAttributes>()
                    .current()
                    .buffer,
                Some(BufferAssignment::NewBuffer(_))
            )
        });
        if new_buffer {
            let pid = surface
                .client()
                .and_then(|client| client.get_credentials(&self.display_handle).ok())
                .and_then(|credentials| u32::try_from(credentials.pid).ok());
            self.perf.frame(surface, pid, std::time::Instant::now());
        }
        on_commit_buffer_handler::<Self>(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self.window_for(&root) {
                window.on_commit();
            }
        }

        if let Some(toplevel) = self.window_for(surface).and_then(|w| w.toplevel().cloned())
            && !toplevel.is_initial_configure_sent()
        {
            toplevel.send_configure();
        }
        // With Ossido's bar the frame grows upwards: it stays in the usable area.
        if let Some(window) = self.window_for(surface)
            && crate::decorations::is_server_side(&window)
        {
            self.make_room_for_titlebar(&window);
        }

        self.layer_commit(surface);

        self.popups.commit(surface);
        if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface)
            && !popup.is_initial_configure_sent()
            && let Err(err) = popup.send_configure()
        {
            warn!("popup configure failed: {err}");
        }
    }
}

static FALLBACK_CLIENT_STATE: std::sync::LazyLock<CompositorClientState> =
    std::sync::LazyLock::new(CompositorClientState::default);

impl State {
    fn window_for(&self, surface: &WlSurface) -> Option<Window> {
        self.space
            .elements()
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
            .cloned()
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Activated);
        });
        let slot = i32::try_from(self.mapped % CASCADE_SLOTS).unwrap_or(0);
        let offset = CASCADE_STEP.saturating_mul(slot.saturating_add(1));
        self.mapped = self.mapped.wrapping_add(1);

        let window = Window::new_wayland_window(surface);
        let id = self.assign_window_id(&window);
        self.publish_toplevel(&window);
        info!("new window {id} at ({offset}, {offset})");
        self.space.map_element(
            window.clone(),
            self.usable_area().loc + Point::<i32, Logical>::from((offset, offset)),
            true,
        );
        self.focus_window(Some(window));
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
        });
        if let Err(err) = self.popups.track_popup(PopupKind::from(surface)) {
            warn!("popup not tracked: {err}");
        }
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        // The window fades out from its last image: the surface still has the buffer.
        if let Some(window) = self.window_for(surface.wl_surface()) {
            self.start_closing(&window);
        }
        self.unpublish_toplevel(&surface);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        self.update_toplevel(&surface);
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        self.update_toplevel(&surface);
    }

    /// The app moves the window from its toolbar (full-size content): like dragging Ossido's
    /// bar, only with the button still pressed.
    fn move_request(&mut self, surface: ToplevelSurface, _seat: WlSeat, serial: Serial) {
        if self.press_serial != Some(serial) || self.moving.is_some() {
            return;
        }
        let Some(window) = self.window_for(surface.wl_surface()) else {
            return;
        };
        let (Some(pointer), Some(frame)) = (self.seat.get_pointer(), self.window_frame(&window))
        else {
            return;
        };
        let grab = pointer.current_location() - frame.loc.to_f64();
        self.moving = Some((window, grab));
    }

    /// The client resizes the window from one of its edges (client-side decorations), with
    /// the button still pressed.
    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        _seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        if self.press_serial != Some(serial) || self.resizing.is_some() || self.moving.is_some() {
            return;
        }
        let edges = crate::resize::Edges::from_xdg(edges);
        let (Some(window), Some(pointer)) = (
            self.window_for(surface.wl_surface()),
            self.seat.get_pointer(),
        ) else {
            return;
        };
        if edges.any() {
            self.start_resize(window, edges, pointer.current_location());
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        // Popup grabs come with the menus (M3); for now the popup stays open.
    }
}

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let client = focused.and_then(|s| self.display_handle.get_client(s.id()).ok());
        set_data_device_focus(&self.display_handle, seat, client);
        self.announce_focus(focused);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }
}

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for State {
    fn started(
        &mut self,
        _source: Option<WlDataSource>,
        icon: Option<WlSurface>,
        _seat: Seat<Self>,
    ) {
        self.drag_icon = icon;
    }

    fn dropped(&mut self, _target: Option<WlSurface>, _validated: bool, _seat: Seat<Self>) {
        self.drag_icon = None;
    }
}

impl ServerDndGrabHandler for State {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {}
}

impl OutputHandler for State {}

delegate_compositor!(State);
delegate_shm!(State);
delegate_xdg_shell!(State);
delegate_seat!(State);
delegate_data_device!(State);
delegate_output!(State);
delegate_viewporter!(State);
