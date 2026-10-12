//! FFI verso `libflutter_engine.so`: l'unico modulo del workspace con `unsafe`
//! (AGENTS.md, "Mai crashare"; verificato da `cargo xtask unsafe`).
//!
//! Il resto dell'embedder vede solo tipi sicuri: [`Engine`], [`Task`], [`Frame`],
//! [`PointerEvent`]. La libreria si carica a runtime, così il workspace compila e i
//! test girano anche dove l'engine non è stato scaricato.

#![allow(unsafe_code)]

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    clippy::all,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::undocumented_unsafe_blocks,
    clippy::unwrap_used
)]
mod bindings;

use std::ffi::{CStr, CString, c_char, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::thread::ThreadId;

use bindings::{
    FLUTTER_ENGINE_VERSION, FlutterCustomTaskRunners, FlutterEngine, FlutterEngineLib,
    FlutterEngineResult, FlutterPointerDeviceKind, FlutterPointerEvent as RawPointerEvent,
    FlutterPointerPhase, FlutterPointerSignalKind, FlutterProjectArgs, FlutterRendererConfig,
    FlutterRendererType, FlutterSoftwareRendererConfig, FlutterTask, FlutterTaskRunnerDescription,
    FlutterWindowMetricsEvent,
};

#[derive(Debug)]
pub enum Error {
    Load(String),
    Symbol(&'static str, String),
    Path(String),
    Engine(&'static str, i32),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Load(err) => write!(f, "libflutter_engine.so non caricabile: {err}"),
            Error::Symbol(name, err) => write!(f, "simbolo {name} assente: {err}"),
            Error::Path(path) => write!(f, "percorso non valido: {path}"),
            Error::Engine(call, code) => write!(f, "{call} ha restituito l'errore {code}"),
        }
    }
}

impl std::error::Error for Error {}

fn check(call: &'static str, result: FlutterEngineResult) -> Result<(), Error> {
    if result == FlutterEngineResult::kSuccess {
        Ok(())
    } else {
        Err(Error::Engine(call, result.0 as i32))
    }
}

/// Un task che l'engine chiede di eseguire sul thread di piattaforma.
pub struct Task {
    inner: FlutterTask,
    /// Quando eseguirlo, nella base di [`Engine::current_time`] (nanosecondi).
    pub target_nanos: u64,
}

// SAFETY: FlutterTask è un handle opaco (puntatore al runner + id) che l'engine passa
// da un thread qualsiasi apposta perché venga eseguito su quello di piattaforma; il
// contratto di FlutterTaskRunnerPostTaskCallback consente di spostarlo tra thread.
unsafe impl Send for Task {}

/// Un frame del renderer software: righe di `stride` byte, pixel a 32 bit nel formato
/// nativo dell'engine (BGRA in memoria su Linux little-endian).
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub pixels: Vec<u8>,
}

/// Le funzioni dell'embedder chiamate dall'engine, da thread qualsiasi.
pub struct Callbacks {
    pub platform_thread: ThreadId,
    pub post_task: Box<dyn Fn(Task) + Send + Sync>,
    pub present: Box<dyn Fn(Frame) + Send + Sync>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerPhase {
    Add,
    Remove,
    Hover,
    Down,
    Move,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    pub phase: PointerPhase,
    /// Posizione in pixel fisici.
    pub x: f64,
    pub y: f64,
    /// Maschera dei pulsanti premuti: 1 primario, 2 secondario, 4 centrale.
    pub buttons: i64,
    /// Scorrimento in pixel fisici, se l'evento è una rotella.
    pub scroll: Option<(f64, f64)>,
    pub timestamp_micros: u64,
}

type RunFn = unsafe extern "C" fn(
    usize,
    *const FlutterRendererConfig,
    *const FlutterProjectArgs,
    *mut c_void,
    *mut FlutterEngine,
) -> FlutterEngineResult;
type ShutdownFn = unsafe extern "C" fn(FlutterEngine) -> FlutterEngineResult;
type RunTaskFn = unsafe extern "C" fn(FlutterEngine, *const FlutterTask) -> FlutterEngineResult;
type MetricsFn =
    unsafe extern "C" fn(FlutterEngine, *const FlutterWindowMetricsEvent) -> FlutterEngineResult;
type PointerFn =
    unsafe extern "C" fn(FlutterEngine, *const RawPointerEvent, usize) -> FlutterEngineResult;
type TimeFn = unsafe extern "C" fn() -> u64;

/// Le funzioni dell'engine usate qui, risolte una volta al caricamento.
struct Api {
    run: RunFn,
    shutdown: ShutdownFn,
    run_task: RunTaskFn,
    metrics: MetricsFn,
    pointer: PointerFn,
    time: TimeFn,
}

macro_rules! symbol {
    ($lib:expr, $name:ident) => {
        *$lib
            .$name
            .as_ref()
            .map_err(|err| Error::Symbol(stringify!($name), err.to_string()))?
    };
}

pub struct EngineConfig<'a> {
    pub library: &'a Path,
    pub assets: &'a Path,
    pub icu_data: &'a Path,
}

