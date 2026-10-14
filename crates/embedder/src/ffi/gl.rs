//! The engine's OpenGL ES renderer: EGL on the app's `wl_surface`.
//!
//! The engine calls these callbacks from the raster thread (window context, swap,
//! resize) and from the IO thread (resource context, shared with the first one).
//! `eglSwapBuffers` attaches and commits the surface by itself.

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use khronos_egl as egl;
use wayland_client::Connection;
use wayland_client::Proxy;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_egl::WlEglSurface;

use super::Error;

/// EGL_PLATFORM_WAYLAND_KHR (EGL_KHR_platform_wayland).
const PLATFORM_WAYLAND: egl::Enum = 0x31D8;
/// GL_RENDERER, for `glGetString`.
const GL_RENDERER: u32 = 0x1F01;

type GetStringFn = unsafe extern "C" fn(u32) -> *const std::ffi::c_char;

fn egl_error(call: &'static str) -> impl Fn(egl::Error) -> Error {
    move |err| Error::Gl(call, err.to_string())
}

/// The app's EGL contexts and surface. Lives in [`super::Callbacks`] while the engine runs.
pub struct GlSurface {
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    context: egl::Context,
    resource_context: egl::Context,
    surface: egl::Surface,
    /// The EGL window and its size in physical pixels; the raster thread resizes it.
    window: Mutex<(WlEglSurface, (u32, u32))>,
    /// Resized since the last swap: Mesa (at least WSL's software path) applies the new
    /// size only from the next buffer, so another frame is needed.
    resized: AtomicBool,
    /// At least one frame reached the compositor: from here on the frame callbacks work.
    presented: AtomicBool,
    /// When the raster thread started the current frame, for the slow-frame log.
    frame_start: Mutex<Option<std::time::Instant>>,
    /// Asks the engine for one more frame; set by the embedder.
    redraw: Mutex<Option<Box<dyn Fn() + Send>>>,
    renderer: String,
}

// SAFETY: the EGL display, contexts and surface are handles that the EGL API allows to use
// from any thread (a context is current on one thread at a time, and the engine follows
// this rule: window on the raster thread, resources on the IO thread); the EGL window is
// protected by the Mutex.
unsafe impl Send for GlSurface {}
// SAFETY: see above; no method exposes mutable references without the Mutex.
unsafe impl Sync for GlSurface {}

impl GlSurface {
    /// Creates the EGL display, contexts and surface for `surface`, of `size` physical pixels.
    pub fn new(conn: &Connection, surface: &WlSurface, size: (u32, u32)) -> Result<Self, Error> {
        // SAFETY: loads the system libEGL (libglvnd or Mesa), which implements the standard
        // EGL 1.5 API checked by load_required.
        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
            .map_err(|err| Error::Gl("libEGL", err.to_string()))?;
        let wl_display = conn.backend().display_ptr().cast::<c_void>();
        // SAFETY: wl_display is the libwayland display of the connection, alive as long as
        // conn, and the declared platform is Wayland.
        let display =
            unsafe { egl.get_platform_display(PLATFORM_WAYLAND, wl_display, &[egl::ATTRIB_NONE]) }
                .map_err(egl_error("eglGetPlatformDisplay"))?;
        egl.initialize(display)
            .map_err(egl_error("eglInitialize"))?;
        egl.bind_api(egl::OPENGL_ES_API)
            .map_err(egl_error("eglBindAPI"))?;
        let config = egl
            .choose_first_config(
                display,
                &[
                    egl::SURFACE_TYPE,
                    egl::WINDOW_BIT,
                    egl::RENDERABLE_TYPE,
                    egl::OPENGL_ES2_BIT,
                    egl::RED_SIZE,
                    8,
                    egl::GREEN_SIZE,
                    8,
                    egl::BLUE_SIZE,
                    8,
                    egl::ALPHA_SIZE,
                    8,
                    egl::STENCIL_SIZE,
                    8,
                    egl::NONE,
                ],
            )
            .map_err(egl_error("eglChooseConfig"))?
            .ok_or(Error::Gl(
                "eglChooseConfig",
                "no RGBA8 configuration".to_owned(),
            ))?;
        let attributes = [egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE];
        let context = egl
            .create_context(display, config, None, &attributes)
            .map_err(egl_error("eglCreateContext"))?;
        let resource_context = egl
            .create_context(display, config, Some(context), &attributes)
            .map_err(egl_error("eglCreateContext"))?;

        let (width, height) = (
            i32::try_from(size.0.max(1)).unwrap_or(1),
            i32::try_from(size.1.max(1)).unwrap_or(1),
        );
        let window = WlEglSurface::new(surface.id(), width, height)
            .map_err(|err| Error::Gl("wl_egl_window_create", err.to_string()))?;
        // SAFETY: window.ptr() is the wl_egl_window just created on the app surface;
        // GlSurface keeps it alive longer than the EGL surface (Drop).
        let surface = unsafe {
            egl.create_platform_window_surface(
                display,
                config,
                window.ptr().cast_mut(),
                &[egl::ATTRIB_NONE],
            )
        }
        .map_err(egl_error("eglCreatePlatformWindowSurface"))?;

        let mut gl = Self {
            egl,
            display,
            context,
            resource_context,
            surface,
            window: Mutex::new((window, size)),
            resized: AtomicBool::new(false),
            presented: AtomicBool::new(false),
            frame_start: Mutex::new(None),
            redraw: Mutex::new(None),
            renderer: String::new(),
        };
        // The swap does not wait for the frame callback: the pace comes from the engine's
        // vsync, and the raster thread does not stay blocked if the compositor stops sending frames.
        if gl.make_current() {
            if let Err(err) = gl.egl.swap_interval(display, 0) {
                tracing::warn!("eglSwapInterval: {err}");
            }
            gl.renderer = gl.gl_renderer();
            gl.clear_current();
        }
        Ok(gl)
    }

