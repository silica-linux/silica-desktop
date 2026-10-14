//! The Wayland window hosting the engine, and the platform thread loop.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use silica_protocols::glass::client::silica_glass_manager_v1::SilicaGlassManagerV1;
use silica_protocols::glass::client::silica_glass_v1::SilicaGlassV1;
use silica_protocols::shell::client::silica_shell_manager_v1::SilicaShellManagerV1;
use silica_protocols::window::client::silica_window_manager_v1::SilicaWindowManagerV1;
use silica_protocols::window::client::silica_window_v1::SilicaWindowV1;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::data_device_manager::data_device::{DataDevice, DataDeviceHandler};
use smithay_client_toolkit::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use smithay_client_toolkit::data_device_manager::data_source::{DataSourceHandler, DragSource};
use smithay_client_toolkit::data_device_manager::{DataDeviceManagerState, WritePipe};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop::channel::{self, Channel, Event};
use smithay_client_toolkit::reexports::calloop::generic::Generic;
use smithay_client_toolkit::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay_client_toolkit::reexports::calloop::{
    EventLoop, Interest, LoopHandle, Mode as LoopMode, PostAction, RegistrationToken,
};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::protocol::wl_data_device::WlDataDevice;
use smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::DndAction;
use smithay_client_toolkit::reexports::client::protocol::wl_data_source::WlDataSource;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::keyboard::{
    KeyEvent as SeatKeyEvent, KeyboardHandler, Keysym, Modifiers,
};
use smithay_client_toolkit::seat::pointer::{
    PointerEvent as SeatPointerEvent, PointerEventKind, PointerHandler,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::xdg::window::{
    Window, WindowConfigure, WindowDecorations, WindowHandler,
};
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_data_device, delegate_keyboard, delegate_layer, delegate_output,
    delegate_pointer, delegate_registry, delegate_seat, delegate_shm, delegate_xdg_shell,
    delegate_xdg_window, registry_handlers,
};
use tracing::{info, warn};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface};
use wayland_client::{Connection, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
};

use crate::{LayerKeyboard, LayerKind, Options, Renderer};

/// A drag that left this window (`silica/drag` › `start`).
struct DragOut {
    source: DragSource,
    /// The `text/uri-list` body to send to whoever asks for it.
    body: Arc<Vec<u8>>,
    /// The icon and its buffer, alive until the end.
    icon: Option<(wl_surface::WlSurface, Buffer)>,
    /// The last action chosen by the compositor.
    action: DndAction,
}

/// The surface hosting the app: a normal window or a shell layer.
enum Shell {
    Window(Window),
    Layer(LayerSurface),
}

impl Shell {
    fn wl_surface(&self) -> &wl_surface::WlSurface {
        match self {
            Shell::Window(window) => window.wl_surface(),
            Shell::Layer(layer) => layer.wl_surface(),
        }
    }

    fn commit(&self) {
        match self {
            Shell::Window(window) => window.commit(),
            Shell::Layer(layer) => layer.commit(),
        }
    }

    /// The window title; shell layers have none.
    fn set_title(&self, title: &str) {
        if let Shell::Window(window) = self {
            window.set_title(title.to_owned());
        }
    }
}
use crate::dbus;
use crate::ffi::{
    Callbacks, Engine, EngineConfig, Frame, GlSurface, KeyEvent, PlatformMessage, PointerEvent,
    PointerPhase, ResponseHandle, Task, VsyncBaton, VulkanSurface,
};
use crate::glass::{self, Glass};
use crate::keyboard::{Keyboard, edit_key, raw_key_message};
use crate::shell::{self, ShellLink};

/// The app state channel (StringCodec).
const LIFECYCLE_CHANNEL: &str = "flutter/lifecycle";

/// The `flutter/lifecycle` message for an active window or not.
fn lifecycle_message(active: bool) -> &'static str {
    if active {
        "AppLifecycleState.resumed"
    } else {
        "AppLifecycleState.inactive"
    }
}

/// The channel for the input region and keyboard of a layer (shell components).
const LAYER_CHANNEL: &str = "silica/layer";

/// The window appearance channel (`silica-window-v1`): full-size content with the app's
/// toolbar, and moving the window by dragging its bar.
const WINDOW_CHANNEL: &str = "silica/window";
use crate::text_input::{self, EditKey, TextInput};

/// Outcome of a key sent to the framework, with the editing action to do if ignored.
struct KeyResult {
    handled: bool,
    edit: Option<EditKey>,
}

/// Collects the answers of the two paths of a key; on the last one it sends the result.
struct KeyJoin {
    remaining: u8,
    handled: bool,
    edit: Option<EditKey>,
    results: channel::Sender<KeyResult>,
}

impl KeyJoin {
    fn done(join: &Mutex<KeyJoin>, handled: bool) {
        let Ok(mut join) = join.lock() else {
            return;
        };
        join.handled |= handled;
        join.remaining = join.remaining.saturating_sub(1);
        if join.remaining == 0 {
            let result = KeyResult {
                handled: join.handled,
                edit: join.edit.take(),
            };
            let _ = join.results.send(result);
        }
    }
}

/// Mouse button codes (linux/input-event-codes.h) and Flutter masks.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

/// Frame period given to the engine with every vsync: 60 Hz.
const FRAME_PERIOD_NANOS: u64 = 16_666_667;

/// Maximum interval between two loop iterations when no task is queued.
const IDLE_WAKEUP: Duration = Duration::from_millis(100);

/// A queued task, ordered by deadline and then by arrival order.
struct Queued {
    target_nanos: u64,
    seq: u64,
    task: Task,
}

impl PartialEq for Queued {
    fn eq(&self, other: &Self) -> bool {
        (self.target_nanos, self.seq) == (other.target_nanos, other.seq)
    }
}
impl Eq for Queued {}
impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Queued {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.target_nanos, self.seq).cmp(&(other.target_nanos, other.seq))
    }
}

