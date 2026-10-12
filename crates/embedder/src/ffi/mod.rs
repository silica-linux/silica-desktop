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
mod gl;

pub use gl::GlSurface;

use std::ffi::{CStr, CString, c_char, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::thread::ThreadId;

use bindings::{
    FLUTTER_ENGINE_VERSION, FlutterCustomTaskRunners, FlutterEngine, FlutterEngineAOTData,
    FlutterEngineAOTDataSource, FlutterEngineAOTDataSourceType, FlutterEngineLib,
    FlutterEngineResult, FlutterFrameInfo, FlutterKeyEvent as RawKeyEvent,
    FlutterKeyEventDeviceType, FlutterKeyEventType, FlutterOpenGLRendererConfig,
    FlutterPlatformMessage, FlutterPlatformMessageResponseHandle, FlutterPointerDeviceKind,
    FlutterPointerEvent as RawPointerEvent, FlutterPointerPhase, FlutterPointerSignalKind,
    FlutterProjectArgs, FlutterRendererConfig, FlutterRendererType, FlutterSoftwareRendererConfig,
    FlutterTask, FlutterTaskRunnerDescription, FlutterWindowMetricsEvent,
};

#[derive(Debug)]
pub enum Error {
    Load(String),
    Symbol(&'static str, String),
    Path(String),
    Engine(&'static str, i32),
    Gl(&'static str, String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Load(err) => write!(f, "libflutter_engine.so non caricabile: {err}"),
            Error::Symbol(name, err) => write!(f, "simbolo {name} assente: {err}"),
            Error::Path(path) => write!(f, "percorso non valido: {path}"),
            Error::Engine(call, code) => write!(f, "{call} ha restituito l'errore {code}"),
            Error::Gl(call, err) => write!(f, "{call}: {err}"),
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

/// La risposta dovuta a un messaggio di piattaforma: va consumata con
/// [`Engine::respond`] esattamente una volta, altrimenti il Future Dart non si chiude.
pub struct ResponseHandle(*const FlutterPlatformMessageResponseHandle);

// SAFETY: l'handle è opaco e l'engine accetta FlutterEngineSendPlatformMessageResponse
// da qualsiasi thread; qui lo si usa comunque solo sul thread di piattaforma.
unsafe impl Send for ResponseHandle {}

/// Un messaggio dal framework Dart verso l'embedder.
pub struct PlatformMessage {
    pub channel: String,
    pub data: Vec<u8>,
    /// Assente per i messaggi che non aspettano risposta.
    pub response: Option<ResponseHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPhase {
    Down,
    Up,
    Repeat,
}

/// Un evento tasto per il `HardwareKeyboard` di Flutter.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyEvent {
    pub phase: KeyPhase,
    /// Chiave fisica di Flutter (pagina USB HID).
    pub physical: u64,
    /// Chiave logica di Flutter.
    pub logical: u64,
    /// Il testo prodotto dal tasto, solo per `Down` e `Repeat`.
    pub character: Option<String>,
    /// Evento inventato dall'embedder per riallineare lo stato dei tasti.
    pub synthesized: bool,
    pub timestamp_micros: u64,
}

/// La richiesta di vsync dell'engine: va restituita con [`Engine::on_vsync`].
pub struct VsyncBaton(isize);

/// Le funzioni dell'embedder chiamate dall'engine, da thread qualsiasi.
pub struct Callbacks {
    pub platform_thread: ThreadId,
    pub post_task: Box<dyn Fn(Task) + Send + Sync>,
    /// Frame del renderer software; inutile con `gl`.
    pub present: Box<dyn Fn(Frame) + Send + Sync>,
    pub platform_message: Box<dyn Fn(PlatformMessage) + Send + Sync>,
    /// Con una superficie EGL l'engine disegna con OpenGL ES, altrimenti in software.
    pub gl: Option<GlSurface>,
    /// Il ritmo dei frame lo dà l'embedder (frame callback del compositor); senza, l'engine
    /// usa un suo timer a 60 Hz, scollegato dal compositor.
    pub vsync: Option<Box<dyn Fn(VsyncBaton) + Send + Sync>>,
}

/// Chi aspetta di sapere se il framework ha gestito un tasto.
type KeyHandled = Box<dyn FnOnce(bool) + Send>;
/// Chi aspetta la risposta del framework a un messaggio.
type Reply = Box<dyn FnOnce(Vec<u8>) + Send>;

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
type KeyFn = unsafe extern "C" fn(
    FlutterEngine,
    *const RawKeyEvent,
    bindings::FlutterKeyEventCallback,
    *mut c_void,
) -> FlutterEngineResult;
type SendMessageFn =
    unsafe extern "C" fn(FlutterEngine, *const FlutterPlatformMessage) -> FlutterEngineResult;
type CreateReplyFn = unsafe extern "C" fn(
    FlutterEngine,
    bindings::FlutterDataCallback,
    *mut c_void,
    *mut *mut FlutterPlatformMessageResponseHandle,
) -> FlutterEngineResult;
type ReleaseReplyFn = unsafe extern "C" fn(
    FlutterEngine,
    *mut FlutterPlatformMessageResponseHandle,
) -> FlutterEngineResult;
type RespondFn = unsafe extern "C" fn(
    FlutterEngine,
    *const FlutterPlatformMessageResponseHandle,
    *const u8,
    usize,
) -> FlutterEngineResult;

type RunsAotFn = unsafe extern "C" fn() -> bool;
type ScheduleFrameFn = unsafe extern "C" fn(FlutterEngine) -> FlutterEngineResult;
type OnVsyncFn = unsafe extern "C" fn(FlutterEngine, isize, u64, u64) -> FlutterEngineResult;
type CreateAotFn = unsafe extern "C" fn(
    *const FlutterEngineAOTDataSource,
    *mut FlutterEngineAOTData,
) -> FlutterEngineResult;
type CollectAotFn = unsafe extern "C" fn(FlutterEngineAOTData) -> FlutterEngineResult;

/// Le funzioni dell'engine usate qui, risolte una volta al caricamento.
struct Api {
    schedule_frame: ScheduleFrameFn,
    on_vsync: OnVsyncFn,
    runs_aot: RunsAotFn,
    create_aot: CreateAotFn,
    collect_aot: CollectAotFn,
    run: RunFn,
    shutdown: ShutdownFn,
    run_task: RunTaskFn,
    metrics: MetricsFn,
    pointer: PointerFn,
    time: TimeFn,
    key: KeyFn,
    send_message: SendMessageFn,
    respond: RespondFn,
    create_reply: CreateReplyFn,
    release_reply: ReleaseReplyFn,
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
    /// Il codice Dart compilato AOT (`libapp.so`), per un engine release o profile.
    pub aot: Option<&'a Path>,
    /// Opzioni da riga di comando per l'engine (per esempio `--enable-impeller`).
    pub engine_args: &'a [String],
}

/// Un engine Flutter in esecuzione. Si usa e si distrugge sul thread di piattaforma.
pub struct Engine {
    api: Api,
    engine: FlutterEngine,
    /// Nullo con un engine debug (JIT); si libera dopo lo spegnimento.
    aot_data: FlutterEngineAOTData,
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

/// La superficie EGL dei callback, se l'engine usa il renderer OpenGL.
///
/// # Safety
/// Come [`callbacks`].
unsafe fn gl_surface<'a>(user_data: *mut c_void) -> Option<&'a GlSurface> {
    // SAFETY: garantito dal chiamante.
    unsafe { callbacks(user_data) }.gl.as_ref()
}

unsafe extern "C" fn gl_make_current(user_data: *mut c_void) -> bool {
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    unsafe { gl_surface(user_data) }.is_some_and(GlSurface::make_current)
}

unsafe extern "C" fn gl_clear_current(user_data: *mut c_void) -> bool {
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    unsafe { gl_surface(user_data) }.is_some_and(GlSurface::clear_current)
}

unsafe extern "C" fn gl_make_resource_current(user_data: *mut c_void) -> bool {
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    unsafe { gl_surface(user_data) }.is_some_and(GlSurface::make_resource_current)
}

unsafe extern "C" fn gl_present(user_data: *mut c_void) -> bool {
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    unsafe { gl_surface(user_data) }.is_some_and(GlSurface::present)
}

unsafe extern "C" fn gl_fbo(user_data: *mut c_void, info: *const FlutterFrameInfo) -> u32 {
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    if let Some(gl) = unsafe { gl_surface(user_data) }
        && !info.is_null()
    {
        // SAFETY: l'engine passa un FlutterFrameInfo valido per la durata della chiamata.
        let size = unsafe { (*info).size };
        gl.resize(size.width, size.height);
    }
    // Il framebuffer predefinito della superficie EGL.
    0
}

unsafe extern "C" fn gl_proc_resolver(user_data: *mut c_void, name: *const c_char) -> *mut c_void {
    if name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    let Some(gl) = (unsafe { gl_surface(user_data) }) else {
        return ptr::null_mut();
    };
    // SAFETY: l'engine passa il nome della funzione GL come stringa C terminata da NUL.
    match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(name) => gl.proc_address(name),
        Err(_) => ptr::null_mut(),
    }
}

unsafe extern "C" fn vsync(user_data: *mut c_void, baton: isize) {
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    if let Some(vsync) = &callbacks.vsync {
        vsync(VsyncBaton(baton));
    }
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

unsafe extern "C" fn platform_message(
    message: *const FlutterPlatformMessage,
    user_data: *mut c_void,
) {
    // SAFETY: user_data è quello passato a FlutterEngineRun (vedi `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    if message.is_null() {
        return;
    }
    // SAFETY: l'engine passa un messaggio valido per la durata della chiamata.
    let message = unsafe { &*message };
    let channel = if message.channel.is_null() {
        String::new()
    } else {
        // SAFETY: il nome del canale è una stringa C terminata da NUL.
        unsafe { CStr::from_ptr(message.channel) }
            .to_string_lossy()
            .into_owned()
    };
    let data = if message.message.is_null() || message.message_size == 0 {
        Vec::new()
    } else {
        // SAFETY: message punta a message_size byte validi durante la chiamata; si copiano.
        unsafe { std::slice::from_raw_parts(message.message, message.message_size) }.to_vec()
    };
    let response =
        (!message.response_handle.is_null()).then_some(ResponseHandle(message.response_handle));
    (callbacks.platform_message)(PlatformMessage {
        channel,
        data,
        response,
    });
}

unsafe extern "C" fn key_handled(handled: bool, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    // SAFETY: user_data è il Box<KeyHandled> creato in send_key con Box::into_raw;
    // l'engine chiama questo callback una sola volta per evento, quindi lo si
    // riprende in possesso una sola volta.
    let done = unsafe { Box::from_raw(user_data.cast::<KeyHandled>()) };
    done(handled);
}

unsafe extern "C" fn reply_received(data: *const u8, size: usize, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    // SAFETY: user_data è il Box<Reply> creato in send_message_with_reply; l'engine
    // chiama il callback di un response handle una sola volta.
    let reply = unsafe { Box::from_raw(user_data.cast::<Reply>()) };
    let bytes = if data.is_null() || size == 0 {
        Vec::new()
    } else {
        // SAFETY: data punta a size byte validi durante la chiamata; si copiano.
        unsafe { std::slice::from_raw_parts(data, size) }.to_vec()
    };
    reply(bytes);
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
            schedule_frame: symbol!(library, FlutterEngineScheduleFrame),
            on_vsync: symbol!(library, FlutterEngineOnVsync),
            runs_aot: symbol!(library, FlutterEngineRunsAOTCompiledDartCode),
            create_aot: symbol!(library, FlutterEngineCreateAOTData),
            collect_aot: symbol!(library, FlutterEngineCollectAOTData),
            run: symbol!(library, FlutterEngineRun),
            shutdown: symbol!(library, FlutterEngineShutdown),
            run_task: symbol!(library, FlutterEngineRunTask),
            metrics: symbol!(library, FlutterEngineSendWindowMetricsEvent),
            pointer: symbol!(library, FlutterEngineSendPointerEvent),
            time: symbol!(library, FlutterEngineGetCurrentTime),
            key: symbol!(library, FlutterEngineSendKeyEvent),
            send_message: symbol!(library, FlutterEngineSendPlatformMessage),
            respond: symbol!(library, FlutterEngineSendPlatformMessageResponse),
            create_reply: symbol!(library, FlutterPlatformMessageCreateResponseHandle),
            release_reply: symbol!(library, FlutterPlatformMessageReleaseResponseHandle),
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
        let mut renderer = FlutterRendererConfig::default();
        if callbacks.gl.is_some() {
            renderer.type_ = FlutterRendererType::kOpenGL;
            renderer.__bindgen_anon_1.open_gl = FlutterOpenGLRendererConfig {
                struct_size: size_of::<FlutterOpenGLRendererConfig>(),
                make_current: Some(gl_make_current),
                clear_current: Some(gl_clear_current),
                present: Some(gl_present),
                fbo_with_frame_info_callback: Some(gl_fbo),
                make_resource_current: Some(gl_make_resource_current),
                gl_proc_resolver: Some(gl_proc_resolver),
                ..Default::default()
            };
        } else {
            renderer.type_ = FlutterRendererType::kSoftware;
            renderer.__bindgen_anon_1.software = FlutterSoftwareRendererConfig {
                struct_size: size_of::<FlutterSoftwareRendererConfig>(),
                surface_present_callback: Some(present),
            };
        }

        // SAFETY: funzione senza argomenti né precondizioni.
        let runs_aot = unsafe { (api.runs_aot)() };
        let aot_data = match (runs_aot, config.aot) {
            (false, Some(path)) => {
                tracing::warn!(
                    "engine debug: {} ignorato, il Dart gira in JIT",
                    path.display()
                );
                ptr::null_mut()
            }
            (false, None) => ptr::null_mut(),
            (true, None) => {
                return Err(Error::Path(
                    "engine release senza libapp.so: cargo xtask bundle <app> --release".to_owned(),
                ));
            }
            (true, Some(path)) => {
                let elf = c_path(path)?;
                let mut source = FlutterEngineAOTDataSource {
                    type_: FlutterEngineAOTDataSourceType::kFlutterEngineAOTDataSourceTypeElfPath,
                    ..Default::default()
                };
                source.__bindgen_anon_1.elf_path = elf.as_ptr();
                let mut data: FlutterEngineAOTData = ptr::null_mut();
                // SAFETY: source è inizializzato e il percorso vive durante la chiamata, che
                // carica l'ELF e ne copia ciò che serve in data.
                check("FlutterEngineCreateAOTData", unsafe {
                    (api.create_aot)(&source, &mut data)
                })?;
                data
            }
        };

        let mut strings = vec![
            c_path(config.assets)?,
            c_path(config.icu_data)?,
            CString::from(c"silica-embedder"),
        ];
        for arg in config.engine_args {
            strings.push(CString::new(arg.as_str()).map_err(|_| Error::Path(arg.clone()))?);
        }
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
            platform_message_callback: Some(platform_message),
            shutdown_dart_vm_when_done: true,
            aot_data,
            vsync_callback: if callbacks.vsync.is_some() {
                Some(vsync)
            } else {
                None
            },
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
        if result != FlutterEngineResult::kSuccess || engine.is_null() {
            if !aot_data.is_null() {
                // SAFETY: aot_data viene da FlutterEngineCreateAOTData e nessun engine lo usa.
                let _ = unsafe { (api.collect_aot)(aot_data) };
            }
            check("FlutterEngineRun", result)?;
            return Err(Error::Engine("FlutterEngineRun", -1));
        }
        Ok(Self {
            api,
            engine,
            aot_data,
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

    /// Porta la superficie EGL (se c'è) a `width`×`height` pixel fisici. Va chiamato prima
    /// di mandare le metriche: Mesa fissa la dimensione del buffer quando il contesto
    /// diventa corrente, prima del callback del frame.
    pub fn resize_surface(&self, width: u32, height: u32) {
        if let Some(gl) = &self._callbacks.gl {
            gl.resize(width, height);
        }
    }

    /// Risponde a una richiesta di vsync: il frame parte a `start` e va finito entro
    /// `target`, nella base di [`Engine::current_time`].
    pub fn on_vsync(&self, baton: VsyncBaton, start: u64, target: u64) -> Result<(), Error> {
        // SAFETY: engine è vivo; il baton viene da questo engine ed è consumato qui.
        check("FlutterEngineOnVsync", unsafe {
            (self.api.on_vsync)(self.engine, baton.0, start, target)
        })
    }

    /// Se la superficie EGL ha già presentato un frame; `true` senza OpenGL.
    pub fn gl_presented(&self) -> bool {
        self._callbacks.gl.as_ref().is_none_or(GlSurface::presented)
    }

    /// Chiede un frame anche se il framework non ne ha in programma.
    pub fn schedule_frame(&self) -> Result<(), Error> {
        // SAFETY: engine è vivo; la funzione non ha altri argomenti.
        check("FlutterEngineScheduleFrame", unsafe {
            (self.api.schedule_frame)(self.engine)
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

impl Engine {
    /// Manda un tasto al framework; `handled` riceve se un widget l'ha consumato.
    pub fn send_key(
        &self,
        event: &KeyEvent,
        handled: impl FnOnce(bool) + Send + 'static,
    ) -> Result<(), Error> {
        let type_ = match event.phase {
            KeyPhase::Down => FlutterKeyEventType::kFlutterKeyEventTypeDown,
            KeyPhase::Up => FlutterKeyEventType::kFlutterKeyEventTypeUp,
            KeyPhase::Repeat => FlutterKeyEventType::kFlutterKeyEventTypeRepeat,
        };
        let character = event
            .character
            .as_deref()
            .and_then(|text| CString::new(text).ok());
        let raw = RawKeyEvent {
            struct_size: size_of::<RawKeyEvent>(),
            timestamp: event.timestamp_micros as f64,
            type_,
            physical: event.physical,
            logical: event.logical,
            character: character.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
            synthesized: event.synthesized,
            device_type: FlutterKeyEventDeviceType::kFlutterKeyEventDeviceTypeKeyboard,
        };
        let done: Box<KeyHandled> = Box::new(Box::new(handled));
        let user_data = Box::into_raw(done).cast::<c_void>();
        // SAFETY: engine è vivo; raw e character vivono per la durata della chiamata
        // (l'engine copia l'evento); user_data passa a key_handled, che lo libera.
        let result = unsafe { (self.api.key)(self.engine, &raw, Some(key_handled), user_data) };
        if result != FlutterEngineResult::kSuccess {
            // L'engine ha rifiutato l'evento e non chiamerà key_handled: si libera qui.
            // SAFETY: user_data viene da Box::into_raw qui sopra e non è stato consumato.
            drop(unsafe { Box::from_raw(user_data.cast::<KeyHandled>()) });
        }
        check("FlutterEngineSendKeyEvent", result)
    }

    /// Risponde a un messaggio di piattaforma; `data` vuoto vuol dire "non gestito".
    pub fn respond(&self, handle: ResponseHandle, data: &[u8]) -> Result<(), Error> {
        let pointer = if data.is_empty() {
            ptr::null()
        } else {
            data.as_ptr()
        };
        // SAFETY: engine è vivo; handle viene da un messaggio di questo engine ed è
        // consumato qui (ResponseHandle non è Clone); data vive durante la chiamata.
        check("FlutterEngineSendPlatformMessageResponse", unsafe {
            (self.api.respond)(self.engine, handle.0, pointer, data.len())
        })
    }

    /// Manda un messaggio al framework su `channel`; `reply` riceve la risposta
    /// (vuota se nessuno lo gestisce).
    pub fn send_message_with_reply(
        &self,
        channel: &str,
        data: &[u8],
        reply: impl FnOnce(Vec<u8>) + Send + 'static,
    ) -> Result<(), Error> {
        let channel = CString::new(channel).map_err(|_| Error::Path(channel.to_owned()))?;
        let boxed: Box<Reply> = Box::new(Box::new(reply));
        let user_data = Box::into_raw(boxed).cast::<c_void>();
        let mut handle: *mut FlutterPlatformMessageResponseHandle = ptr::null_mut();
        // SAFETY: engine è vivo; user_data passa a reply_received, che lo libera.
        let created = unsafe {
            (self.api.create_reply)(self.engine, Some(reply_received), user_data, &mut handle)
        };
        if created != FlutterEngineResult::kSuccess || handle.is_null() {
            // SAFETY: l'handle non esiste, quindi reply_received non verrà chiamato.
            drop(unsafe { Box::from_raw(user_data.cast::<Reply>()) });
            return check("FlutterPlatformMessageCreateResponseHandle", created);
        }
        let message = FlutterPlatformMessage {
            struct_size: size_of::<FlutterPlatformMessage>(),
            channel: channel.as_ptr(),
            message: data.as_ptr(),
            message_size: data.len(),
            response_handle: handle,
        };
        // SAFETY: engine è vivo; message, channel e data vivono durante la chiamata.
        let sent = unsafe { (self.api.send_message)(self.engine, &message) };
        // SAFETY: l'handle è stato creato qui; dopo l'invio l'engine ne tiene una copia
        // propria e quello dell'embedder va rilasciato (flutter_embedder.h).
        let released = unsafe { (self.api.release_reply)(self.engine, handle) };
        check("FlutterEngineSendPlatformMessage", sent)?;
        check("FlutterPlatformMessageReleaseResponseHandle", released)
    }

    /// Manda un messaggio senza risposta al framework su `channel`.
    pub fn send_message(&self, channel: &str, data: &[u8]) -> Result<(), Error> {
        let channel = CString::new(channel).map_err(|_| Error::Path(channel.to_owned()))?;
        let message = FlutterPlatformMessage {
            struct_size: size_of::<FlutterPlatformMessage>(),
            channel: channel.as_ptr(),
            message: data.as_ptr(),
            message_size: data.len(),
            response_handle: ptr::null(),
        };
        // SAFETY: engine è vivo; message, channel e data vivono durante la chiamata.
        check("FlutterEngineSendPlatformMessage", unsafe {
            (self.api.send_message)(self.engine, &message)
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
        if !self.aot_data.is_null() {
            // SAFETY: l'engine che usava i dati AOT è spento; si liberano una volta sola.
            let result = unsafe { (self.api.collect_aot)(self.aot_data) };
            if let Err(err) = check("FlutterEngineCollectAOTData", result) {
                tracing::warn!("{err}");
            }
        }
    }
}