    /// The GL renderer (driver and GPU), for the log and the measurements.
    pub fn renderer(&self) -> &str {
        &self.renderer
    }

    /// `glGetString(GL_RENDERER)`; needs a current context.
    fn gl_renderer(&self) -> String {
        let Some(get_string) = self.egl.get_proc_address("glGetString") else {
            return String::new();
        };
        // SAFETY: glGetString has this signature (GLES 2.0, `const GLubyte *glGetString(GLenum)`);
        // eglGetProcAddress returned its address.
        let get_string: GetStringFn = unsafe { std::mem::transmute(get_string) };
        // SAFETY: the context is current on this thread; GL_RENDERER is a valid name.
        let text = unsafe { get_string(GL_RENDERER) };
        if text.is_null() {
            return String::new();
        }
        // SAFETY: glGetString returns a static NUL-terminated string.
        unsafe { std::ffi::CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    }

    pub(super) fn make_current(&self) -> bool {
        if let Ok(mut start) = self.frame_start.lock() {
            *start = Some(std::time::Instant::now());
        }
        self.egl
            .make_current(
                self.display,
                Some(self.surface),
                Some(self.surface),
                Some(self.context),
            )
            .is_ok()
    }

    pub(super) fn clear_current(&self) -> bool {
        self.egl
            .make_current(self.display, None, None, None)
            .is_ok()
    }

    pub(super) fn make_resource_current(&self) -> bool {
        self.egl
            .make_current(self.display, None, None, Some(self.resource_context))
            .is_ok()
    }

    pub fn presented(&self) -> bool {
        self.presented.load(Ordering::Acquire)
    }

    /// Whom to call when one more frame is needed after a resize.
    pub fn on_redraw(&self, redraw: impl Fn() + Send + 'static) {
        if let Ok(mut slot) = self.redraw.lock() {
            *slot = Some(Box::new(redraw));
        }
    }

    pub(super) fn present(&self) -> bool {
        let swap_start = std::time::Instant::now();
        let swapped = self.egl.swap_buffers(self.display, self.surface).is_ok();
        if let Ok(start) = self.frame_start.lock()
            && let Some(start) = *start
            && start.elapsed() > std::time::Duration::from_millis(12)
        {
            tracing::debug!(
                "slow raster frame: {:.1} ms, of which swap {:.1} ms",
                start.elapsed().as_secs_f64() * 1000.0,
                swap_start.elapsed().as_secs_f64() * 1000.0
            );
        }
        if swapped {
            self.presented.store(true, Ordering::Release);
        }
        if self.resized.swap(false, Ordering::AcqRel)
            && let Ok(redraw) = self.redraw.lock()
            && let Some(redraw) = redraw.as_ref()
        {
            redraw();
        }
        swapped
    }

    /// Brings the EGL window to a new size; it applies from the next buffer.
    pub(super) fn resize(&self, width: u32, height: u32) {
        let Ok(mut window) = self.window.lock() else {
            return;
        };
        if window.1 == (width, height) || width == 0 || height == 0 {
            return;
        }
        let (Ok(w), Ok(h)) = (i32::try_from(width), i32::try_from(height)) else {
            return;
        };
        window.0.resize(w, h, 0, 0);
        window.1 = (width, height);
        self.resized.store(true, Ordering::Release);
    }

    pub(super) fn proc_address(&self, name: &str) -> *mut c_void {
        self.egl
            .get_proc_address(name)
            .map_or(std::ptr::null_mut(), |f| f as *mut c_void)
    }
}

impl Drop for GlSurface {
    fn drop(&mut self) {
        // The engine is already shut down: no thread holds the contexts current anymore.
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.destroy_surface(self.display, self.surface);
        let _ = self
            .egl
            .destroy_context(self.display, self.resource_context);
        let _ = self.egl.destroy_context(self.display, self.context);
        // The wl_egl_window is destroyed with the `window` field, after the EGL surface.
    }
}