/// Un engine Flutter in esecuzione. Si usa e si distrugge sul thread di piattaforma.
pub struct Engine {
    api: Api,
    engine: FlutterEngine,
    // Tutto ciò a cui l'engine tiene puntatori: deve vivere quanto lui.
    _callbacks: Box<Callbacks>,
    _runner: Box<FlutterTaskRunnerDescription>,
    _runners: Box<FlutterCustomTaskRunners>,
    _strings: Vec<CString>,
    _argv: Vec<*const c_char>,
    _library: FlutterEngineLib,
}

fn c_path(path: &Path) -> Result<CString, Error> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Path(path.display().to_string()))
}

/// Il puntatore a [`Callbacks`] passato all'engine come `user_data`.
///
/// # Safety
/// `user_data` deve essere il puntatore a `Callbacks` dato a [`Engine::run`], ancora vivo.
unsafe fn callbacks<'a>(user_data: *mut c_void) -> &'a Callbacks {
    // SAFETY: garantito dal chiamante; Engine tiene il Box finché l'engine non è spento,
    // e dopo FlutterEngineShutdown l'engine non chiama più i callback.
    unsafe { &*user_data.cast_const().cast::<Callbacks>() }
}

unsafe extern "C" fn present(
    user_data: *mut c_void,
    allocation: *const c_void,
    row_bytes: usize,
    height: usize,
) -> bool {
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    let Some(len) = row_bytes.checked_mul(height) else {
        return false;
    };
    if allocation.is_null() || len == 0 {
        return false;
    }
    // SAFETY: per SoftwareSurfacePresentCallback il buffer ha row_bytes * height byte
    // validi per tutta la durata della chiamata; lo copiamo prima di tornare.
    let pixels = unsafe { std::slice::from_raw_parts(allocation.cast::<u8>(), len) }.to_vec();
    let (Ok(width), Ok(height)) = (u32::try_from(row_bytes / 4), u32::try_from(height)) else {
        return false;
    };
    (callbacks.present)(Frame {
        width,
        height,
        stride: row_bytes,
        pixels,
    });
    true
}

unsafe extern "C" fn post_task(task: FlutterTask, target_nanos: u64, user_data: *mut c_void) {
    // SAFETY: user_data è quello del FlutterTaskRunnerDescription (vedi `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    (callbacks.post_task)(Task {
        inner: task,
        target_nanos,
    });
}

unsafe extern "C" fn runs_on_current_thread(user_data: *mut c_void) -> bool {
    // SAFETY: user_data è quello del FlutterTaskRunnerDescription (vedi `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    callbacks.platform_thread == std::thread::current().id()
}

unsafe extern "C" fn log_message(tag: *const c_char, message: *const c_char, _: *mut c_void) {
    let text = |ptr: *const c_char| {
        if ptr.is_null() {
            String::new()
        } else {
            // SAFETY: l'engine passa stringhe C terminate da NUL, valide durante la chiamata.
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        }
    };
    tracing::info!(target: "flutter", "{}: {}", text(tag), text(message));
}

impl Engine {
    /// Carica l'engine e avvia l'app del bundle. Va chiamato sul thread di piattaforma,
    /// lo stesso di `callbacks.platform_thread`.
    pub fn run(config: &EngineConfig<'_>, callbacks: Callbacks) -> Result<Self, Error> {
        // SAFETY: carica la libreria ufficiale dell'engine alla versione fissata in
        // ENGINE_VERSION, la stessa dell'header da cui vengono i binding; i suoi
        // inizializzatori non hanno precondizioni.
        let library = unsafe { FlutterEngineLib::new(config.library) }
            .map_err(|e| Error::Load(e.to_string()))?;
        let api = Api {
            run: symbol!(library, FlutterEngineRun),
            shutdown: symbol!(library, FlutterEngineShutdown),
            run_task: symbol!(library, FlutterEngineRunTask),
            metrics: symbol!(library, FlutterEngineSendWindowMetricsEvent),
            pointer: symbol!(library, FlutterEngineSendPointerEvent),
            time: symbol!(library, FlutterEngineGetCurrentTime),
        };

        let callbacks = Box::new(callbacks);
        let user_data = ptr::from_ref::<Callbacks>(&callbacks)
            .cast_mut()
            .cast::<c_void>();
        let runner = Box::new(FlutterTaskRunnerDescription {
            struct_size: size_of::<FlutterTaskRunnerDescription>(),
            user_data,
            runs_task_on_current_thread_callback: Some(runs_on_current_thread),
            post_task_callback: Some(post_task),
            identifier: 1,
            ..Default::default()
        });
        let runners = Box::new(FlutterCustomTaskRunners {
            struct_size: size_of::<FlutterCustomTaskRunners>(),
            platform_task_runner: ptr::from_ref(&*runner),
            ..Default::default()
        });
        let mut renderer = FlutterRendererConfig {
            type_: FlutterRendererType::kSoftware,
            ..Default::default()
        };
        renderer.__bindgen_anon_1.software = FlutterSoftwareRendererConfig {
            struct_size: size_of::<FlutterSoftwareRendererConfig>(),
            surface_present_callback: Some(present),
        };

        let strings = vec![
            c_path(config.assets)?,
            c_path(config.icu_data)?,
            CString::from(c"silica-embedder"),
        ];
        let argv: Vec<*const c_char> = strings.iter().skip(2).map(|s| s.as_ptr()).collect();
        let (Some(assets), Some(icu)) = (strings.first(), strings.get(1)) else {
            return Err(Error::Path("argomenti mancanti".to_owned()));
        };
        let args = FlutterProjectArgs {
            struct_size: size_of::<FlutterProjectArgs>(),
            assets_path: assets.as_ptr(),
            icu_data_path: icu.as_ptr(),
            command_line_argc: i32::try_from(argv.len()).unwrap_or(0),
            command_line_argv: argv.as_ptr(),
            custom_task_runners: ptr::from_ref(&*runners),
            log_message_callback: Some(log_message),
            shutdown_dart_vm_when_done: true,
            ..Default::default()
        };

        let mut engine: FlutterEngine = ptr::null_mut();
        // SAFETY: renderer e args sono struct inizializzate con struct_size corretto; i
        // puntatori che contengono (stringhe, argv, runner, callbacks) restano vivi in
        // Engine finché l'engine non viene spento in Drop.
        let result = unsafe {
            (api.run)(
                FLUTTER_ENGINE_VERSION as usize,
                &renderer,
                &args,
                user_data,
                &mut engine,
            )
        };
        check("FlutterEngineRun", result)?;
        if engine.is_null() {
            return Err(Error::Engine("FlutterEngineRun", -1));
        }
        Ok(Self {
            api,
            engine,
            _callbacks: callbacks,
            _runner: runner,
            _runners: runners,
            _strings: strings,
            _argv: argv,
            _library: library,
        })
    }

