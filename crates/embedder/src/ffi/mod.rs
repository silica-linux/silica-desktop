//! FFI to `libflutter_engine.so`: the only module of the workspace with `unsafe`
//! (AGENTS.md, "Never crash"; checked by `cargo xtask unsafe`).
//!
//! The rest of the embedder sees only safe types: [`Engine`], [`Task`], [`Frame`],
//! [`PointerEvent`]. The library is loaded at runtime, so the workspace compiles and the
//! tests run even where the engine has not been downloaded.

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
mod compositor;
mod gl;
pub mod pty;
mod raster;
mod vulkan;

pub use gl::GlSurface;
pub use raster::FrameClock;
pub use vulkan::VulkanSurface;

use std::cell::Cell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::sync::Arc;
use std::thread::ThreadId;

use raster::RasterRunner;

use bindings::{
    FLUTTER_ENGINE_VERSION, FlutterCustomTaskRunners, FlutterEngine, FlutterEngineAOTData,
    FlutterEngineAOTDataSource, FlutterEngineAOTDataSourceType, FlutterEngineLib,
    FlutterEngineResult, FlutterFrameInfo, FlutterKeyEvent as RawKeyEvent,
    FlutterKeyEventDeviceType, FlutterKeyEventType, FlutterLocale, FlutterOpenGLRendererConfig,
    FlutterPlatformMessage, FlutterPlatformMessageResponseHandle, FlutterPointerDeviceKind,
    FlutterPointerEvent as RawPointerEvent, FlutterPointerPhase, FlutterPointerSignalKind,
    FlutterProjectArgs, FlutterRendererConfig, FlutterRendererType, FlutterSoftwareRendererConfig,
    FlutterTask, FlutterTaskRunnerDescription, FlutterVulkanImage, FlutterVulkanRendererConfig,
    FlutterWindowMetricsEvent,
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
            Error::Load(err) => write!(f, "cannot load libflutter_engine.so: {err}"),
            Error::Symbol(name, err) => write!(f, "symbol {name} missing: {err}"),
            Error::Path(path) => write!(f, "invalid path: {path}"),
            Error::Engine(call, code) => write!(f, "{call} returned error {code}"),
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

/// A task the engine asks to run on the platform thread.
pub struct Task {
    inner: FlutterTask,
    /// When to run it, in the base of [`Engine::current_time`] (nanoseconds).
    pub target_nanos: u64,
    /// Raster tasks posted before this one ([`FrameClock::posted`]): a message it delivers
    /// was sent after the frame those tasks present.
    pub raster_seq: u64,
    /// Due at once when posted: a present waits for it ([`FrameClock::before_present`]).
    immediate: bool,
}

thread_local! {
    /// The `raster_seq` of the platform task running on this thread.
    static RUNNING_STAMP: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: FlutterTask is an opaque handle (runner pointer + id) that the engine passes from
// any thread precisely so that it runs on the platform one; the contract of
// FlutterTaskRunnerPostTaskCallback allows moving it between threads.
unsafe impl Send for Task {}

/// A frame of the software renderer: rows of `stride` bytes, 32-bit pixels in the engine's
/// native format (BGRA in memory on little-endian Linux).
pub struct Frame {
    /// The view (window) it was drawn for: 0 for the main one.
    pub view_id: i64,
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub pixels: Vec<u8>,
    /// The raster task that drew it ([`FrameClock::running`]).
    pub raster_seq: u64,
}

/// The response owed to a platform message: it must be consumed with
/// [`Engine::respond`] exactly once, otherwise the Dart Future never completes.
pub struct ResponseHandle(*const FlutterPlatformMessageResponseHandle);

// SAFETY: the handle is opaque and the engine accepts FlutterEngineSendPlatformMessageResponse
// from any thread; here it is only used on the platform thread anyway.
unsafe impl Send for ResponseHandle {}

/// A message from the Dart framework to the embedder.
pub struct PlatformMessage {
    pub channel: String,
    pub data: Vec<u8>,
    /// Absent for messages that do not expect a response.
    pub response: Option<ResponseHandle>,
    /// Raster tasks posted before the message ([`Task::raster_seq`]): Dart sent it after the
    /// frame they present (the glass regions measured on that frame).
    pub raster_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPhase {
    Down,
    Up,
    Repeat,
}

/// A key event for Flutter's `HardwareKeyboard`.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyEvent {
    pub phase: KeyPhase,
    /// Flutter physical key (USB HID page).
    pub physical: u64,
    /// Flutter logical key.
    pub logical: u64,
    /// The text produced by the key, only for `Down` and `Repeat`.
    pub character: Option<String>,
    /// Event made up by the embedder to realign the key state.
    pub synthesized: bool,
    pub timestamp_micros: u64,
}

/// The engine's vsync request: it must be returned with [`Engine::on_vsync`].
pub struct VsyncBaton(isize);

/// The embedder functions called by the engine, from any thread.
pub struct Callbacks {
    pub platform_thread: ThreadId,
    pub post_task: Box<dyn Fn(Task) + Send + Sync>,
    /// Frames of the software renderer; unused with `gl`.
    pub present: Box<dyn Fn(Frame) + Send + Sync>,
    pub platform_message: Box<dyn Fn(PlatformMessage) + Send + Sync>,
    /// With an EGL surface the engine draws with OpenGL ES, otherwise in software.
    pub gl: Option<GlSurface>,
    /// With a Vulkan swapchain the engine draws on Vulkan (it takes precedence over `gl`):
    /// with Impeller, or with Skia for several views on an engine without Impeller's Vulkan
    /// backing stores.
    pub vulkan: Option<VulkanSurface>,
    /// The frame pace comes from the embedder (compositor frame callbacks); without it, the
    /// engine uses its own 60 Hz timer, unrelated to the compositor.
    pub vsync: Option<Box<dyn Fn(VsyncBaton) + Send + Sync>>,
    /// The raster thread's tasks and frames, numbered (the raster thread is the embedder's).
    pub frames: Arc<FrameClock>,
    /// Several views (windows, shell layers) in this engine: it composes through backing
    /// stores (`compositor.rs`), in software, with OpenGL ES or with Vulkan.
    pub multi_view: bool,
}

/// Whoever waits to know whether the framework handled a key.
type KeyHandled = Box<dyn FnOnce(bool) + Send>;
/// Whoever waits for the framework's response to a message.
type Reply = Box<dyn FnOnce(Vec<u8>) + Send>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerPhase {
    Add,
    Remove,
    Hover,
    Down,
    Move,
    Up,
    /// An interrupted gesture (the pointer taken away by a drag between apps).
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    pub phase: PointerPhase,
    /// Position in physical pixels.
    pub x: f64,
    pub y: f64,
    /// Mask of the pressed buttons: 1 primary, 2 secondary, 4 middle.
    pub buttons: i64,
    /// Scroll in physical pixels, if the event is a wheel.
    pub scroll: Option<(f64, f64)>,
    pub timestamp_micros: u64,
    /// The view (window) the pointer is in: 0 for the main one.
    pub view_id: i64,
}

type InitializeFn = unsafe extern "C" fn(
    usize,
    *const FlutterRendererConfig,
    *const FlutterProjectArgs,
    *mut c_void,
    *mut FlutterEngine,
) -> FlutterEngineResult;
type RunInitializedFn = unsafe extern "C" fn(FlutterEngine) -> FlutterEngineResult;
type DeinitializeFn = unsafe extern "C" fn(FlutterEngine) -> FlutterEngineResult;
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

type UpdateLocalesFn =
    unsafe extern "C" fn(FlutterEngine, *mut *const FlutterLocale, usize) -> FlutterEngineResult;
type RunsAotFn = unsafe extern "C" fn() -> bool;
type ScheduleFrameFn = unsafe extern "C" fn(FlutterEngine) -> FlutterEngineResult;
type OnVsyncFn = unsafe extern "C" fn(FlutterEngine, isize, u64, u64) -> FlutterEngineResult;
type CreateAotFn = unsafe extern "C" fn(
    *const FlutterEngineAOTDataSource,
    *mut FlutterEngineAOTData,
) -> FlutterEngineResult;
type CollectAotFn = unsafe extern "C" fn(FlutterEngineAOTData) -> FlutterEngineResult;
type AddViewFn =
    unsafe extern "C" fn(FlutterEngine, *const bindings::FlutterAddViewInfo) -> FlutterEngineResult;
type RemoveViewFn = unsafe extern "C" fn(
    FlutterEngine,
    *const bindings::FlutterRemoveViewInfo,
) -> FlutterEngineResult;
type ViewFocusFn = unsafe extern "C" fn(
    FlutterEngine,
    *const bindings::FlutterViewFocusEvent,
) -> FlutterEngineResult;
/// Whoever waits for a view to be added or removed (`true`: done).
type ViewDone = Box<dyn FnOnce(bool) + Send>;

/// The engine functions used here, resolved once at load time.
struct Api {
    schedule_frame: ScheduleFrameFn,
    on_vsync: OnVsyncFn,
    runs_aot: RunsAotFn,
    create_aot: CreateAotFn,
    collect_aot: CollectAotFn,
    initialize: InitializeFn,
    run_initialized: RunInitializedFn,
    deinitialize: DeinitializeFn,
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
    update_locales: UpdateLocalesFn,
    add_view: AddViewFn,
    remove_view: RemoveViewFn,
    view_focus: ViewFocusFn,
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
    /// The AOT-compiled Dart code (`libapp.so`), for a release or profile engine.
    pub aot: Option<&'a Path>,
    /// Command-line options for the engine (for example `--enable-impeller`).
    pub engine_args: &'a [String],
    /// Arguments of the Dart app's `main(List<String> args)`.
    pub dart_args: &'a [String],
}

/// A running Flutter engine. It is used and destroyed on the platform thread.
pub struct Engine {
    api: Api,
    engine: FlutterEngine,
    /// Null with a debug engine (JIT); freed after shutdown.
    aot_data: FlutterEngineAOTData,
    /// The raster thread: stopped right after the shutdown, before the callbacks it may use
    /// are dropped (fields drop in declaration order).
    raster: RasterRunner,
    // Everything the engine keeps pointers to: it must live as long as the engine.
    _callbacks: Box<Callbacks>,
    /// The compositor of the multi-view path (`compositor.rs`).
    _compositor: Option<Box<bindings::FlutterCompositor>>,
    _runner: Box<FlutterTaskRunnerDescription>,
    _render_runner: Box<FlutterTaskRunnerDescription>,
    _runners: Box<FlutterCustomTaskRunners>,
    _strings: Vec<CString>,
    _argv: Vec<*const c_char>,
    /// The app's `main` arguments: the engine reads them when the isolate starts.
    _dart_strings: Vec<CString>,
    _dart_argv: Vec<*const c_char>,
    _library: FlutterEngineLib,
}

fn c_path(path: &Path) -> Result<CString, Error> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Path(path.display().to_string()))
}

