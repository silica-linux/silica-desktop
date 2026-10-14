//! Compositor state: Wayland protocols, window space, seat.

use std::ffi::OsString;
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::desktop::utils::send_frames_surface_tree;
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
use smithay::wayland::selection::ext_data_control::DataControlState;
use smithay::wayland::shell::wlr_layer::{Layer, WlrLayerShellState};
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::viewporter::ViewporterState;
use tracing::{info, warn};

use silica_control::Shortcut;

use crate::backend::Backend;

/// Desktop background color until there is a wallpaper (linear RGBA).
pub const CLEAR_COLOR: [f32; 4] = [0.11, 0.12, 0.16, 1.0];

/// Event loop data: the state and the `Display` it serves.
pub struct Ossido {
    pub state: State,
    pub display: Display<State>,
}

pub struct State {
    pub start_time: Instant,
    pub socket_name: OsString,
    /// Address of the private session bus (`--session-bus`), passed to the clients.
    pub session_bus: Option<String>,
    /// The `system.*` commands towards silica-daemon.
    pub system: crate::system::SystemLink,
    /// The appearance forced with `--appearance` (tests, sessions without the daemon): it wins
    /// over silica-daemon's state.
    pub appearance: Option<bool>,
    /// The icon of a drag between apps (`wl_data_device.start_drag`): it follows the pointer
    /// until the drop.
    pub drag_icon: Option<WlSurface>,
    /// The cursor asked by the focused client: drawn by Ossido only in the real session.
    pub cursor_status: smithay::input::pointer::CursorImageStatus,
    /// The window screenshot mode (K-05): the frame of the window under the pointer is veiled.
    pub window_veil: bool,
    /// A shape Ossido itself imposes (resize edges, window bar), over the client's.
    pub cursor_override: Option<smithay::input::pointer::CursorIcon>,
    /// The cursor shapes (silica-cursors), rasterized on demand.
    pub cursor_theme: crate::cursor_theme::CursorTheme,
    /// wp_cursor_shape_v1: clients ask for a named shape instead of drawing a surface.
    pub cursor_shape_state: smithay::wayland::cursor_shape::CursorShapeManagerState,
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
    /// ext-data-control: the clipboard history in silica-daemon (and only there, see
    /// `clipboard_manager_client`).
    pub data_control_state: DataControlState,
    pub layer_shell_state: WlrLayerShellState,
    /// wp_viewporter: scaled or cropped surfaces (iced needs it for HiDPI).
    pub viewporter_state: ViewporterState,
    /// linux-dmabuf: buffers from the clients' GPU (winit only, see `dmabuf.rs`).
    pub dmabuf_state: DmabufState,
    pub dmabuf_global: Option<DmabufGlobal>,
    pub xdg_decoration_state: XdgDecorationState,
    pub glass_manager_state: crate::glass::GlassManagerState,
    pub window_manager_state: crate::window_appearance::WindowManagerState,
    /// The windows for the shell (`ext-foreign-toplevel-list`, only to its components).
    pub foreign_toplevel_list: smithay::wayland::foreign_toplevel_list::ForeignToplevelListState,
    pub shell_manager_state: crate::shell::ShellManagerState,
    /// The shell components bound to `silica_shell_manager_v1`.
    pub shell_managers:
        Vec<silica_protocols::shell::server::silica_shell_manager_v1::SilicaShellManagerV1>,
    /// The states announced by the shell components (`broadcast`), by topic.
    pub shell_states: std::collections::BTreeMap<String, String>,
    /// The last focused window announced to the shell.
    pub announced_focus: Option<String>,
    pub seat: Seat<State>,

