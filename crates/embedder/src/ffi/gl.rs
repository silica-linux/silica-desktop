//! Il renderer OpenGL ES dell'engine: EGL sulla `wl_surface` dell'app.
//!
//! L'engine chiama questi callback dal thread raster (contesto della finestra, swap,
//! ridimensionamento) e dal thread IO (contesto delle risorse, condiviso con il primo).
//! `eglSwapBuffers` fa da sé attach e commit della superficie.

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
/// GL_RENDERER, per `glGetString`.
const GL_RENDERER: u32 = 0x1F01;

type GetStringFn = unsafe extern "C" fn(u32) -> *const std::ffi::c_char;

fn egl_error(call: &'static str) -> impl Fn(egl::Error) -> Error {
    move |err| Error::Gl(call, err.to_string())
}

/// Contesti e superficie EGL dell'app. Vive in [`super::Callbacks`] finché l'engine gira.
pub struct GlSurface {
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    context: egl::Context,
    resource_context: egl::Context,
    surface: egl::Surface,
    /// La finestra EGL e la sua dimensione in pixel fisici; la ridimensiona il thread raster.
    window: Mutex<(WlEglSurface, (u32, u32))>,
    /// Ridimensionata dall'ultimo swap: Mesa (almeno il percorso software di WSL) applica
    /// la dimensione nuova solo dal buffer successivo, quindi serve un altro frame.
    resized: AtomicBool,
    /// Almeno un frame è arrivato al compositor: da qui i frame callback funzionano.
    presented: AtomicBool,
    /// Quando il thread raster ha iniziato il frame in corso, per il log dei frame lenti.
    frame_start: Mutex<Option<std::time::Instant>>,
    /// Chiede all'engine un frame in più; lo imposta l'embedder.
    redraw: Mutex<Option<Box<dyn Fn() + Send>>>,
    renderer: String,
}

// SAFETY: display, contesti e superficie EGL sono handle che l'API EGL consente di usare
// da qualsiasi thread (un contesto è corrente su un solo thread alla volta, e l'engine
// rispetta questa regola: finestra sul raster, risorse sull'IO); la finestra EGL è
// protetta dal Mutex.
unsafe impl Send for GlSurface {}
// SAFETY: vedi sopra; nessun metodo espone riferimenti mutabili senza il Mutex.
unsafe impl Sync for GlSurface {}

impl GlSurface {
    /// Crea display, contesti e superficie EGL per `surface`, di `size` pixel fisici.
    pub fn new(conn: &Connection, surface: &WlSurface, size: (u32, u32)) -> Result<Self, Error> {
        // SAFETY: carica la libEGL di sistema (libglvnd o Mesa), che implementa l'API EGL
        // 1.5 standard verificata da load_required.
        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
            .map_err(|err| Error::Gl("libEGL", err.to_string()))?;
        let wl_display = conn.backend().display_ptr().cast::<c_void>();
        // SAFETY: wl_display è il display libwayland della connessione, vivo quanto conn,
        // e la piattaforma dichiarata è Wayland.
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
                "nessuna configurazione RGBA8".to_owned(),
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
        // SAFETY: window.ptr() è la wl_egl_window appena creata sulla superficie dell'app;
        // GlSurface la tiene viva più a lungo della superficie EGL (Drop).
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
        // Lo swap non aspetta il frame callback: il ritmo lo dà il vsync dell'engine, e il
        // thread raster non resta bloccato se il compositor smette di mandare frame.
        if gl.make_current() {
            if let Err(err) = gl.egl.swap_interval(display, 0) {
                tracing::warn!("eglSwapInterval: {err}");
            }
            gl.renderer = gl.gl_renderer();
            gl.clear_current();
        }
        Ok(gl)
    }

    /// Il renderer GL (driver e GPU), per il log e le misure.
    pub fn renderer(&self) -> &str {
        &self.renderer
    }

    /// `glGetString(GL_RENDERER)`; serve un contesto corrente.
    fn gl_renderer(&self) -> String {
        let Some(get_string) = self.egl.get_proc_address("glGetString") else {
            return String::new();
        };
        // SAFETY: glGetString ha questa firma (GLES 2.0, `const GLubyte *glGetString(GLenum)`);
        // eglGetProcAddress ne ha restituito l'indirizzo.
        let get_string: GetStringFn = unsafe { std::mem::transmute(get_string) };
        // SAFETY: il contesto è corrente su questo thread; GL_RENDERER è un nome valido.
        let text = unsafe { get_string(GL_RENDERER) };
        if text.is_null() {
            return String::new();
        }
        // SAFETY: glGetString restituisce una stringa statica terminata da NUL.
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

    /// Chi chiamare quando serve un frame in più dopo un ridimensionamento.
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
                "frame raster lento: {:.1} ms, di cui swap {:.1} ms",
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

    /// Porta la finestra EGL a una nuova dimensione; vale dal prossimo buffer.
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
        // L'engine è già spento: nessun thread ha più i contesti correnti.
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.destroy_surface(self.display, self.surface);
        let _ = self
            .egl
            .destroy_context(self.display, self.resource_context);
        let _ = self.egl.destroy_context(self.display, self.context);
        // La wl_egl_window si distrugge con il campo `window`, dopo la superficie EGL.
    }
}