struct App {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    surface: Shell,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keys: Keyboard,
    modifiers: Modifiers,
    text_input: TextInput,
    /// Glass regions of the `Glass` widgets (`silica/glass`).
    glass: Glass,
    /// `silica-window-v1`: the manager and the window object (created on the first request).
    appearance_manager: Option<SilicaWindowManagerV1>,
    appearance: Option<SilicaWindowV1>,
    /// The window active for Ossido (xdg_toplevel's `activated`), as Flutter was told.
    activated: Option<bool>,
    /// Drag between apps: the manager, the seat's device, the drag that left from here and
    /// the offer dropped here (the data arrives from a thread on `drops`).
    data_manager: Option<DataDeviceManagerState>,
    data_device: Option<DataDevice>,
    drag_out: Option<DragOut>,
    drag_in: bool,
    dropped: Option<DragOffer>,
    /// Pseudo-terminals of the app (`silica/pty`), their sources in the loop and whether the
    /// frame timer runs.
    ptys: crate::pty::PtyHost,
    pty_sources: BTreeMap<u32, RegistrationToken>,
    pty_ticking: bool,
    loop_handle: LoopHandle<'static, App>,
    drops: Option<channel::Sender<(f64, f64, Vec<u8>)>>,
    /// The pointer seat and the serial of the last press (to move the window).
    seat: Option<wl_seat::WlSeat>,
    press_serial: u32,
    /// Windows and commands for the shell components (`silica/shell`).
    shell: ShellLink,
    /// For the input regions (`silica/layer`).
    compositor: CompositorState,
    /// The `silica/dbus` channel, on a separate thread.
    dbus: Option<dbus::Bridge<ResponseHandle>>,
    qh: QueueHandle<App>,
    key_results: channel::Sender<KeyResult>,
    engine: Option<Engine>,

    tasks: BinaryHeap<Reverse<Queued>>,
    next_seq: u64,

    /// Size in logical pixels and scale factor of the surface.
    size: (u32, u32),
    scale: i32,
    configured: bool,
    /// The last engine frame not yet presented.
    pending: Option<Frame>,
    /// Waiting for the compositor's frame callback before presenting again.
    waiting_frame: bool,
    buffer: Option<Buffer>,
    buttons: i64,
    /// With OpenGL the engine's swap attaches and commits: here only the scale is handled.
    gl: bool,
    /// Software has presented at least one frame (with OpenGL the EGL surface knows).
    presented: bool,
    /// The engine's vsync request waiting for the compositor's frame callback.
    vsync: Option<VsyncBaton>,
    /// A frame callback asked for the vsync and not yet arrived.
    vsync_requested: bool,
    exit: bool,
}