    /// Orologio monotono dell'engine, in nanosecondi.
    pub fn current_time(&self) -> u64 {
        // SAFETY: funzione senza argomenti né precondizioni.
        unsafe { (self.api.time)() }
    }

    /// Esegue un task ricevuto da `post_task`. Solo sul thread di piattaforma.
    pub fn run_task(&self, task: &Task) -> Result<(), Error> {
        // SAFETY: engine è vivo; task viene da post_task di questo engine e ogni task
        // si esegue una volta sola (il chiamante lo consuma dalla coda).
        check("FlutterEngineRunTask", unsafe {
            (self.api.run_task)(self.engine, &task.inner)
        })
    }

    /// Dimensione della vista in pixel fisici e rapporto pixel fisici / logici.
    pub fn send_window_metrics(
        &self,
        width: u32,
        height: u32,
        pixel_ratio: f64,
    ) -> Result<(), Error> {
        let event = FlutterWindowMetricsEvent {
            struct_size: size_of::<FlutterWindowMetricsEvent>(),
            width: width as usize,
            height: height as usize,
            pixel_ratio,
            ..Default::default()
        };
        // SAFETY: engine è vivo; event è inizializzato e vive per la durata della chiamata.
        check("FlutterEngineSendWindowMetricsEvent", unsafe {
            (self.api.metrics)(self.engine, &event)
        })
    }

    pub fn send_pointer(&self, event: &PointerEvent) -> Result<(), Error> {
        let phase = match event.phase {
            PointerPhase::Add => FlutterPointerPhase::kAdd,
            PointerPhase::Remove => FlutterPointerPhase::kRemove,
            PointerPhase::Hover => FlutterPointerPhase::kHover,
            PointerPhase::Down => FlutterPointerPhase::kDown,
            PointerPhase::Move => FlutterPointerPhase::kMove,
            PointerPhase::Up => FlutterPointerPhase::kUp,
        };
        let (signal_kind, (scroll_delta_x, scroll_delta_y)) = match event.scroll {
            Some(delta) => (
                FlutterPointerSignalKind::kFlutterPointerSignalKindScroll,
                delta,
            ),
            None => (
                FlutterPointerSignalKind::kFlutterPointerSignalKindNone,
                (0.0, 0.0),
            ),
        };
        let raw = RawPointerEvent {
            struct_size: size_of::<RawPointerEvent>(),
            phase,
            timestamp: usize::try_from(event.timestamp_micros).unwrap_or(usize::MAX),
            x: event.x,
            y: event.y,
            signal_kind,
            scroll_delta_x,
            scroll_delta_y,
            device_kind: FlutterPointerDeviceKind::kFlutterPointerDeviceKindMouse,
            buttons: event.buttons,
            ..Default::default()
        };
        // SAFETY: engine è vivo; raw è un array di un elemento inizializzato.
        check("FlutterEngineSendPointerEvent", unsafe {
            (self.api.pointer)(self.engine, &raw, 1)
        })
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: engine è vivo e non più usato dopo; i campi che l'engine referenzia
        // vengono distrutti solo dopo questa chiamata (ordine dei campi in Engine).
        let result = unsafe { (self.api.shutdown)(self.engine) };
        if let Err(err) = check("FlutterEngineShutdown", result) {
            tracing::warn!("{err}");
        }
    }
}