/// The pointer to [`Callbacks`] passed to the engine as `user_data`.
///
/// # Safety
/// `user_data` must be the pointer to `Callbacks` given to [`Engine::run`], still alive.
unsafe fn callbacks<'a>(user_data: *mut c_void) -> &'a Callbacks {
    // SAFETY: guaranteed by the caller; Engine keeps the Box until the engine is shut down,
    // and after FlutterEngineShutdown the engine no longer calls the callbacks.
    unsafe { &*user_data.cast_const().cast::<Callbacks>() }
}

unsafe extern "C" fn present(
    user_data: *mut c_void,
    allocation: *const c_void,
    row_bytes: usize,
    height: usize,
) -> bool {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    // With several views the frames come from `present_view`: the root surface shows nothing.
    if callbacks.multi_view {
        return true;
    }
    let Some(len) = row_bytes.checked_mul(height) else {
        return false;
    };
    if allocation.is_null() || len == 0 {
        return false;
    }
    // SAFETY: for SoftwareSurfacePresentCallback the buffer has row_bytes * height valid bytes
    // for the whole duration of the call; we copy it before returning.
    let pixels = unsafe { std::slice::from_raw_parts(allocation.cast::<u8>(), len) }.to_vec();
    let (Ok(width), Ok(height)) = (u32::try_from(row_bytes / 4), u32::try_from(height)) else {
        return false;
    };
    (callbacks.present)(Frame {
        view_id: 0,
        width,
        height,
        stride: row_bytes,
        pixels,
        raster_seq: callbacks.frames.running(),
    });
    true
}