pub fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    for path in [&options.library, &options.icu_data] {
        if !path.exists() {
            return Err(format!("{} missing: run `cargo xtask engine`", path.display()).into());
        }
    }
    // A debug bundle carries the Dart kernel, a release one the asset manifest and libapp.so.
    if !options.bundle.join("kernel_blob.bin").exists()
        && !options.bundle.join("AssetManifest.bin").exists()
    {
        return Err(format!(
            "{} is not a Flutter bundle: run `cargo xtask bundle <app>`",
            options.bundle.display()
        )
        .into());
    }

    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<App> = EventLoop::try_new()?;
    WaylandSource::new(conn.clone(), queue)
        .insert(event_loop.handle())
        .map_err(|e| format!("Wayland source: {e}"))?;

    let compositor = CompositorState::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let surface = compositor.create_surface(&qh);
    let surface = match &options.layer {
        None => {
            let xdg_shell = XdgShell::bind(&globals, &qh)?;
            let window = xdg_shell.create_window(surface, WindowDecorations::RequestServer, &qh);
            window.set_title(options.title.clone());
            window.set_app_id(options.app_id.clone());
            window.commit();
            Shell::Window(window)
        }
        Some(layer) => {
            let shell = LayerShell::bind(&globals, &qh)?;
            let level = match layer.layer {
                LayerKind::Background => Layer::Background,
                LayerKind::Bottom => Layer::Bottom,
                LayerKind::Top => Layer::Top,
                LayerKind::Overlay => Layer::Overlay,
            };
            let surface = shell.create_layer_surface(
                &qh,
                surface,
                level,
                Some(layer.namespace.clone()),
                None,
            );
            let mut anchor = Anchor::empty();
            for (on, edge) in [
                (layer.anchor.top, Anchor::TOP),
                (layer.anchor.bottom, Anchor::BOTTOM),
                (layer.anchor.left, Anchor::LEFT),
                (layer.anchor.right, Anchor::RIGHT),
            ] {
                anchor.set(edge, on);
            }
            surface.set_anchor(anchor);
            // A size anchored to both sides is decided by the compositor (0).
            let (w, h) = options.size;
            let width = if layer.anchor.left && layer.anchor.right {
                0
            } else {
                w
            };
            let height = if layer.anchor.top && layer.anchor.bottom {
                0
            } else {
                h
            };
            surface.set_size(width, height);
            surface.set_exclusive_zone(layer.exclusive_zone);
            surface.set_keyboard_interactivity(match layer.keyboard {
                LayerKeyboard::None => KeyboardInteractivity::None,
                LayerKeyboard::Exclusive => KeyboardInteractivity::Exclusive,
                LayerKeyboard::OnDemand => KeyboardInteractivity::OnDemand,
            });
            surface.commit();
            Shell::Layer(surface)
        }
    };

    let mut engine_args = options.engine_args.clone();
    // Vulkan with Impeller; if Vulkan is not there (or does not present on this surface) it
    // falls back to OpenGL ES with Impeller.
    let vulkan = match options.renderer {
        Renderer::Vulkan => match VulkanSurface::new(&conn, surface.wl_surface(), options.size) {
            Ok(vulkan) => {
                info!("renderer Vulkan: {}", vulkan.renderer());
                engine_args.push("--enable-impeller".to_owned());
                Some(vulkan)
            }
            Err(err) => {
                warn!("Vulkan unavailable ({err}): using OpenGL ES with Impeller");
                None
            }
        },
        _ => None,
    };
    let gl = match options.renderer {
        Renderer::Vulkan if vulkan.is_some() => None,
        Renderer::Gl | Renderer::Impeller | Renderer::Vulkan => {
            let gl = GlSurface::new(&conn, surface.wl_surface(), options.size)?;
            info!("renderer OpenGL ES: {}", gl.renderer());
            if options.renderer != Renderer::Gl {
                if crate::is_software_gl(gl.renderer()) {
                    warn!("Impeller crashes on a software GL driver: using Skia");
                } else {
                    engine_args.push("--enable-impeller".to_owned());
                }
            }
            let (redraw, redraw_channel) = channel::channel::<()>();
            event_loop
                .handle()
                .insert_source(redraw_channel, |event, _, app: &mut App| {
                    if let (Event::Msg(()), Some(engine)) = (event, &app.engine)
                        && let Err(err) = engine.schedule_frame()
                    {
                        warn!("{err}");
                    }
                })
                .map_err(|e| format!("redraw channel: {e}"))?;
            gl.on_redraw(move || {
                let _ = redraw.send(());
            });
            Some(gl)
        }
        Renderer::Software => None,
    };
    let gl_mode = gl.is_some() || vulkan.is_some();

    let (width, height) = options.size;
    let pool = SlotPool::new((width * height * 4) as usize, &shm)?;

    // Tasks and frames arrive from the engine threads: two channels to this thread.
    let (task_sender, task_channel): (channel::Sender<Task>, Channel<Task>) = channel::channel();
    let (frame_sender, frame_channel): (channel::Sender<Frame>, Channel<Frame>) =
        channel::channel();
    event_loop
        .handle()
        .insert_source(task_channel, |event, _, app: &mut App| {
            if let Event::Msg(task) = event {
                app.next_seq = app.next_seq.wrapping_add(1);
                app.tasks.push(Reverse(Queued {
                    target_nanos: task.target_nanos,
                    seq: app.next_seq,
                    task,
                }));
            }
        })
        .map_err(|e| format!("task channel: {e}"))?;
    let (message_sender, message_channel) = channel::channel::<PlatformMessage>();
    event_loop
        .handle()
        .insert_source(message_channel, |event, _, app: &mut App| {
            if let Event::Msg(message) = event {
                app.handle_message(message);
            }
        })
        .map_err(|e| format!("message channel: {e}"))?;
    let (dbus_sender, dbus_channel) = channel::channel::<dbus::Event<ResponseHandle>>();
    event_loop
        .handle()
        .insert_source(dbus_channel, |event, _, app: &mut App| {
            if let Event::Msg(event) = event {
                app.handle_dbus(event);
            }
        })
        .map_err(|e| format!("D-Bus channel: {e}"))?;
    let dbus = dbus::Bridge::start(dbus::Addresses::default(), move |event| {
        let _ = dbus_sender.send(event);
    })
    .map_err(|err| warn!("silica/dbus unavailable: {err}"))
    .ok();
    let (vsync_sender, vsync_channel) = channel::channel::<VsyncBaton>();
    let vsync_qh = qh.clone();
    event_loop
        .handle()
        .insert_source(vsync_channel, move |event, _, app: &mut App| {
            if let Event::Msg(baton) = event {
                app.request_vsync(baton, &vsync_qh);
            }
        })
        .map_err(|e| format!("vsync channel: {e}"))?;
    let (key_results, key_channel) = channel::channel::<KeyResult>();
    event_loop
        .handle()
        .insert_source(key_channel, |event, _, app: &mut App| {
            if let Event::Msg(result) = event {
                app.key_result(result);
            }
        })
        .map_err(|e| format!("key channel: {e}"))?;
    let frame_qh = qh.clone();
    event_loop
        .handle()
        .insert_source(frame_channel, move |event, _, app: &mut App| {
            if let Event::Msg(frame) = event {
                app.pending = Some(frame);
                app.present(&frame_qh);
            }
        })
        .map_err(|e| format!("frame channel: {e}"))?;

    let callbacks = Callbacks {
        platform_thread: std::thread::current().id(),
        post_task: Box::new(move |task| {
            if task_sender.send(task).is_err() {
                warn!("task dropped: the main loop is closed");
            }
        }),
        present: Box::new(move |frame| {
            let _ = frame_sender.send(frame);
        }),
        platform_message: Box::new(move |message| {
            // If the loop is closed the message falls with its handle: the engine is shutting
            // down and no longer waits for answers.
            let _ = message_sender.send(message);
        }),
        gl,
        vulkan,
        vsync: Some(Box::new(move |baton| {
            let _ = vsync_sender.send(baton);
        })),
    };
    let engine = Engine::run(
        &EngineConfig {
            library: &options.library,
            assets: &options.bundle,
            icu_data: &options.icu_data,
            aot: options.aot.as_deref(),
            engine_args: &engine_args,
            dart_args: &options.dart_args,
        },
        callbacks,
    )?;
    info!("engine started with {}", options.bundle.display());

    let mut app = App {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        surface,
        pointer: None,
        keyboard: None,
        keys: Keyboard::default(),
        modifiers: Modifiers::default(),
        text_input: TextInput::default(),
        appearance_manager: globals
            .bind::<SilicaWindowManagerV1, App, ()>(&qh, 1..=1, ())
            .ok(),
        appearance: None,
        activated: None,
        data_manager: DataDeviceManagerState::bind(&globals, &qh).ok(),
        data_device: None,
        drag_out: None,
        drag_in: false,
        dropped: None,
        ptys: crate::pty::PtyHost::default(),
        pty_sources: BTreeMap::new(),
        pty_ticking: false,
        loop_handle: event_loop.handle(),
        drops: None,
        seat: None,
        press_serial: 0,
        glass: Glass::new(
            globals
                .bind::<SilicaGlassManagerV1, App, ()>(&qh, 1..=5, ())
                .ok(),
        ),
        shell: ShellLink::new(
            globals
                .bind::<SilicaShellManagerV1, App, ()>(&qh, 1..=5, ())
                .ok(),
            globals
                .bind::<ExtForeignToplevelListV1, App, ()>(&qh, 1..=1, ())
                .ok(),
        ),
        compositor,
        dbus,
        qh: qh.clone(),
        key_results,
        engine: Some(engine),
        tasks: BinaryHeap::new(),
        next_seq: 0,
        size: options.size,
        scale: 1,
        configured: false,
        pending: None,
        waiting_frame: false,
        buffer: None,
        buttons: 0,
        gl: gl_mode,
        presented: false,
        vsync: None,
        vsync_requested: false,
        exit: false,
    };

    // Appearance: light until the portal says otherwise, then on every change.
    // The system languages, before the first frame.
    if let Some(engine) = &app.engine
        && let Err(err) = engine.update_locales(&crate::locale::from_env(|k| std::env::var(k).ok()))
    {
        warn!("languages: {err}");
    }
    app.send_appearance(false);
    let (appearance_sender, appearance_channel) = channel::channel::<bool>();
    event_loop
        .handle()
        .insert_source(appearance_channel, |event, _, app: &mut App| {
            if let Event::Msg(dark) = event {
                app.send_appearance(dark);
            }
        })
        .map_err(|e| format!("appearance channel: {e}"))?;
    let _appearance = crate::appearance::watch(appearance_sender);

    // The data of a drop, read on a thread (the source may be this very window).
    let (drop_sender, drop_channel) = channel::channel::<(f64, f64, Vec<u8>)>();
    event_loop
        .handle()
        .insert_source(drop_channel, |event, _, app: &mut App| {
            if let Event::Msg((x, y, data)) = event {
                app.deliver_drop(x, y, &data);
            }
        })
        .map_err(|e| format!("drop channel: {e}"))?;
    app.drops = Some(drop_sender);

    while !app.exit {
        let timeout = app.run_due_tasks();
        event_loop.dispatch(Some(timeout), &mut app)?;
    }
    // Shuts down the engine before closing the Wayland connection.
    app.engine = None;
    Ok(())
}

