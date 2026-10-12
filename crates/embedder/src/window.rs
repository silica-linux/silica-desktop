//! La finestra Wayland che ospita l'engine, e il ciclo del thread di piattaforma.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Duration;

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop::EventLoop;
use smithay_client_toolkit::reexports::calloop::channel::{self, Channel, Event};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{
    PointerEvent as SeatPointerEvent, PointerEventKind, PointerHandler,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::xdg::window::{
    Window, WindowConfigure, WindowDecorations, WindowHandler,
};
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_output, delegate_pointer, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window, registry_handlers,
};
use tracing::{info, warn};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface};
use wayland_client::{Connection, QueueHandle};

use crate::Options;
use crate::ffi::{Callbacks, Engine, EngineConfig, Frame, PointerEvent, PointerPhase, Task};

/// Codici dei pulsanti del mouse (linux/input-event-codes.h) e maschere di Flutter.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

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
    window: Window,
    pointer: Option<wl_pointer::WlPointer>,
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
    exit: bool,
}

pub fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    for path in [&options.library, &options.icu_data] {
        if !path.exists() {
            return Err(format!("{} assente: esegui `cargo xtask engine`", path.display()).into());
        }
    }
    if !options.bundle.join("kernel_blob.bin").exists() {
        return Err(format!(
            "{} non è un bundle Flutter (manca kernel_blob.bin): esegui `cargo xtask bundle <app>`",
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
    let xdg_shell = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let surface = compositor.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_title("Silica");
    window.set_app_id("dev.frascella.silica.Flutter");
    window.commit();

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
    };
    let engine = Engine::run(
        &EngineConfig {
            library: &options.library,
            assets: &options.bundle,
            icu_data: &options.icu_data,
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
        window,
        pointer: None,
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

    /// Presenta l'ultimo frame dell'engine, se il compositor è pronto a riceverne uno.
    fn present(&mut self, qh: &QueueHandle<Self>) {
        if self.waiting_frame || !self.configured {
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

        let surface = self.window.wl_surface();
        surface.set_buffer_scale(self.scale.max(1));
        surface.damage_buffer(0, 0, width, height);
        surface.frame(qh, surface.clone());
        if let Err(err) = buffer.attach_to(surface) {
            warn!("attach del buffer fallito: {err}");
            return;
        }
        self.window.commit();
        self.buffer = Some(buffer);
        self.waiting_frame = true;
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
        if let (Some(w), Some(h)) = configure.new_size {
            self.size = (w.get(), h.get());
        }
        self.configured = true;
        self.send_metrics();
        self.present(qh);
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
            if &event.surface != self.window.wl_surface() {
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
delegate_xdg_shell!(App);
delegate_xdg_window!(App);
delegate_registry!(App);