/// The EGL surface of the callbacks, if the engine uses the OpenGL renderer.
///
/// # Safety
/// Like [`callbacks`].
unsafe fn gl_surface<'a>(user_data: *mut c_void) -> Option<&'a GlSurface> {
    // SAFETY: guaranteed by the caller.
    unsafe { callbacks(user_data) }.gl.as_ref()
}

unsafe extern "C" fn gl_make_current(user_data: *mut c_void) -> bool {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    unsafe { gl_surface(user_data) }.is_some_and(GlSurface::make_current)
}

unsafe extern "C" fn gl_clear_current(user_data: *mut c_void) -> bool {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    unsafe { gl_surface(user_data) }.is_some_and(GlSurface::clear_current)
}

unsafe extern "C" fn gl_make_resource_current(user_data: *mut c_void) -> bool {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    unsafe { gl_surface(user_data) }.is_some_and(GlSurface::make_resource_current)
}

unsafe extern "C" fn gl_present(user_data: *mut c_void) -> bool {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    static FIRST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    crate::trace::mark_once(&FIRST, "first GL present");
    // With several views the swaps happen in `present_view`: the root frame is not shown.
    if callbacks.multi_view {
        return true;
    }
    // The swap commits the surface: what goes with this frame is sent before it.
    callbacks.frames.before_present(0);
    callbacks.gl.as_ref().is_some_and(GlSurface::present)
}

unsafe extern "C" fn gl_fbo(user_data: *mut c_void, info: *const FlutterFrameInfo) -> u32 {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    if let Some(gl) = unsafe { gl_surface(user_data) }
        && !gl.multi_view()
        && !info.is_null()
    {
        // SAFETY: the engine passes a FlutterFrameInfo valid for the duration of the call.
        let size = unsafe { (*info).size };
        gl.resize(size.width, size.height);
    }
    // The default framebuffer of the EGL surface.
    0
}

unsafe extern "C" fn gl_proc_resolver(user_data: *mut c_void, name: *const c_char) -> *mut c_void {
    if name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    let Some(gl) = (unsafe { gl_surface(user_data) }) else {
        return ptr::null_mut();
    };
    // SAFETY: the engine passes the GL function name as a NUL-terminated C string.
    match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(name) => gl.proc_address(name),
        Err(_) => ptr::null_mut(),
    }
}

/// The Vulkan surface of the callbacks, if the engine uses the Vulkan renderer.
///
/// # Safety
/// Like [`callbacks`].
unsafe fn vulkan_surface<'a>(user_data: *mut c_void) -> Option<&'a VulkanSurface> {
    // SAFETY: guaranteed by the caller.
    unsafe { callbacks(user_data) }.vulkan.as_ref()
}

unsafe extern "C" fn vulkan_proc_address(
    user_data: *mut c_void,
    instance: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    if name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    let Some(vulkan) = (unsafe { vulkan_surface(user_data) }) else {
        return ptr::null_mut();
    };
    // SAFETY: the engine passes the function name as a NUL-terminated C string.
    vulkan.proc_address(instance, unsafe { CStr::from_ptr(name) })
}