impl App {
    /// Answers a framework message. Every message with a handle gets a
    /// response, empty if the channel is not handled.
    fn handle_message(&mut self, message: PlatformMessage) {
        // silica/dbus answers later, from its thread (`handle_dbus`).
        if message.channel == dbus::CHANNEL
            && let Some(bridge) = &self.dbus
        {
            if let Some(handle) = message.response {
                bridge.handle(&message.data, handle);
            }
            return;
        }
        let reply = match message.channel.as_str() {
            text_input::CHANNEL => self.text_input.handle_call(&message.data),
            shell::CHANNEL => self.shell.handle_call(&message.data, &self.qh),
            LAYER_CHANNEL => self.handle_layer_call(&message.data),
            WINDOW_CHANNEL => self.handle_window_call(&message.data),
            crate::drag::CHANNEL => self.handle_drag_call(&message.data),
            crate::pty::CHANNEL => self.handle_pty_call(&message.data),
            glass::CHANNEL => {
                let (reply, changed) =
                    self.glass
                        .handle_call(&message.data, self.surface.wl_surface(), &self.qh);
                // The regions apply from the commit: without a new frame it is needed here.
                if changed {
                    self.surface.commit();
                }
                reply
            }
            "flutter/platform" => self.handle_platform_call(&message.data),
            // StandardMethodCodec: getKeyboardState, the pressed keys physical → logical.
            "flutter/keyboard" => crate::codec::success_int_map(
                self.keys
                    .pressed()
                    .iter()
                    .map(|(physical, logical)| (*physical as i64, *logical as i64)),
            ),
            _ => Vec::new(),
        };
        if let (Some(engine), Some(handle)) = (&self.engine, message.response)
            && let Err(err) = engine.respond(handle, &reply)
        {
            warn!("response on {}: {err}", message.channel);
        }
    }

    /// The `silica/window` channel (JSONMethodCodec):
    /// - `setFullSizeContent {toolbarHeight}`: full-size content (0 goes back to Ossido's
    ///   bar); answers whether Ossido offers it;
    /// - `startMove`: moves the window with the pointer (from the app's toolbar).
    fn handle_window_call(&mut self, data: &[u8]) -> Vec<u8> {
        let Ok(call) = serde_json::from_slice::<serde_json::Value>(data) else {
            return Vec::new();
        };
        let Shell::Window(window) = &self.surface else {
            return crate::text_input::success(serde_json::json!(false));
        };
        match call.get("method").and_then(serde_json::Value::as_str) {
            Some("setFullSizeContent") => {
                let Some(manager) = &self.appearance_manager else {
                    return crate::text_input::success(serde_json::json!(false));
                };
                let height = call
                    .pointer("/args/toolbarHeight")
                    .and_then(serde_json::Value::as_f64)
                    .filter(|h| h.is_finite() && *h >= 0.0)
                    .map_or(0, |h| h.round().min(200.0) as u32);
                let object = self
                    .appearance
                    .get_or_insert_with(|| manager.get_window(window.wl_surface(), &self.qh, ()));
                object.set_full_size_content(height);
                window.commit();
                crate::text_input::success(serde_json::json!(true))
            }
            Some("startMove") => {
                if let Some(seat) = &self.seat {
                    window.move_(seat, self.press_serial);
                }
                crate::text_input::success(serde_json::Value::Null)
            }
            _ => Vec::new(),
        }
    }

