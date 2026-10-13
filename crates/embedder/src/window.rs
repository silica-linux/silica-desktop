//! La finestra Wayland che ospita l'engine, e il ciclo del thread di piattaforma.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use silica_protocols::glass::client::silica_glass_manager_v1::SilicaGlassManagerV1;
use silica_protocols::glass::client::silica_glass_v1::SilicaGlassV1;
use silica_protocols::shell::client::silica_shell_manager_v1::SilicaShellManagerV1;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop::EventLoop;
use smithay_client_toolkit::reexports::calloop::channel::{self, Channel, Event};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
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
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    registry_handlers,
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

/// La superficie che ospita l'app: una finestra normale o un layer della shell.
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

    /// Il titolo della finestra; i layer della shell non ne hanno.
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

/// Il canale per regione d'input e tastiera di un layer (componenti della shell).
const LAYER_CHANNEL: &str = "silica/layer";
use crate::text_input::{self, EditKey, TextInput};

/// Esito di un tasto mandato al framework, con l'azione di editing da fare se ignorato.
struct KeyResult {
    handled: bool,
    edit: Option<EditKey>,
}

/// Raccoglie le risposte delle due strade di un tasto; all'ultima manda il risultato.
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

/// Codici dei pulsanti del mouse (linux/input-event-codes.h) e maschere di Flutter.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

/// Periodo dei frame dato all'engine con ogni vsync: 60 Hz.
const FRAME_PERIOD_NANOS: u64 = 16_666_667;

/// Intervallo massimo tra due giri del ciclo quando non c'è alcun task in coda.
const IDLE_WAKEUP: Duration = Duration::from_millis(100);

/// Un task in coda, ordinato per scadenza e poi per ordine di arrivo.
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
    /// Regioni di vetro dei widget `Glass` (`silica/glass`).
    glass: Glass,
    /// Finestre e comandi per i componenti della shell (`silica/shell`).
    shell: ShellLink,
    /// Per le regioni d'input (`silica/layer`).
    compositor: CompositorState,
    /// Il canale `silica/dbus`, su un thread a parte.
    dbus: Option<dbus::Bridge<ResponseHandle>>,
    qh: QueueHandle<App>,
    key_results: channel::Sender<KeyResult>,
    engine: Option<Engine>,

    tasks: BinaryHeap<Reverse<Queued>>,
    next_seq: u64,

    /// Dimensione in pixel logici e fattore di scala della superficie.
    size: (u32, u32),
    scale: i32,
    configured: bool,
    /// L'ultimo frame dell'engine non ancora presentato.
    pending: Option<Frame>,
    /// Si aspetta il frame callback del compositor prima di presentare ancora.
    waiting_frame: bool,
    buffer: Option<Buffer>,
    buttons: i64,
    /// Con OpenGL lo swap dell'engine fa attach e commit: qui si cura solo la scala.
    gl: bool,
    /// Il software ha presentato almeno un frame (con OpenGL lo sa la superficie EGL).
    presented: bool,
    /// La richiesta di vsync dell'engine in attesa del frame callback del compositor.
    vsync: Option<VsyncBaton>,
    /// Un frame callback chiesto per il vsync e non ancora arrivato.
    vsync_requested: bool,
    exit: bool,
}