unsafe extern "C" fn vulkan_next_image(
    user_data: *mut c_void,
    info: *const FlutterFrameInfo,
) -> FlutterVulkanImage {
    let mut image = FlutterVulkanImage {
        struct_size: size_of::<FlutterVulkanImage>(),
        ..Default::default()
    };
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    let Some(vulkan) = (unsafe { vulkan_surface(user_data) }) else {
        return image;
    };
    let size = if info.is_null() {
        (0, 0)
    } else {
        // SAFETY: the engine passes a FlutterFrameInfo valid for the duration of the call.
        let size = unsafe { (*info).size };
        (size.width, size.height)
    };
    let (handle, format) = vulkan.next_image(size);
    image.image = handle;
    image.format = format;
    image
}

unsafe extern "C" fn vulkan_present(
    user_data: *mut c_void,
    _image: *const FlutterVulkanImage,
) -> bool {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    static FIRST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    crate::trace::mark_once(&FIRST, "first Vulkan present");
    // The present commits the surface: what goes with this frame is sent before it.
    callbacks.frames.before_present(0);
    callbacks
        .vulkan
        .as_ref()
        .is_some_and(VulkanSurface::present)
}

unsafe extern "C" fn vsync(user_data: *mut c_void, baton: isize) {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    static FIRST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    crate::trace::mark_once(&FIRST, "first vsync request (Dart scheduled a frame)");
    if let Some(vsync) = &callbacks.vsync {
        vsync(VsyncBaton(baton));
    }
}

unsafe extern "C" fn post_task(task: FlutterTask, target_nanos: u64, user_data: *mut c_void) {
    // SAFETY: user_data is the one of the FlutterTaskRunnerDescription (see `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    let immediate = callbacks.frames.platform_posted(target_nanos);
    (callbacks.post_task)(Task {
        inner: task,
        target_nanos,
        raster_seq: callbacks.frames.posted(),
        immediate,
    });
}

unsafe extern "C" fn runs_on_current_thread(user_data: *mut c_void) -> bool {
    // SAFETY: user_data is the one of the FlutterTaskRunnerDescription (see `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    callbacks.platform_thread == std::thread::current().id()
}

unsafe extern "C" fn platform_message(
    message: *const FlutterPlatformMessage,
    user_data: *mut c_void,
) {
    // SAFETY: user_data is the one passed to FlutterEngineRun (see `callbacks`).
    let callbacks = unsafe { callbacks(user_data) };
    if message.is_null() {
        return;
    }
    // SAFETY: the engine passes a message valid for the duration of the call.
    let message = unsafe { &*message };
    let channel = if message.channel.is_null() {
        String::new()
    } else {
        // SAFETY: the channel name is a NUL-terminated C string.
        unsafe { CStr::from_ptr(message.channel) }
            .to_string_lossy()
            .into_owned()
    };
    let data = if message.message.is_null() || message.message_size == 0 {
        Vec::new()
    } else {
        // SAFETY: message points to message_size valid bytes during the call; they are copied.
        unsafe { std::slice::from_raw_parts(message.message, message.message_size) }.to_vec()
    };
    if crate::trace::enabled() {
        crate::trace::mark(&format!("platform message {channel}"));
    }
    let response =
        (!message.response_handle.is_null()).then_some(ResponseHandle(message.response_handle));
    (callbacks.platform_message)(PlatformMessage {
        channel,
        data,
        response,
        raster_seq: RUNNING_STAMP.with(Cell::get),
    });
}

unsafe extern "C" fn key_handled(handled: bool, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    // SAFETY: user_data is the Box<KeyHandled> created in send_key with Box::into_raw;
    // the engine calls this callback only once per event, so ownership is taken back
    // only once.
    let done = unsafe { Box::from_raw(user_data.cast::<KeyHandled>()) };
    done(handled);
}

unsafe extern "C" fn reply_received(data: *const u8, size: usize, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    // SAFETY: user_data is the Box<Reply> created in send_message_with_reply; the engine
    // calls the callback of a response handle only once.
    let reply = unsafe { Box::from_raw(user_data.cast::<Reply>()) };
    let bytes = if data.is_null() || size == 0 {
        Vec::new()
    } else {
        // SAFETY: data points to size valid bytes during the call; they are copied.
        unsafe { std::slice::from_raw_parts(data, size) }.to_vec()
    };
    reply(bytes);
}

/// The metrics of view `view_id`.
fn metrics(view_id: i64, width: u32, height: u32, pixel_ratio: f64) -> FlutterWindowMetricsEvent {
    FlutterWindowMetricsEvent {
        struct_size: size_of::<FlutterWindowMetricsEvent>(),
        width: width as usize,
        height: height as usize,
        pixel_ratio,
        view_id,
        ..Default::default()
    }
}

unsafe extern "C" fn view_added(result: *const bindings::FlutterAddViewResult) {
    if result.is_null() {
        return;
    }
    // SAFETY: the engine passes a result valid during the call.
    let result = unsafe { &*result };
    if result.user_data.is_null() {
        return;
    }
    // SAFETY: user_data is the Box<ViewDone> of add_view; the engine calls this once.
    let done = unsafe { Box::from_raw(result.user_data.cast::<ViewDone>()) };
    done(result.added);
}

unsafe extern "C" fn view_removed(result: *const bindings::FlutterRemoveViewResult) {
    if result.is_null() {
        return;
    }
    // SAFETY: the engine passes a result valid during the call.
    let result = unsafe { &*result };
    if result.user_data.is_null() {
        return;
    }
    // SAFETY: user_data is the Box<ViewDone> of remove_view; the engine calls this once.
    let done = unsafe { Box::from_raw(result.user_data.cast::<ViewDone>()) };
    done(result.removed);
}