    /// The `flutter/platform` channel (JSONMethodCodec): for now the window title
    /// (`SystemChrome.setApplicationSwitcherDescription`, `label`). The rest is not
    /// implemented (empty response).
    fn handle_platform_call(&mut self, data: &[u8]) -> Vec<u8> {
        let Ok(call) = serde_json::from_slice::<serde_json::Value>(data) else {
            return Vec::new();
        };
        match call.get("method").and_then(serde_json::Value::as_str) {
            Some("SystemChrome.setApplicationSwitcherDescription") => {
                let label = call
                    .get("args")
                    .and_then(|args| args.get("label"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                // A very long title from an app must not become a problem for Ossido.
                let title: String = label.chars().take(256).collect();
                self.surface.set_title(&title);
                self.surface.commit();
                b"[null]".to_vec()
            }
            _ => Vec::new(),
        }
    }

    /// `flutter/lifecycle`: the active window is `resumed`, the others `inactive` (apps follow
    /// the state with `AppLifecycleListener`, for the colors of the inactive window).
    fn send_lifecycle(&mut self, active: bool) {
        if let Some(engine) = &self.engine
            && let Err(err) =
                engine.send_message(LIFECYCLE_CHANNEL, lifecycle_message(active).as_bytes())
        {
            warn!("flutter/lifecycle: {err}");
        }
    }

    /// The `silica/pty` channel (see `pty.rs`).
    fn handle_pty_call(&mut self, data: &[u8]) -> Vec<u8> {
        let reply = match crate::pty::parse_call(data) {
            Ok(call) => self.ptys.handle(call),
            Err(err) => Err(err),
        };
        let reply = match reply {
            Ok(reply) => reply,
            Err(err) => return crate::pty::error(&err),
        };
        if let Some(id) = reply.closed
            && let Some(token) = self.pty_sources.remove(&id)
        {
            self.loop_handle.remove(token);
        }
        if let Some(id) = reply.spawned {
            self.watch_pty(id);
        }
        self.start_pty_ticks();
        crate::text_input::success(reply.value)
    }

    /// Reads the pty `id` when its master has data.
    fn watch_pty(&mut self, id: u32) {
        let fd = match self.ptys.master(id).map(|fd| fd.try_clone_to_owned()) {
            Some(Ok(fd)) => fd,
            Some(Err(err)) => {
                warn!("pty {id}: {err}");
                return;
            }
            None => return,
        };
        let source = Generic::new(fd, Interest::READ, LoopMode::Level);
        let inserted = self
            .loop_handle
            .insert_source(source, move |_, _, app: &mut App| {
                Ok(match app.ptys.readable(id) {
                    crate::pty::Readable::Continue => PostAction::Continue,
                    crate::pty::Readable::Pause => PostAction::Disable,
                    crate::pty::Readable::Remove => {
                        app.pty_sources.remove(&id);
                        PostAction::Remove
                    }
                })
            });
        match inserted {
            Ok(token) => {
                self.pty_sources.insert(id, token);
            }
            Err(err) => warn!("pty {id}: {err}"),
        }
    }

    /// One message per frame per pty: a timer at the frame period while there are ptys.
    fn start_pty_ticks(&mut self) {
        if self.pty_ticking {
            return;
        }
        let timer = Timer::from_duration(Duration::from_nanos(FRAME_PERIOD_NANOS));
        let inserted = self
            .loop_handle
            .insert_source(timer, |_, _, app: &mut App| {
                if app.pty_tick() {
                    TimeoutAction::ToDuration(Duration::from_nanos(FRAME_PERIOD_NANOS))
                } else {
                    app.pty_ticking = false;
                    TimeoutAction::Drop
                }
            });
        match inserted {
            Ok(_) => self.pty_ticking = true,
            Err(err) => warn!("pty timer: {err}"),
        }
    }

    /// Sends the output and the exits of the frame; false when no pty is left.
    fn pty_tick(&mut self) -> bool {
        let tick = self.ptys.tick(std::time::Instant::now());
        if let Some(engine) = &self.engine {
            for message in &tick.messages {
                if let Err(err) = engine.send_message(crate::pty::CHANNEL, message) {
                    warn!("silica/pty: {err}");
                }
            }
        }
        for id in &tick.resume {
            if let Some(token) = self.pty_sources.get(id)
                && let Err(err) = self.loop_handle.enable(token)
            {
                warn!("pty {id}: {err}");
            }
        }
        for id in &tick.gone {
            if let Some(token) = self.pty_sources.remove(id) {
                self.loop_handle.remove(token);
            }
        }
        !tick.idle
    }

    /// The `silica/drag` channel (see `drag.rs`): `start` begins a drag.
    fn handle_drag_call(&mut self, data: &[u8]) -> Vec<u8> {
        let reply = |started: bool| crate::text_input::success(serde_json::json!(started));
        let Ok(call) = serde_json::from_slice::<serde_json::Value>(data) else {
            return Vec::new();
        };
        if call.get("method").and_then(serde_json::Value::as_str) != Some("start") {
            return Vec::new();
        }
        let Some(start) = call.get("args").and_then(crate::drag::parse_start) else {
            return reply(false);
        };
        let (Some(manager), Some(device)) = (&self.data_manager, &self.data_device) else {
            return reply(false);
        };
        let source = manager.create_drag_and_drop_source(
            &self.qh,
            [crate::drag::URI_LIST],
            DndAction::Copy | DndAction::Move,
        );
        let icon = start.icon.as_ref().and_then(|icon| {
            let side = |v: u32| i32::try_from(v).ok();
            let (width, height) = (side(icon.width)?, side(icon.height)?);
            let (buffer, canvas) = self
                .pool
                .create_buffer(width, height, width * 4, wl_shm::Format::Argb8888)
                .ok()?;
            canvas
                .get_mut(..icon.bgra.len())?
                .copy_from_slice(&icon.bgra);
            let surface = self.compositor.create_surface(&self.qh);
            Some((surface, buffer, width, height))
        });
        source.start_drag(
            device,
            self.surface.wl_surface(),
            icon.as_ref().map(|(surface, ..)| surface),
            self.press_serial,
        );
        let icon = icon.and_then(|(surface, buffer, width, height)| {
            buffer.attach_to(&surface).ok()?;
            surface.damage_buffer(0, 0, width, height);
            surface.commit();
            Some((surface, buffer))
        });
        self.drag_out = Some(DragOut {
            source,
            body: Arc::new(crate::drag::uri_list(&start.uris)),
            icon,
            action: DndAction::empty(),
        });
        reply(true)
    }

    /// A call to the app on `silica/drag`.
    fn send_drag(&self, method: &str, args: serde_json::Value) {
        if let Some(engine) = &self.engine
            && let Err(err) =
                engine.send_message(crate::drag::CHANNEL, &crate::drag::call(method, args))
        {
            warn!("silica/drag: {err}");
        }
    }

    /// The drag that left from here has finished: the app learns with which action.
    fn finish_drag_out(&mut self, completed: bool) {
        let Some(drag) = self.drag_out.take() else {
            return;
        };
        let action = if !completed {
            "none"
        } else if drag.action.contains(DndAction::Move) {
            "move"
        } else if drag.action.contains(DndAction::Copy) {
            "copy"
        } else {
            "none"
        };
        if let Some((surface, _)) = &drag.icon {
            surface.destroy();
        }
        self.send_drag("finished", serde_json::json!({ "action": action }));
    }

    /// The data of a drop has arrived: the URIs to the app, then the offer closes.
    fn deliver_drop(&mut self, x: f64, y: f64, data: &[u8]) {
        let uris = crate::drag::parse_uri_list(data);
        self.send_drag("drop", serde_json::json!({ "x": x, "y": y, "uris": uris }));
        if let Some(offer) = self.dropped.take() {
            offer.finish();
            offer.destroy();
        }
    }

    /// `flutter/settings` with the appearance (light or dark): `MediaQuery.platformBrightness`.
    fn send_appearance(&mut self, dark: bool) {
        if let Some(engine) = &self.engine
            && let Err(err) = engine.send_message(
                crate::appearance::SETTINGS_CHANNEL,
                &crate::appearance::settings_message(dark),
            )
        {
            warn!("flutter/settings: {err}");
        }
    }

    /// An outcome of the D-Bus thread: the reply to a call or a signal for Dart.
    fn handle_dbus(&mut self, event: dbus::Event<ResponseHandle>) {
        let Some(engine) = &self.engine else { return };
        let result = match event {
            dbus::Event::Reply(handle, reply) => engine.respond(handle, &reply),
            dbus::Event::Signal(message) => engine.send_message(dbus::CHANNEL, &message),
        };
        if let Err(err) = result {
            warn!("silica/dbus: {err}");
        }
    }

    /// The `silica/layer` channel: a hidden shell component must not take the
    /// pointer or the keyboard (a closed Faro keeps running, transparent).
    /// - `setKeyboardInteractivity {mode: none|exclusive|on-demand}`;
    /// - `setInputRegion {rects: [[x, y, w, h], …] | null}` (null: the whole surface);
    /// - `outputSize` → `{width, height}` of the screen in points (null if unknown);
    /// - `setSize {width, height}` in points, 0 to follow the anchors (the menu bar grows
    ///   while a menu is open).
    fn handle_layer_call(&mut self, data: &[u8]) -> Vec<u8> {
        let Ok(call) = serde_json::from_slice::<serde_json::Value>(data) else {
            return Vec::new();
        };
        let args = call.get("args").cloned().unwrap_or(serde_json::Value::Null);
        let done = match call.get("method").and_then(serde_json::Value::as_str) {
            Some("setKeyboardInteractivity") => {
                let mode = match args.get("mode").and_then(serde_json::Value::as_str) {
                    Some("exclusive") => KeyboardInteractivity::Exclusive,
                    Some("on-demand") => KeyboardInteractivity::OnDemand,
                    _ => KeyboardInteractivity::None,
                };
                match &self.surface {
                    Shell::Layer(layer) => {
                        layer.set_keyboard_interactivity(mode);
                        true
                    }
                    Shell::Window(_) => false,
                }
            }
            Some("outputSize") => {
                // The logical size of the screen (the first output): to center an alert
                // in a surface that grows until it covers it.
                let size = self
                    .output_state
                    .outputs()
                    .next()
                    .and_then(|output| self.output_state.info(&output))
                    .and_then(|info| info.logical_size);
                return text_input::success(match size {
                    Some((w, h)) => serde_json::json!({"width": w, "height": h}),
                    None => serde_json::Value::Null,
                });
            }
            Some("setSize") => {
                let value = |key: &str| {
                    args.get(key)
                        .and_then(serde_json::Value::as_f64)
                        .filter(|v| v.is_finite())
                        .map(|v| v.round().clamp(0.0, 16384.0) as u32)
                };
                match (&self.surface, value("width"), value("height")) {
                    (Shell::Layer(layer), Some(w), Some(h)) => {
                        layer.set_size(w, h);
                        true
                    }
                    _ => false,
                }
            }
            Some("setInputRegion") => match args.get("rects").and_then(serde_json::Value::as_array)
            {
                None => {
                    self.surface.wl_surface().set_input_region(None);
                    true
                }
                Some(rects) => match Region::new(&self.compositor) {
                    Ok(region) => {
                        for rect in rects {
                            let value = |i: usize| {
                                rect.get(i)
                                    .and_then(serde_json::Value::as_f64)
                                    .filter(|v| v.is_finite())
                                    .map(|v| v.round().clamp(-16384.0, 16384.0) as i32)
                            };
                            if let (Some(x), Some(y), Some(w), Some(h)) =
                                (value(0), value(1), value(2), value(3))
                                && w > 0
                                && h > 0
                            {
                                region.add(x, y, w, h);
                            }
                        }
                        self.surface
                            .wl_surface()
                            .set_input_region(Some(region.wl_region()));
                        true
                    }
                    Err(err) => {
                        warn!("input region: {err}");
                        false
                    }
                },
            },
            _ => return Vec::new(),
        };
        if done {
            self.surface.commit();
        }
        text_input::success(serde_json::json!(done))
    }

    /// Sends a key to the framework through the two paths it expects (key data and
    /// `flutter/keyevent` message); if neither consumes it, the editing action goes to
    /// the text field.
    fn send_key(&self, event: &KeyEvent, raw: Option<Vec<u8>>, edit: Option<EditKey>) {
        let Some(engine) = &self.engine else {
            return;
        };
        let join = Arc::new(Mutex::new(KeyJoin {
            remaining: if raw.is_some() { 2 } else { 1 },
            handled: false,
            edit,
            results: self.key_results.clone(),
        }));
        let data_join = Arc::clone(&join);
        if let Err(err) = engine.send_key(event, move |handled| KeyJoin::done(&data_join, handled))
        {
            warn!("{err}");
            KeyJoin::done(&join, false);
        }
        if let Some(raw) = raw {
            let raw_join = Arc::clone(&join);
            let sent = engine.send_message_with_reply("flutter/keyevent", &raw, move |reply| {
                let handled = serde_json::from_slice::<serde_json::Value>(&reply)
                    .ok()
                    .and_then(|reply| reply.get("handled")?.as_bool())
                    .unwrap_or(false);
                KeyJoin::done(&raw_join, handled);
            });
            if let Err(err) = sent {
                warn!("{err}");
                KeyJoin::done(&join, false);
            }
        }
    }

    fn gtk_modifiers(&self) -> i64 {
        use crate::keyboard::gtk_modifiers::{ALT, CAPS_LOCK, CONTROL, META, NUM_LOCK, SHIFT};
        let m = &self.modifiers;
        [
            (m.shift, SHIFT),
            (m.caps_lock, CAPS_LOCK),
            (m.ctrl, CONTROL),
            (m.alt, ALT),
            (m.num_lock, NUM_LOCK),
            (m.logo, META),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(0, |mask, (_, bit)| mask | bit)
    }

    /// A key not consumed by the framework goes to the active text field.
    fn key_result(&mut self, result: KeyResult) {
        tracing::debug!(
            "key handled by the framework: {}, editing: {:?}",
            result.handled,
            result.edit
        );
        let (false, Some(edit)) = (result.handled, result.edit) else {
            return;
        };
        let Some(engine) = &self.engine else {
            return;
        };
        for message in self.text_input.handle_key(&edit) {
            if let Err(err) = engine.send_message(text_input::CHANNEL, &message) {
                warn!("{err}");
            }
        }
    }

    fn now_micros(&self) -> u64 {
        self.engine
            .as_ref()
            .map_or(0, |engine| engine.current_time() / 1000)
    }

    /// Runs the expired tasks and tells how long to wait for the next one.
    fn run_due_tasks(&mut self) -> Duration {
        let Some(engine) = &self.engine else {
            return IDLE_WAKEUP;
        };
        loop {
            let now = engine.current_time();
            match self.tasks.peek() {
                Some(Reverse(next)) if next.target_nanos <= now => {
                    if let Some(Reverse(queued)) = self.tasks.pop()
                        && let Err(err) = engine.run_task(&queued.task)
                    {
                        warn!("{err}");
                    }
                }
                Some(Reverse(next)) => {
                    return Duration::from_nanos(next.target_nanos - now).min(IDLE_WAKEUP);
                }
                None => return IDLE_WAKEUP,
            }
        }
    }

    fn send_metrics(&self) {
        let Some(engine) = &self.engine else {
            return;
        };
        let scale = u32::try_from(self.scale.max(1)).unwrap_or(1);
        let (w, h) = (
            self.size.0.saturating_mul(scale),
            self.size.1.saturating_mul(scale),
        );
        engine.resize_surface(w, h);
        if let Err(err) = engine.send_window_metrics(w, h, f64::from(scale)) {
            warn!("{err}");
        }
    }

    fn send_pointer(&self, phase: PointerPhase, position: (f64, f64), scroll: Option<(f64, f64)>) {
        let Some(engine) = &self.engine else {
            return;
        };
        let scale = f64::from(self.scale.max(1));
        let event = PointerEvent {
            phase,
            x: position.0 * scale,
            y: position.1 * scale,
            buttons: self.buttons,
            scroll: scroll.map(|(x, y)| (x * scale, y * scale)),
            timestamp_micros: engine.current_time() / 1000,
        };
        if let Err(err) = engine.send_pointer(&event) {
            warn!("{err}");
        }
    }

    /// The engine wants the next vsync: it arrives with the compositor's frame callback.
    /// Before the first frame the compositor sends none, so it answers right away.
    fn request_vsync(&mut self, baton: VsyncBaton, qh: &QueueHandle<Self>) {
        let presented =
            self.presented || (self.gl && self.engine.as_ref().is_some_and(Engine::gl_presented));
        if !presented || !self.configured {
            self.answer_vsync(baton);
            return;
        }
        self.vsync = Some(baton);
        // A frame callback already asked for (by the vsync or by the presentation) is enough.
        if self.vsync_requested || self.waiting_frame {
            return;
        }
        let surface = self.surface.wl_surface();
        surface.frame(qh, surface.clone());
        self.surface.commit();
        self.vsync_requested = true;
    }

    fn answer_vsync(&self, baton: VsyncBaton) {
        let Some(engine) = &self.engine else {
            return;
        };
        let start = engine.current_time();
        if let Err(err) = engine.on_vsync(baton, start, start.saturating_add(FRAME_PERIOD_NANOS)) {
            warn!("{err}");
        }
    }

    /// Presents the last engine frame, if the compositor is ready to receive one.
    fn present(&mut self, qh: &QueueHandle<Self>) {
        if self.gl || self.waiting_frame || !self.configured {
            return;
        }
        let Some(frame) = self.pending.take() else {
            return;
        };
        let (Ok(width), Ok(height)) = (i32::try_from(frame.width), i32::try_from(frame.height))
        else {
            return;
        };
        let stride = width.saturating_mul(4);
        let (buffer, canvas) =
            match self
                .pool
                .create_buffer(width, height, stride, wl_shm::Format::Argb8888)
            {
                Ok(created) => created,
                Err(err) => {
                    warn!("shm buffer not created: {err}");
                    return;
                }
            };
        let row = stride as usize;
        for (dst, src) in canvas
            .chunks_exact_mut(row)
            .zip(frame.pixels.chunks(frame.stride.max(1)))
        {
            if let (Some(dst), Some(src)) = (dst.get_mut(..row), src.get(..row)) {
                dst.copy_from_slice(src);
            }
        }

        let surface = self.surface.wl_surface();
        surface.set_buffer_scale(self.scale.max(1));
        surface.damage_buffer(0, 0, width, height);
        surface.frame(qh, surface.clone());
        if let Err(err) = buffer.attach_to(surface) {
            warn!("buffer attach failed: {err}");
            return;
        }
        self.surface.commit();
        self.buffer = Some(buffer);
        self.waiting_frame = true;
        self.presented = true;
    }
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        factor: i32,
    ) {
        if factor != self.scale && factor >= 1 {
            self.scale = factor;
            if self.gl {
                // It applies from the next commit, that is from the swap of the frame at the new size.
                self.surface.wl_surface().set_buffer_scale(factor);
            }
            self.send_metrics();
        }
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.waiting_frame = false;
        self.vsync_requested = false;
        if let Some(baton) = self.vsync.take() {
            self.answer_vsync(baton);
        }
        self.present(qh);
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl App {
    fn configured(&mut self, qh: &QueueHandle<Self>, size: (u32, u32)) {
        if size.0 > 0 && size.1 > 0 {
            self.size = size;
        }
        self.configured = true;
        self.send_metrics();
        self.present(qh);
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        self.configured(qh, configure.new_size);
    }
}

impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        let size = match configure.new_size {
            (Some(w), Some(h)) => (w.get(), h.get()),
            _ => (0, 0),
        };
        let active = configure.is_activated();
        if self.activated != Some(active) {
            self.activated = Some(active);
            self.send_lifecycle(active);
        }
        self.configured(qh, size);
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(keyboard) => self.keyboard = Some(keyboard),
                Err(err) => warn!("keyboard unavailable: {err}"),
            }
        }
        if capability == Capability::Pointer {
            self.seat = Some(seat.clone());
            if self.data_device.is_none()
                && let Some(manager) = &self.data_manager
            {
                self.data_device = Some(manager.get_data_device(qh, &seat));
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seat_state.get_pointer(qh, &seat) {
                Ok(pointer) => self.pointer = Some(pointer),
                Err(err) => warn!("pointer unavailable: {err}"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer
            && let Some(pointer) = self.pointer.take()
        {
            pointer.release();
        }
        if capability == Capability::Keyboard
            && let Some(keyboard) = self.keyboard.take()
        {
            keyboard.release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

fn button_mask(button: u32) -> i64 {
    match button {
        BTN_LEFT => 1,
        BTN_RIGHT => 2,
        BTN_MIDDLE => 4,
        _ => 0,
    }
}

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[SeatPointerEvent],
    ) {
        for event in events {
            if &event.surface != self.surface.wl_surface() {
                continue;
            }
            let position = event.position;
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.send_pointer(PointerPhase::Add, position, None);
                }
                PointerEventKind::Leave { .. } => {
                    // With a button pressed (a drag handed over to Ossido) the gesture in
                    // progress is cancelled before removing the pointer.
                    if self.buttons != 0 {
                        self.send_pointer(PointerPhase::Cancel, position, None);
                    }
                    self.buttons = 0;
                    self.send_pointer(PointerPhase::Remove, position, None);
                }
                PointerEventKind::Motion { .. } => {
                    let phase = if self.buttons == 0 {
                        PointerPhase::Hover
                    } else {
                        PointerPhase::Move
                    };
                    self.send_pointer(phase, position, None);
                }
                PointerEventKind::Press { button, serial, .. } => {
                    self.press_serial = serial;
                    let was_up = self.buttons == 0;
                    self.buttons |= button_mask(button);
                    let phase = if was_up {
                        PointerPhase::Down
                    } else {
                        PointerPhase::Move
                    };
                    self.send_pointer(phase, position, None);
                }
                PointerEventKind::Release { button, .. } => {
                    self.buttons &= !button_mask(button);
                    let phase = if self.buttons == 0 {
                        PointerPhase::Up
                    } else {
                        PointerPhase::Move
                    };
                    self.send_pointer(phase, position, None);
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    let phase = if self.buttons == 0 {
                        PointerPhase::Hover
                    } else {
                        PointerPhase::Move
                    };
                    let delta = (horizontal.absolute, vertical.absolute);
                    self.send_pointer(phase, position, Some(delta));
                }
            }
        }
    }
}

impl KeyboardHandler for App {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        // Outside the window releases do not arrive: they are synthesized now.
        let micros = self.now_micros();
        for event in self.keys.release_all(micros) {
            self.send_key(&event, None, None);
        }
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: SeatKeyEvent,
    ) {
        let keysym = event.keysym.raw();
        let utf8 = event.utf8.as_deref();
        let micros = self.now_micros();
        let key = self.keys.press(event.raw_code, keysym, utf8, micros);
        let raw = raw_key_message(true, event.raw_code, keysym, utf8, self.gtk_modifiers());
        let edit = edit_key(
            keysym,
            utf8,
            self.modifiers.shift,
            self.modifiers.ctrl || self.modifiers.alt,
        );
        self.send_key(&key, Some(raw), edit);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: SeatKeyEvent,
    ) {
        let micros = self.now_micros();
        if let Some(key) = self.keys.release(event.raw_code, micros) {
            let raw = raw_key_message(
                false,
                event.raw_code,
                event.keysym.raw(),
                None,
                self.gtk_modifiers(),
            );
            self.send_key(&key, Some(raw), None);
        }
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: u32,
    ) {
        self.modifiers = modifiers;
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl DataDeviceHandler for App {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &WlDataDevice,
        x: f64,
        y: f64,
        surface: &wl_surface::WlSurface,
    ) {
        if surface != self.surface.wl_surface() {
            return;
        }
        let Some(offer) = self
            .data_device
            .as_ref()
            .filter(|d| d.inner() == device)
            .and_then(|d| d.data().drag_offer())
        else {
            return;
        };
        let files = offer.with_mime_types(|mimes| mimes.iter().any(|m| m == crate::drag::URI_LIST));
        if !files {
            offer.accept_mime_type(offer.serial, None);
            return;
        }
        offer.accept_mime_type(offer.serial, Some(crate::drag::URI_LIST.to_owned()));
        offer.set_actions(DndAction::Copy | DndAction::Move, DndAction::Copy);
        self.drag_in = true;
        self.send_drag("enter", serde_json::json!({ "x": x, "y": y }));
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        if std::mem::take(&mut self.drag_in) {
            self.send_drag("leave", serde_json::Value::Null);
        }
    }

    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, x: f64, y: f64) {
        if self.drag_in {
            self.send_drag("motion", serde_json::json!({ "x": x, "y": y }));
        }
    }

    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        if !std::mem::take(&mut self.drag_in) {
            return;
        }
        let Some(offer) = self
            .data_device
            .as_ref()
            .filter(|d| d.inner() == device)
            .and_then(|d| d.data().drag_offer())
        else {
            return;
        };
        let (x, y) = (offer.x, offer.y);
        let pipe = match offer.receive(crate::drag::URI_LIST.to_owned()) {
            Ok(pipe) => pipe,
            Err(err) => {
                warn!("unreadable drop: {err}");
                return;
            }
        };
        let Some(sender) = self.drops.clone() else {
            return;
        };
        self.dropped = Some(offer);
        std::thread::spawn(move || {
            let mut data = Vec::new();
            let mut file = std::fs::File::from(std::os::fd::OwnedFd::from(pipe));
            // At most 16 MB of URIs: beyond that it is truncated.
            let _ = std::io::Read::read_to_end(
                &mut std::io::Read::take(&mut file, 16 << 20),
                &mut data,
            );
            let _ = sender.send((x, y, data));
        });
    }
}

impl DataOfferHandler for App {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }

    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
}