    /// Number of windows opened so far, to cascade them.
    pub mapped: u32,
    /// Last id assigned to a window.
    pub next_window_id: u64,
    pub shortcuts: Vec<(Shortcut, &'static str)>,
    /// Keycodes for the synthetic keys (`input.key`, `input.type`).
    pub keys: crate::keys::KeyLookup,
    /// Frame pace, startup and memory of the clients (`perf.*`).
    pub perf: crate::perf::Perf,
    /// Desktop color until there is a wallpaper (`--background`).
    pub clear_color: [f32; 4],
    /// Minimized windows, with the position they go back to.
    pub minimized: Vec<crate::windows::Minimized>,
    /// Windows hidden by their app (its last window closed, the app still running, #142),
    /// with the position they come back to.
    pub hidden: Vec<crate::windows::Minimized>,
    /// The app that stays frontmost after closing its last window (#142, APP-LC): no window
    /// has the focus, the menu bar shows that app and Cmd+Q quits it. Cleared when a window
    /// takes the focus.
    pub windowless_front: Option<String>,
    /// Where each app's window last closed: its next window opens there (APP-LC).
    pub app_frames: crate::app_frames::AppFrames,
    /// Shell components kept running (`--keep-shell`, `keep.rs`).
    pub keeper: crate::keep::Keeper,
    /// The workspaces and the scroll in progress (`workspaces.rs`).
    pub workspaces: crate::workspaces::Workspaces,
    /// Panorama open or closing (`panorama.rs`).
    pub panorama: Option<crate::panorama::Panorama>,
    /// The click that opened or used Panorama: the release does not go to the client.
    pub panorama_press: bool,
    /// Open menus, panels and grabbed popups (`transient.rs`, #149).
    pub transients: Vec<crate::transient::Transient>,
    /// A press outside a transient was consumed: its release does not go to the client.
    pub transient_press: bool,
    /// The app switcher is open: releasing the Logo key confirms it.
    pub switcher_active: bool,
    /// The key acting as Logo (`--logo-key`).
    pub logo_key: crate::keys::LogoKey,
    /// Trackpad gesture in progress (`gestures.rs`).
    pub gesture: Option<crate::gestures::Gesture>,
    /// Window with the pointer on the bar's button group.
    pub titlebar_hover: Option<Window>,
    /// Bar button pressed: the action starts on release over the same button.
    pub titlebar_press: Option<(Window, ossido_render::titlebar::Part)>,
    /// The pointer is still on the pressed button (leaving it cancels the action).
    pub titlebar_press_inside: bool,
    /// How many button presses there have been: the long-press timer on the green button
    /// acts only if the press is still the one that started it.
    pub titlebar_press_count: u64,
    /// How many screenshots were written (the temporary file name).
    pub screenshot_count: u64,
    /// Button symbols: when the pointer entered the group and when it left it.
    pub buttons_hover: Option<ButtonsHover>,
    /// Window dragged by the bar, with the grab point relative to the frame.
    pub moving: Option<(Window, Point<f64, Logical>)>,
    /// Window resized from an edge (MU-08) or at the client's request.
    pub resizing: Option<crate::resize::Resize>,
    /// The serial of the last mouse button press, until it is released:
    /// `xdg_toplevel.move` applies only with this serial.
    pub press_serial: Option<smithay::utils::Serial>,
    /// Multiplier of the animation durations; 0 turns them off (`--animation-scale`,
    /// `ui.animation_scale`).
    pub animation_speed: f64,
    /// Closed windows fading out.
    pub closing: Vec<crate::render::Closing>,
    /// Windows minimizing into the Dock or coming out of it (genie).
    pub genies: Vec<crate::render::Genie>,
    /// How windows go into the Dock: genie or scale (the Dock's setting, SH-DK-09).
    pub minimize_effect: crate::render::MinimizeEffect,
    /// Chosen glass quality (`--glass`, `ui.glass_quality`).
    pub glass_setting: GlassSetting,
    /// The renderer runs on a software rasterizer (llvmpipe): the automatic glass is
    /// frosted.
    pub software_renderer: bool,
}

/// Requested glass quality: a fixed one or the automatic choice.
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
    /// The dark appearance: the one forced with `--appearance`, otherwise silica-daemon's
    /// (`appearance.dark`, light if unknown). It is read on every frame, so a daemon change
    /// shows on the next frame.
    pub fn dark(&self) -> bool {
        self.appearance.unwrap_or_else(|| self.system.dark())
    }
}