unsafe extern "C" fn log_message(tag: *const c_char, message: *const c_char, _: *mut c_void) {
    let text = |ptr: *const c_char| {
        if ptr.is_null() {
            String::new()
        } else {
            // SAFETY: the engine passes NUL-terminated C strings, valid during the call.
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        }
    };
    tracing::info!(target: "flutter", "{}: {}", text(tag), text(message));
}

impl Engine {
    /// Loads the engine and starts the bundle's app. Must be called on the platform thread,
    /// the same as `callbacks.platform_thread`.
    pub fn run(config: &EngineConfig<'_>, callbacks: Callbacks) -> Result<Self, Error> {
        // SAFETY: loads the official engine library at the version pinned in
        // ENGINE_VERSION, the same as the header the bindings come from; its
        // initializers have no preconditions.
        let library = unsafe { FlutterEngineLib::new(config.library) }
            .map_err(|e| Error::Load(e.to_string()))?;
        crate::trace::mark("engine library loaded");
        let api = Api {
            schedule_frame: symbol!(library, FlutterEngineScheduleFrame),
            on_vsync: symbol!(library, FlutterEngineOnVsync),
            runs_aot: symbol!(library, FlutterEngineRunsAOTCompiledDartCode),
            create_aot: symbol!(library, FlutterEngineCreateAOTData),
            collect_aot: symbol!(library, FlutterEngineCollectAOTData),
            initialize: symbol!(library, FlutterEngineInitialize),
            run_initialized: symbol!(library, FlutterEngineRunInitialized),
            deinitialize: symbol!(library, FlutterEngineDeinitialize),
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
            update_locales: symbol!(library, FlutterEngineUpdateLocales),
            add_view: symbol!(library, FlutterEngineAddView),
            remove_view: symbol!(library, FlutterEngineRemoveView),
            view_focus: symbol!(library, FlutterEngineSendViewFocusEvent),
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
        // The raster thread is the embedder's: its tasks are numbered (`FrameClock`).
        callbacks.frames.set_time(api.time);
        let raster = RasterRunner::start(Arc::clone(&callbacks.frames), api.run_task, api.time)?;
        // Several views on Vulkan: the views of a frame are presented together, when its raster
        // task ends (`VulkanSurface::flush_presents`).
        if callbacks.multi_view && callbacks.vulkan.is_some() {
            let address = user_data as usize;
            raster.on_task_end(move || {
                // SAFETY: address is the Callbacks given to the engine, kept by Engine until after
                // the raster thread is stopped (fields drop in declaration order: `raster` first).
                let callbacks = unsafe { self::callbacks(address as *mut c_void) };
                if let Some(vulkan) = &callbacks.vulkan {
                    vulkan.flush_presents();
                }
            });
        }
        let render_runner = Box::new(FlutterTaskRunnerDescription {
            struct_size: size_of::<FlutterTaskRunnerDescription>(),
            user_data: raster.user_data(),
            runs_task_on_current_thread_callback: Some(raster::runs_on_current_thread),
            post_task_callback: Some(raster::post_task),
            identifier: 2,
            ..Default::default()
        });
        let runners = Box::new(FlutterCustomTaskRunners {
            struct_size: size_of::<FlutterCustomTaskRunners>(),
            platform_task_runner: ptr::from_ref(&*runner),
            render_task_runner: ptr::from_ref(&*render_runner),
            ..Default::default()
        });
        let mut renderer = FlutterRendererConfig::default();
        if let Some(vulkan) = &callbacks.vulkan {
            let (instance, physical, device, family, queue) = vulkan.handles();
            renderer.type_ = FlutterRendererType::kVulkan;
            renderer.__bindgen_anon_1.vulkan = FlutterVulkanRendererConfig {
                struct_size: size_of::<FlutterVulkanRendererConfig>(),
                version: ash_api_version_1_1(),
                instance,
                physical_device: physical,
                device,
                queue_family_index: family,
                queue,
                enabled_instance_extension_count: vulkan.instance_extensions().len(),
                enabled_instance_extensions: vulkan.instance_extensions().as_ptr().cast_mut(),
                enabled_device_extension_count: vulkan.device_extensions().len(),
                enabled_device_extensions: vulkan.device_extensions().as_ptr().cast_mut(),
                get_instance_proc_address_callback: Some(vulkan_proc_address),
                get_next_image_callback: Some(vulkan_next_image),
                present_image_callback: Some(vulkan_present),
            };
        } else if callbacks.gl.is_some() {
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

        // SAFETY: function without arguments or preconditions.
        let runs_aot = unsafe { (api.runs_aot)() };
        let aot_data = match (runs_aot, config.aot) {
            (false, Some(path)) => {
                tracing::warn!(
                    "debug engine: {} ignored, the Dart code runs in JIT",
                    path.display()
                );
                ptr::null_mut()
            }
            (false, None) => ptr::null_mut(),
            (true, None) => {
                return Err(Error::Path(
                    "release engine without libapp.so: cargo xtask bundle <app> --release"
                        .to_owned(),
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
                // SAFETY: source is initialized and the path lives during the call, which
                // loads the ELF and copies what it needs into data.
                check("FlutterEngineCreateAOTData", unsafe {
                    (api.create_aot)(&source, &mut data)
                })?;
                data
            }
        };

        crate::trace::mark("aot data ready");
        let mut strings = vec![
            c_path(config.assets)?,
            c_path(config.icu_data)?,
            CString::from(c"silica-embedder"),
        ];
        for arg in config.engine_args {
            strings.push(CString::new(arg.as_str()).map_err(|_| Error::Path(arg.clone()))?);
        }
        let dart_strings = config
            .dart_args
            .iter()
            .map(|arg| CString::new(arg.as_str()).map_err(|_| Error::Path(arg.clone())))
            .collect::<Result<Vec<CString>, Error>>()?;
        let dart_argv: Vec<*const c_char> = dart_strings.iter().map(|s| s.as_ptr()).collect();
        let argv: Vec<*const c_char> = strings.iter().skip(2).map(|s| s.as_ptr()).collect();
        let (Some(assets), Some(icu)) = (strings.first(), strings.get(1)) else {
            return Err(Error::Path("missing arguments".to_owned()));
        };
        // Several views: the engine draws them into our backing stores (`compositor.rs`).
        let compositor = callbacks.multi_view.then(|| {
            Box::new(bindings::FlutterCompositor {
                struct_size: size_of::<bindings::FlutterCompositor>(),
                user_data,
                create_backing_store_callback: Some(compositor::create_backing_store),
                collect_backing_store_callback: Some(compositor::collect_backing_store),
                present_layers_callback: None,
                // A Vulkan backing store is a swapchain image: a new one every frame.
                avoid_backing_store_cache: callbacks.vulkan.is_some(),
                present_view_callback: Some(compositor::present_view),
            })
        });
        let args = FlutterProjectArgs {
            struct_size: size_of::<FlutterProjectArgs>(),
            assets_path: assets.as_ptr(),
            icu_data_path: icu.as_ptr(),
            command_line_argc: i32::try_from(argv.len()).unwrap_or(0),
            command_line_argv: argv.as_ptr(),
            dart_entrypoint_argc: i32::try_from(dart_argv.len()).unwrap_or(0),
            dart_entrypoint_argv: dart_argv.as_ptr(),
            custom_task_runners: ptr::from_ref(&*runners),
            log_message_callback: Some(log_message),
            platform_message_callback: Some(platform_message),
            shutdown_dart_vm_when_done: true,
            aot_data,
            compositor: compositor.as_deref().map_or(ptr::null(), ptr::from_ref),
            vsync_callback: if callbacks.vsync.is_some() {
                Some(vsync)
            } else {
                None
            },
            ..Default::default()
        };

        let mut engine: FlutterEngine = ptr::null_mut();
        // Initialize, then run: the raster thread needs the handle before the shell starts
        // (the shell waits for its first raster tasks while it starts).
        // SAFETY: renderer and args are structs initialized with a correct struct_size; the
        // pointers they contain (strings, argv, runners, callbacks) stay alive in Engine until
        // the engine is shut down in Drop.
        let result = unsafe {
            (api.initialize)(
                FLUTTER_ENGINE_VERSION as usize,
                &renderer,
                &args,
                user_data,
                &mut engine,
            )
        };
        crate::trace::mark("engine initialized");
        let result = if result == FlutterEngineResult::kSuccess && !engine.is_null() {
            raster.set_engine(engine);
            // SAFETY: the engine was just initialized and has not run yet.
            let run = unsafe { (api.run_initialized)(engine) };
            if run != FlutterEngineResult::kSuccess {
                raster.closing();
                // SAFETY: the engine was initialized, did not start and is not used again.
                let _ = unsafe { (api.deinitialize)(engine) };
            }
            run
        } else {
            result
        };
        if result != FlutterEngineResult::kSuccess || engine.is_null() {
            std::mem::drop(raster);
            if !aot_data.is_null() {
                // SAFETY: aot_data comes from FlutterEngineCreateAOTData and no engine uses it.
                let _ = unsafe { (api.collect_aot)(aot_data) };
            }
            check("FlutterEngineRun", result)?;
            return Err(Error::Engine("FlutterEngineRun", -1));
        }
        Ok(Self {
            api,
            engine,
            aot_data,
            raster,
            _callbacks: callbacks,
            _compositor: compositor,
            _runner: runner,
            _render_runner: render_runner,
            _runners: runners,
            _strings: strings,
            _argv: argv,
            _dart_strings: dart_strings,
            _dart_argv: dart_argv,
            _library: library,
        })
    }

    /// The engine's monotonic clock, in nanoseconds.
    pub fn current_time(&self) -> u64 {
        // SAFETY: function without arguments or preconditions.
        unsafe { (self.api.time)() }
    }

    /// Runs a task received from `post_task`. Only on the platform thread.
    pub fn run_task(&self, task: &Task) -> Result<(), Error> {
        RUNNING_STAMP.with(|stamp| stamp.set(task.raster_seq));
        // SAFETY: the engine is alive; task comes from this engine's post_task and every task
        // runs only once (the caller consumes it from the queue).
        let result = check("FlutterEngineRunTask", unsafe {
            (self.api.run_task)(self.engine, &task.inner)
        });
        if task.immediate {
            self._callbacks.frames.platform_done();
        }
        result
    }

    /// Size of view `view_id` in physical pixels and ratio of physical to logical pixels.
    pub fn send_window_metrics(
        &self,
        view_id: i64,
        width: u32,
        height: u32,
        pixel_ratio: f64,
    ) -> Result<(), Error> {
        let event = metrics(view_id, width, height, pixel_ratio);
        // SAFETY: the engine is alive; event is initialized and lives for the duration of the call.
        check("FlutterEngineSendWindowMetricsEvent", unsafe {
            (self.api.metrics)(self.engine, &event)
        })
    }

    /// Adds view `view_id` of `width`×`height` physical pixels (several windows, #146);
    /// `done` learns from an engine thread whether it was added. Its surface must stay until
    /// the view is removed.
    pub fn add_view(
        &self,
        view_id: i64,
        (width, height): (u32, u32),
        pixel_ratio: f64,
        done: impl FnOnce(bool) + Send + 'static,
    ) -> Result<(), Error> {
        let event = metrics(view_id, width, height, pixel_ratio);
        let user_data = Box::into_raw(Box::new(Box::new(done) as ViewDone)).cast::<c_void>();
        let info = bindings::FlutterAddViewInfo {
            struct_size: size_of::<bindings::FlutterAddViewInfo>(),
            view_id,
            view_metrics: &event,
            user_data,
            add_view_callback: Some(view_added),
        };
        // SAFETY: the engine is alive; info and the metrics live during the call (the engine
        // copies them); user_data passes to view_added, which frees it.
        let result = unsafe { (self.api.add_view)(self.engine, &info) };
        if result != FlutterEngineResult::kSuccess {
            // Rejected: the callback is not called.
            // SAFETY: user_data comes from Box::into_raw above and was not consumed.
            drop(unsafe { Box::from_raw(user_data.cast::<ViewDone>()) });
        }
        check("FlutterEngineAddView", result)
    }

    /// Removes view `view_id`; `done` learns from an engine thread whether it was removed
    /// (only then its surface may go).
    pub fn remove_view(
        &self,
        view_id: i64,
        done: impl FnOnce(bool) + Send + 'static,
    ) -> Result<(), Error> {
        let user_data = Box::into_raw(Box::new(Box::new(done) as ViewDone)).cast::<c_void>();
        let info = bindings::FlutterRemoveViewInfo {
            struct_size: size_of::<bindings::FlutterRemoveViewInfo>(),
            view_id,
            user_data,
            remove_view_callback: Some(view_removed),
        };
        // SAFETY: the engine is alive; info lives during the call; user_data passes to
        // view_removed, which frees it.
        let result = unsafe { (self.api.remove_view)(self.engine, &info) };
        if result != FlutterEngineResult::kSuccess {
            // SAFETY: as in add_view.
            drop(unsafe { Box::from_raw(user_data.cast::<ViewDone>()) });
        }
        check("FlutterEngineRemoveView", result)
    }

    /// Tells the framework which view has the keyboard focus (its `FocusManager` moves the
    /// primary focus into that view).
    pub fn send_view_focus(&self, view_id: i64, focused: bool) -> Result<(), Error> {
        let event = bindings::FlutterViewFocusEvent {
            struct_size: size_of::<bindings::FlutterViewFocusEvent>(),
            view_id,
            state: if focused {
                bindings::FlutterViewFocusState::kFocused
            } else {
                bindings::FlutterViewFocusState::kUnfocused
            },
            direction: bindings::FlutterViewFocusDirection::kUndefined,
        };
        // SAFETY: the engine is alive; event lives during the call.
        check("FlutterEngineSendViewFocusEvent", unsafe {
            (self.api.view_focus)(self.engine, &event)
        })
    }

    /// The EGL contexts and surfaces, with the OpenGL ES renderer.
    pub fn gl(&self) -> Option<&GlSurface> {
        self._callbacks.gl.as_ref()
    }

    /// The Vulkan device and swapchains, with the Vulkan renderer.
    pub fn vulkan(&self) -> Option<&VulkanSurface> {
        self._callbacks.vulkan.as_ref()
    }

    /// Brings the EGL surface (if any) to `width`×`height` physical pixels. Must be called
    /// before sending the metrics: Mesa fixes the buffer size when the context becomes
    /// current, before the frame callback.
    pub fn resize_surface(&self, width: u32, height: u32) {
        if let Some(gl) = &self._callbacks.gl {
            gl.resize(width, height);
        }
    }

    /// Answers a vsync request: the frame starts at `start` and must be done by `target`,
    /// in the base of [`Engine::current_time`].
    pub fn on_vsync(&self, baton: VsyncBaton, start: u64, target: u64) -> Result<(), Error> {
        // SAFETY: the engine is alive; the baton comes from this engine and is consumed here.
        check("FlutterEngineOnVsync", unsafe {
            (self.api.on_vsync)(self.engine, baton.0, start, target)
        })
    }

    /// Whether the EGL or Vulkan surface has already presented a frame; `true` in software.
    pub fn gl_presented(&self) -> bool {
        self._callbacks.gl.as_ref().is_none_or(GlSurface::presented)
            && self
                ._callbacks
                .vulkan
                .as_ref()
                .is_none_or(VulkanSurface::presented)
    }

    /// Asks for a frame even if the framework has none scheduled.
    pub fn schedule_frame(&self) -> Result<(), Error> {
        // SAFETY: the engine is alive; the function has no other arguments.
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
            PointerPhase::Cancel => FlutterPointerPhase::kCancel,
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
            view_id: event.view_id,
            ..Default::default()
        };
        // SAFETY: the engine is alive; raw is an initialized one-element array.
        check("FlutterEngineSendPointerEvent", unsafe {
            (self.api.pointer)(self.engine, &raw, 1)
        })
    }
}

impl Engine {
    /// Sends a key to the framework; `handled` receives whether a widget consumed it.
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
        // SAFETY: the engine is alive; raw and character live for the duration of the call
        // (the engine copies the event); user_data passes to key_handled, which frees it.
        let result = unsafe { (self.api.key)(self.engine, &raw, Some(key_handled), user_data) };
        if result != FlutterEngineResult::kSuccess {
            // The engine rejected the event and will not call key_handled: free it here.
            // SAFETY: user_data comes from Box::into_raw above and was not consumed.
            drop(unsafe { Box::from_raw(user_data.cast::<KeyHandled>()) });
        }
        check("FlutterEngineSendKeyEvent", result)
    }

    /// Answers a platform message; empty `data` means "not handled".
    pub fn respond(&self, handle: ResponseHandle, data: &[u8]) -> Result<(), Error> {
        let pointer = if data.is_empty() {
            ptr::null()
        } else {
            data.as_ptr()
        };
        // SAFETY: the engine is alive; handle comes from a message of this engine and is
        // consumed here (ResponseHandle is not Clone); data lives during the call.
        check("FlutterEngineSendPlatformMessageResponse", unsafe {
            (self.api.respond)(self.engine, handle.0, pointer, data.len())
        })
    }

    /// Sends a message to the framework on `channel`; `reply` receives the response
    /// (empty if nobody handles it).
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
        // SAFETY: the engine is alive; user_data passes to reply_received, which frees it.
        let created = unsafe {
            (self.api.create_reply)(self.engine, Some(reply_received), user_data, &mut handle)
        };
        if created != FlutterEngineResult::kSuccess || handle.is_null() {
            // SAFETY: the handle does not exist, so reply_received will not be called.
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
        // SAFETY: the engine is alive; message, channel and data live during the call.
        let sent = unsafe { (self.api.send_message)(self.engine, &message) };
        // SAFETY: the handle was created here; after sending, the engine keeps its own copy
        // and the embedder's must be released (flutter_embedder.h).
        let released = unsafe { (self.api.release_reply)(self.engine, handle) };
        check("FlutterEngineSendPlatformMessage", sent)?;
        check("FlutterPlatformMessageReleaseResponseHandle", released)
    }

    /// The system languages, most preferred first (`FlutterEngineUpdateLocales`).
    pub fn update_locales(&self, locales: &[crate::locale::Locale]) -> Result<(), Error> {
        let strings: Vec<(CString, Option<CString>)> = locales
            .iter()
            .filter_map(|l| {
                let language = CString::new(l.language.as_str()).ok()?;
                let country = l.country.as_deref().and_then(|c| CString::new(c).ok());
                Some((language, country))
            })
            .collect();
        let structs: Vec<FlutterLocale> = strings
            .iter()
            .map(|(language, country)| FlutterLocale {
                struct_size: size_of::<FlutterLocale>(),
                language_code: language.as_ptr(),
                country_code: country.as_ref().map_or(ptr::null(), |c| c.as_ptr()),
                script_code: ptr::null(),
                variant_code: ptr::null(),
            })
            .collect();
        let mut pointers: Vec<*const FlutterLocale> = structs.iter().map(ptr::from_ref).collect();
        // SAFETY: the engine is alive; the strings, structs and pointers live during the call
        // (the engine copies them).
        check("FlutterEngineUpdateLocales", unsafe {
            (self.api.update_locales)(self.engine, pointers.as_mut_ptr(), pointers.len())
        })
    }

    /// Sends a message without a response to the framework on `channel`.
    pub fn send_message(&self, channel: &str, data: &[u8]) -> Result<(), Error> {
        let channel = CString::new(channel).map_err(|_| Error::Path(channel.to_owned()))?;
        let message = FlutterPlatformMessage {
            struct_size: size_of::<FlutterPlatformMessage>(),
            channel: channel.as_ptr(),
            message: data.as_ptr(),
            message_size: data.len(),
            response_handle: ptr::null(),
        };
        // SAFETY: the engine is alive; message, channel and data live during the call.
        check("FlutterEngineSendPlatformMessage", unsafe {
            (self.api.send_message)(self.engine, &message)
        })
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // The delayed raster tasks already queued never run: the engine is gone afterwards.
        self.raster.closing();
        // SAFETY: the engine is alive and no longer used afterwards; the fields the engine
        // references are destroyed only after this call (field order in Engine).
        let result = unsafe { (self.api.shutdown)(self.engine) };
        if let Err(err) = check("FlutterEngineShutdown", result) {
            tracing::warn!("{err}");
        }
        if !self.aot_data.is_null() {
            // SAFETY: the engine that used the AOT data is shut down; they are freed only once.
            let result = unsafe { (self.api.collect_aot)(self.aot_data) };
            if let Err(err) = check("FlutterEngineCollectAOTData", result) {
                tracing::warn!("{err}");
            }
        }
    }
}

/// `VK_API_VERSION_1_1`, the version the instance and the device are created with.
fn ash_api_version_1_1() -> u32 {
    ash::vk::API_VERSION_1_1
}