impl DataSourceHandler for App {
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: Option<String>,
    ) {
    }

    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        source: &WlDataSource,
        mime: String,
        mut pipe: WritePipe,
    ) {
        let Some(drag) = self
            .drag_out
            .as_ref()
            .filter(|d| d.source.inner() == source)
        else {
            return;
        };
        if mime != crate::drag::URI_LIST {
            return;
        }
        // On a thread: the reader may be this very window.
        let body = Arc::clone(&drag.body);
        std::thread::spawn(move || {
            if let Err(err) = std::io::Write::write_all(&mut pipe, &body) {
                warn!("drag: data not sent: {err}");
            }
        });
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self
            .drag_out
            .as_ref()
            .is_some_and(|d| d.source.inner() == source)
        {
            self.finish_drag_out(false);
        }
    }

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self
            .drag_out
            .as_ref()
            .is_some_and(|d| d.source.inner() == source)
        {
            self.finish_drag_out(true);
        }
    }

    fn action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        source: &WlDataSource,
        action: DndAction,
    ) {
        if let Some(drag) = self
            .drag_out
            .as_mut()
            .filter(|d| d.source.inner() == source)
        {
            drag.action = action;
        }
    }
}

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_pointer!(App);
delegate_keyboard!(App);
delegate_xdg_shell!(App);
delegate_xdg_window!(App);
delegate_layer!(App);
delegate_registry!(App);
delegate_data_device!(App);
wayland_client::delegate_noop!(App: ignore SilicaGlassManagerV1);
wayland_client::delegate_noop!(App: ignore SilicaWindowManagerV1);
wayland_client::delegate_noop!(App: ignore SilicaWindowV1);
wayland_client::delegate_noop!(App: ignore SilicaGlassV1);
impl wayland_client::Dispatch<SilicaShellManagerV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &SilicaShellManagerV1,
        event: silica_protocols::shell::client::silica_shell_manager_v1::Event,
        _: &(),
        _: &wayland_client::Connection,
        _: &QueueHandle<Self>,
    ) {
        use silica_protocols::shell::client::silica_shell_manager_v1::Event;
        let message = match event {
            Event::Focused { identifier } => app.shell.set_focused(identifier),
            // A catalog command for the shell (e.g. faro.toggle).
            Event::Command { id } => {
                serde_json::to_vec(&serde_json::json!({ "method": "command", "args": id }))
                    .unwrap_or_default()
            }
            Event::Minimized {
                identifier,
                minimized,
            } => app.shell.set_minimized(identifier, minimized != 0),
            Event::Thumbnail {
                identifier,
                width,
                height,
            } => app.shell.thumbnail_ready(&identifier, width, height),
            // The state announced by a shell component (Control Center open…).
            Event::Broadcast { topic, value } => serde_json::to_vec(&serde_json::json!({
                "method": "broadcast",
                "args": { "topic": topic, "value": value },
            }))
            .unwrap_or_default(),
            // Long press on the green button: the arrangement popover.
            Event::WindowMenu { identifier, x, y } => serde_json::to_vec(&serde_json::json!({
                "method": "windowMenu",
                "args": { "id": identifier, "x": x, "y": y },
            }))
            .unwrap_or_default(),
            _ => return,
        };
        if let Some(engine) = &app.engine
            && let Err(err) = engine.send_message(shell::CHANNEL, &message)
        {
            warn!("shell event to Dart: {err}");
        }
    }
}

impl wayland_client::Dispatch<ExtForeignToplevelListV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &wayland_client::Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            app.shell.add(toplevel);
        }
    }

    wayland_client::event_created_child!(App, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl wayland_client::Dispatch<ExtForeignToplevelHandleV1, ()> for App {
    fn event(
        app: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &wayland_client::Connection,
        _: &QueueHandle<Self>,
    ) {
        if app.shell.event(handle, event)
            && let Some(engine) = &app.engine
            && let Err(err) = engine.send_message(shell::CHANNEL, &app.shell.toplevels_message())
        {
            warn!("window list to Dart: {err}");
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::lifecycle_message;

    #[test]
    fn the_active_window_is_resumed_the_others_inactive() {
        assert_eq!(lifecycle_message(true), "AppLifecycleState.resumed");
        assert_eq!(lifecycle_message(false), "AppLifecycleState.inactive");
    }
}