impl State {
    /// The glass quality in use.
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
    /// Creates the state, opens the Wayland socket and registers the `Display` in the loop.
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
        let data_control_state =
            DataControlState::new::<State, _>(&dh, None, crate::handlers::clipboard_manager_client);
        let layer_shell_state = WlrLayerShellState::new::<State>(&dh);
        let viewporter_state = ViewporterState::new::<State>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<State>(&dh);
        let glass_manager_state = crate::glass::GlassManagerState::new(&dh);
        let window_manager_state = crate::window_appearance::WindowManagerState::new(&dh);
        let foreign_toplevel_list = crate::shell::toplevel_list(&dh);
        let shell_manager_state = crate::shell::ShellManagerState::new(&dh);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "seat0");
        let (delay, rate) = key_repeat();
        seat.add_keyboard(Default::default(), delay, rate)?;
        seat.add_pointer();

        let _global = output.create_global::<State>(&dh);
        let mut space = Space::default();
        space.map_output(&output, (0, 0));

        let socket = ListeningSocketSource::new_auto()?;
        let socket_name = socket.socket_name().to_os_string();
        loop_handle.insert_source(socket, |stream, _, data| {
            // The process on the other side of the socket tells whether it is a shell
            // component: it is read here, once, because the global filters cannot query the
            // display while it is locked by the dispatch.
            let shell = crate::shell::stream_is_shell(&stream);
            let pid = rustix::net::sockopt::socket_peercred(&stream)
                .ok()
                .map(|credentials| credentials.pid.as_raw_nonzero().get());
            if let Err(err) = data.state.display_handle.insert_client(
                stream,
                Arc::new(ClientState {
                    shell,
                    pid,
                    ..ClientState::default()
                }),
            ) {
                warn!("Wayland client refused: {err}");
            }
        })?;

        let poll_fd = display.backend().poll_fd().try_clone_to_owned()?;
        loop_handle.insert_source(
            Generic::new(poll_fd, Interest::READ, Mode::Level),
            |_, _, data| {
                if let Err(err) = data.display.dispatch_clients(&mut data.state) {
                    warn!("client dispatch failed: {err}");
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
            appearance: None,
            drag_icon: None,
            cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
            cursor_override: None,
            window_veil: false,
            cursor_theme: crate::cursor_theme::CursorTheme::load(),
            cursor_shape_state: smithay::wayland::cursor_shape::CursorShapeManagerState::new::<State>(
                &dh,
            ),
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
            data_control_state,
            layer_shell_state,
            viewporter_state,
            dmabuf_state: DmabufState::new(),
            dmabuf_global: None,
            xdg_decoration_state,
            glass_manager_state,
            window_manager_state,
            foreign_toplevel_list,
            shell_manager_state,
            shell_managers: Vec::new(),
            transients: Vec::new(),
            transient_press: false,
            shell_states: std::collections::BTreeMap::new(),
            announced_focus: None,
            seat,
            mapped: 0,
            next_window_id: 0,
            shortcuts: crate::commands::shortcuts(),
            keys: crate::keys::KeyLookup::new()?,
            perf: crate::perf::Perf::default(),
            clear_color: CLEAR_COLOR,
            minimized: Vec::new(),
            hidden: Vec::new(),
            windowless_front: None,
            app_frames: crate::app_frames::AppFrames::default(),
            keeper: crate::keep::Keeper::default(),
            workspaces: crate::workspaces::Workspaces::default(),
            panorama: None,
            panorama_press: false,
            switcher_active: false,
            logo_key: crate::keys::LogoKey::None,
            gesture: None,
            titlebar_hover: None,
            titlebar_press: None,
            titlebar_press_inside: false,
            titlebar_press_count: 0,
            screenshot_count: 0,
            buttons_hover: None,
            moving: None,
            resizing: None,
            press_serial: None,
            animation_speed: 1.0,
            closing: Vec::new(),
            genies: Vec::new(),
            minimize_effect: crate::render::MinimizeEffect::Genie,
            glass_setting: GlassSetting::Auto,
            software_renderer: false,
        })
    }

    /// The surface under a point of the desktop, with its origin: first the layers above
    /// the windows, then the windows, then the layers below.
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

    /// Sends the frame callbacks to every visible window, and to hidden ones (#142).
    pub fn send_frames(&self) {
        let output = &self.output;
        let now = self.start_time.elapsed();
        for window in self.space.elements() {
            window.send_frame(output, now, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
        // A windowless app (#142) draws its next window while hidden: its frame callbacks keep
        // coming, so its renderer never waits on a surface that is not on screen.
        for (window, _) in &self.hidden {
            window.send_frame(output, now, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
        for layer in layer_map_for_output(output).layers() {
            layer.send_frame(output, now, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
        if let Some(icon) = &self.drag_icon {
            send_frames_surface_tree(icon, output, now, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
    }

    /// The drag icon and where to draw it: its corner on the pointer.
    pub fn drag_icon_view(&self) -> Option<(WlSurface, Point<f64, Logical>)> {
        let icon = self.drag_icon.as_ref()?;
        if !smithay::reexports::wayland_server::Resource::is_alive(icon) {
            return None;
        }
        let pointer = self.seat.get_pointer()?;
        Some((icon.clone(), pointer.current_location()))
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    /// Shell component (`shell.rs`): sees the shell protocols.
    pub shell: bool,
    /// The client process (SO_PEERCRED), for force quit.
    pub pid: Option<i32>,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

/// The pointer on a window's button group.
#[derive(Clone)]
pub struct ButtonsHover {
    pub window: Window,
    pub entered: std::time::Instant,
    pub left: Option<std::time::Instant>,
}

/// Default key repeat, sent to the clients in `wl_keyboard.repeat_info` (they run the timer).
/// Provisional: the reference only exposes the Keyboard sliders' positions (K-03), not
/// milliseconds; these are its system defaults (initial delay 375 ms, one repeat every 90 ms).
/// Override with `OSSIDO_KEY_REPEAT=<delay ms>,<rate per second>`.
const KEY_REPEAT_DELAY_MS: i32 = 375;
const KEY_REPEAT_RATE: i32 = 11;

fn key_repeat() -> (i32, i32) {
    parse_key_repeat(std::env::var("OSSIDO_KEY_REPEAT").ok().as_deref())
}

fn parse_key_repeat(value: Option<&str>) -> (i32, i32) {
    let parsed = value.and_then(|v| {
        let (delay, rate) = v.split_once(',')?;
        let delay: i32 = delay.trim().parse().ok()?;
        let rate: i32 = rate.trim().parse().ok()?;
        (delay > 0 && (0..=200).contains(&rate)).then_some((delay, rate))
    });
    parsed.unwrap_or((KEY_REPEAT_DELAY_MS, KEY_REPEAT_RATE))
}

#[cfg(test)]
mod key_repeat_tests {
    use super::*;

    #[test]
    fn key_repeat_defaults_and_override() {
        assert_eq!(parse_key_repeat(None), (375, 11));
        assert_eq!(parse_key_repeat(Some("250, 30")), (250, 30));
        // Rate 0 disables repeat (Wayland semantics).
        assert_eq!(parse_key_repeat(Some("500,0")), (500, 0));
        assert_eq!(parse_key_repeat(Some("fast")), (375, 11));
        assert_eq!(parse_key_repeat(Some("-1,30")), (375, 11));
    }
}