pub fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    for path in [&options.library, &options.icu_data] {
        if !path.exists() {
            return Err(format!("{} assente: esegui `cargo xtask engine`", path.display()).into());
        }
    }
    // Un bundle debug porta il kernel Dart, uno release il manifest degli asset e libapp.so.
    if !options.bundle.join("kernel_blob.bin").exists()
        && !options.bundle.join("AssetManifest.bin").exists()
    {
        return Err(format!(
            "{} non è un bundle Flutter: esegui `cargo xtask bundle <app>`",
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
        .map_err(|e| format!("sorgente Wayland: {e}"))?;

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
            // Una dimensione ancorata a entrambi i lati la decide il compositor (0).
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
    // Vulkan con Impeller; se Vulkan non c'è (o non presenta su questa superficie) si
    // ripiega su OpenGL ES con Impeller.
    let vulkan = match options.renderer {
        Renderer::Vulkan => match VulkanSurface::new(&conn, surface.wl_surface(), options.size) {
            Ok(vulkan) => {
                info!("renderer Vulkan: {}", vulkan.renderer());
                engine_args.push("--enable-impeller".to_owned());
                Some(vulkan)
            }
            Err(err) => {
                warn!("Vulkan non disponibile ({err}): uso OpenGL ES con Impeller");
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
                    warn!("Impeller su un driver GL software va in crash: uso Skia");
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
                .map_err(|e| format!("canale dei ridisegni: {e}"))?;
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

    // Task e frame arrivano dai thread dell'engine: due canali verso questo thread.
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
        .map_err(|e| format!("canale dei task: {e}"))?;
    let (message_sender, message_channel) = channel::channel::<PlatformMessage>();
    event_loop
        .handle()
        .insert_source(message_channel, |event, _, app: &mut App| {
            if let Event::Msg(message) = event {
                app.handle_message(message);
            }
        })
        .map_err(|e| format!("canale dei messaggi: {e}"))?;
    let (dbus_sender, dbus_channel) = channel::channel::<dbus::Event<ResponseHandle>>();
    event_loop
        .handle()
        .insert_source(dbus_channel, |event, _, app: &mut App| {
            if let Event::Msg(event) = event {
                app.handle_dbus(event);
            }
        })
        .map_err(|e| format!("canale D-Bus: {e}"))?;
    let dbus = dbus::Bridge::start(dbus::Addresses::default(), move |event| {
        let _ = dbus_sender.send(event);
    })
    .map_err(|err| warn!("silica/dbus non disponibile: {err}"))
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
        .map_err(|e| format!("canale del vsync: {e}"))?;
    let (key_results, key_channel) = channel::channel::<KeyResult>();
    event_loop
        .handle()
        .insert_source(key_channel, |event, _, app: &mut App| {
            if let Event::Msg(result) = event {
                app.key_result(result);
            }
        })
        .map_err(|e| format!("canale dei tasti: {e}"))?;
    let frame_qh = qh.clone();
    event_loop
        .handle()
        .insert_source(frame_channel, move |event, _, app: &mut App| {
            if let Event::Msg(frame) = event {
                app.pending = Some(frame);
                app.present(&frame_qh);
            }
        })
        .map_err(|e| format!("canale dei frame: {e}"))?;

    let callbacks = Callbacks {
        platform_thread: std::thread::current().id(),
        post_task: Box::new(move |task| {
            if task_sender.send(task).is_err() {
                warn!("task scartato: il ciclo principale è chiuso");
            }
        }),
        present: Box::new(move |frame| {
            let _ = frame_sender.send(frame);
        }),
        platform_message: Box::new(move |message| {
            // Se il ciclo è chiuso il messaggio cade con il suo handle: l'engine si sta
            // spegnendo e non aspetta più risposte.
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
    info!("engine avviato con {}", options.bundle.display());

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
        glass: Glass::new(
            globals
                .bind::<SilicaGlassManagerV1, App, ()>(&qh, 1..=3, ())
                .ok(),
        ),
        shell: ShellLink::new(
            globals
                .bind::<SilicaShellManagerV1, App, ()>(&qh, 1..=2, ())
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

    while !app.exit {
        let timeout = app.run_due_tasks();
        event_loop.dispatch(Some(timeout), &mut app)?;
    }
    // Spegne l'engine prima di chiudere la connessione Wayland.
    app.engine = None;
    Ok(())
}

impl App {
    /// Risponde a un messaggio del framework. Ogni messaggio con handle riceve una
    /// risposta, vuota se il canale non è gestito.
    fn handle_message(&mut self, message: PlatformMessage) {
        // silica/dbus risponde più tardi, dal suo thread (`handle_dbus`).
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
            glass::CHANNEL => {
                let (reply, changed) =
                    self.glass
                        .handle_call(&message.data, self.surface.wl_surface(), &self.qh);
                // Le regioni valgono dal commit: senza un frame nuovo serve qui.
                if changed {
                    self.surface.commit();
                }
                reply
            }
            "flutter/platform" => self.handle_platform_call(&message.data),
            // StandardMethodCodec: getKeyboardState, i tasti premuti fisica → logica.
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
            warn!("risposta su {}: {err}", message.channel);
        }
    }

    /// Il canale `flutter/platform` (JSONMethodCodec): per ora il titolo della finestra
    /// (`SystemChrome.setApplicationSwitcherDescription`, `label`). Il resto non è
    /// implementato (risposta vuota).
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
                // Un titolo lunghissimo da un'app non deve diventare un problema per Ossido.
                let title: String = label.chars().take(256).collect();
                self.surface.set_title(&title);
                self.surface.commit();
                b"[null]".to_vec()
            }
            _ => Vec::new(),
        }
    }

    /// Un esito del thread D-Bus: la risposta a una chiamata o un segnale per Dart.
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

    /// Il canale `silica/layer`: un componente della shell nascosto non deve prendere
    /// puntatore né tastiera (Faro chiuso resta in esecuzione, trasparente).
    /// - `setKeyboardInteractivity {mode: none|exclusive|on-demand}`;
    /// - `setInputRegion {rects: [[x, y, w, h], …] | null}` (null: tutta la superficie);
    /// - `outputSize` → `{width, height}` dello schermo in punti (null se non si sa);
    /// - `setSize {width, height}` in punti, 0 per seguire gli ancoraggi (la barra dei menu si
    ///   allunga finché un menu è aperto).
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
                // La dimensione logica dello schermo (il primo output): per centrare un avviso
                // in una superficie che si allunga fino a coprirlo.
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
                        warn!("regione d'input: {err}");
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

    /// Manda un tasto al framework per le due strade che si aspetta (dato tasto e
    /// messaggio `flutter/keyevent`); se nessuna delle due lo consuma, l'azione di
    /// editing va al campo di testo.
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

    /// Un tasto non consumato dal framework va al campo di testo attivo.
    fn key_result(&mut self, result: KeyResult) {
        tracing::debug!(
            "tasto gestito dal framework: {}, editing: {:?}",
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

    /// Esegue i task scaduti e dice quanto aspettare il prossimo.
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

    /// L'engine vuole il prossimo vsync: arriva con il frame callback del compositor.
    /// Prima del primo frame il compositor non ne manda, quindi si risponde subito.
    fn request_vsync(&mut self, baton: VsyncBaton, qh: &QueueHandle<Self>) {
        let presented =
            self.presented || (self.gl && self.engine.as_ref().is_some_and(Engine::gl_presented));
        if !presented || !self.configured {
            self.answer_vsync(baton);
            return;
        }
        self.vsync = Some(baton);
        // Un frame callback già chiesto (dal vsync o dalla presentazione) basta.
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

    /// Presenta l'ultimo frame dell'engine, se il compositor è pronto a riceverne uno.
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
                    warn!("buffer shm non creato: {err}");
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
            warn!("attach del buffer fallito: {err}");
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
                // Vale dal prossimo commit, cioè dallo swap del frame alla nuova dimensione.
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
                Err(err) => warn!("tastiera non disponibile: {err}"),
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seat_state.get_pointer(qh, &seat) {
                Ok(pointer) => self.pointer = Some(pointer),
                Err(err) => warn!("puntatore non disponibile: {err}"),
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
                PointerEventKind::Press { button, .. } => {
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
        // Fuori dalla finestra i rilasci non arrivano: li si sintetizza ora.
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
wayland_client::delegate_noop!(App: ignore SilicaGlassManagerV1);
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
            // Un comando del catalogo per la shell (es. faro.toggle).
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
            _ => return,
        };
        if let Some(engine) = &app.engine
            && let Err(err) = engine.send_message(shell::CHANNEL, &message)
        {
            warn!("evento della shell a Dart: {err}");
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
            warn!("elenco delle finestre a Dart: {err}");
        }
    }
}
