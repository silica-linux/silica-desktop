//! The engine's OpenGL ES renderer: EGL on the app's `wl_surface`.
//!
//! The engine calls these callbacks from the raster thread (window context, swap,
//! resize) and from the IO thread (resource context, shared with the first one).
//! `eglSwapBuffers` attaches and commits the surface by itself.
//!
//! With several views (`compositor.rs`) the engine draws into framebuffers ([`GlTarget`]) and
//! every view has its own EGL surface on its `wl_surface`: [`GlSurface::present_view`] blits the
//! framebuffer on it and swaps. The main surface stays the one the context is current on.

use std::collections::BTreeMap;
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

/// `GL_RGBA8`: the sized format of the backing stores' color texture.
pub const GL_RGBA8: u32 = 0x8058;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
const GL_NEAREST: u32 = 0x2600;
const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_READ_FRAMEBUFFER: u32 = 0x8CA8;
const GL_DRAW_FRAMEBUFFER: u32 = 0x8CA9;
const GL_RENDERBUFFER: u32 = 0x8D41;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_DEPTH_ATTACHMENT: u32 = 0x8D00;
const GL_STENCIL_ATTACHMENT: u32 = 0x8D20;
const GL_DEPTH24_STENCIL8: u32 = 0x88F0;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_READ_FRAMEBUFFER_BINDING: u32 = 0x8CAA;
const GL_RENDERBUFFER_BINDING: u32 = 0x8CA7;
const GL_TEXTURE_BINDING_2D: u32 = 0x8069;
const GL_SCISSOR_TEST: u32 = 0x0C11;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;

type GenFn = unsafe extern "C" fn(i32, *mut u32);
type DeleteFn = unsafe extern "C" fn(i32, *const u32);
type BindFn = unsafe extern "C" fn(u32, u32);
type AttachTextureFn = unsafe extern "C" fn(u32, u32, u32, u32, i32);
type AttachRenderbufferFn = unsafe extern "C" fn(u32, u32, u32, u32);
type StatusFn = unsafe extern "C" fn(u32) -> u32;
type TexImageFn = unsafe extern "C" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void);
type TexParameterFn = unsafe extern "C" fn(u32, u32, i32);
type StorageFn = unsafe extern "C" fn(u32, u32, i32, i32);
type BlitFn = unsafe extern "C" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32);
type GetIntegerFn = unsafe extern "C" fn(u32, *mut i32);
type IsEnabledFn = unsafe extern "C" fn(u32) -> u8;
type CapabilityFn = unsafe extern "C" fn(u32);
type ClearColorFn = unsafe extern "C" fn(f32, f32, f32, f32);
type ClearFn = unsafe extern "C" fn(u32);

/// The GL entry points of the backing stores and their presentation (several views).
#[derive(Clone, Copy)]
struct GlFns {
    gen_framebuffers: GenFn,
    delete_framebuffers: DeleteFn,
    bind_framebuffer: BindFn,
    framebuffer_texture_2d: AttachTextureFn,
    framebuffer_renderbuffer: AttachRenderbufferFn,
    check_framebuffer_status: StatusFn,
    gen_textures: GenFn,
    delete_textures: DeleteFn,
    bind_texture: BindFn,
    tex_image_2d: TexImageFn,
    tex_parameteri: TexParameterFn,
    gen_renderbuffers: GenFn,
    delete_renderbuffers: DeleteFn,
    bind_renderbuffer: BindFn,
    renderbuffer_storage: StorageFn,
    blit_framebuffer: BlitFn,
    get_integerv: GetIntegerFn,
    is_enabled: IsEnabledFn,
    enable: CapabilityFn,
    disable: CapabilityFn,
    clear_color: ClearColorFn,
    clear: ClearFn,
}

impl GlFns {
    /// The entry points from `eglGetProcAddress` (GLES 3.0 for the blit); None if one is missing.
    fn load(egl: &egl::DynamicInstance<egl::EGL1_5>) -> Option<Self> {
        macro_rules! load {
            ($name:literal, $ty:ty) => {{
                let f = egl.get_proc_address($name)?;
                // SAFETY: the GLES function named here has the C signature `$ty` (GLES 3.0
                // headers); eglGetProcAddress returned its address.
                unsafe { std::mem::transmute::<extern "system" fn(), $ty>(f) }
            }};
        }
        Some(Self {
            gen_framebuffers: load!("glGenFramebuffers", GenFn),
            delete_framebuffers: load!("glDeleteFramebuffers", DeleteFn),
            bind_framebuffer: load!("glBindFramebuffer", BindFn),
            framebuffer_texture_2d: load!("glFramebufferTexture2D", AttachTextureFn),
            framebuffer_renderbuffer: load!("glFramebufferRenderbuffer", AttachRenderbufferFn),
            check_framebuffer_status: load!("glCheckFramebufferStatus", StatusFn),
            gen_textures: load!("glGenTextures", GenFn),
            delete_textures: load!("glDeleteTextures", DeleteFn),
            bind_texture: load!("glBindTexture", BindFn),
            tex_image_2d: load!("glTexImage2D", TexImageFn),
            tex_parameteri: load!("glTexParameteri", TexParameterFn),
            gen_renderbuffers: load!("glGenRenderbuffers", GenFn),
            delete_renderbuffers: load!("glDeleteRenderbuffers", DeleteFn),
            bind_renderbuffer: load!("glBindRenderbuffer", BindFn),
            renderbuffer_storage: load!("glRenderbufferStorage", StorageFn),
            blit_framebuffer: load!("glBlitFramebuffer", BlitFn),
            get_integerv: load!("glGetIntegerv", GetIntegerFn),
            is_enabled: load!("glIsEnabled", IsEnabledFn),
            enable: load!("glEnable", CapabilityFn),
            disable: load!("glDisable", CapabilityFn),
            clear_color: load!("glClearColor", ClearColorFn),
            clear: load!("glClear", ClearFn),
        })
    }

    /// `glGetIntegerv` of one binding, as a GL name.
    fn binding(&self, name: u32) -> u32 {
        let mut value = 0i32;
        // SAFETY: a context is current on this thread (the callers' contract); `name` is a
        // binding query that writes one integer.
        unsafe { (self.get_integerv)(name, &mut value) };
        u32::try_from(value).unwrap_or(0)
    }
}

/// A backing store: a framebuffer with an RGBA8 texture and a depth-stencil renderbuffer
/// (Impeller clips with the stencil). Deleted with the context current, on the raster thread.
pub struct GlTarget {
    fns: GlFns,
    framebuffer: u32,
    texture: u32,
    depth_stencil: u32,
}

impl GlTarget {
    pub fn framebuffer(&self) -> u32 {
        self.framebuffer
    }
}

impl Drop for GlTarget {
    fn drop(&mut self) {
        // SAFETY: the objects were created in this context, which the engine has current
        // when it releases a backing store (raster thread); each is deleted once.
        unsafe {
            (self.fns.delete_framebuffers)(1, &self.framebuffer);
            (self.fns.delete_textures)(1, &self.texture);
            (self.fns.delete_renderbuffers)(1, &self.depth_stencil);
        }
    }
}

/// The EGL surface of a view other than the main one.
struct ViewSurface {
    surface: egl::Surface,
    window: WlEglSurface,
    size: (u32, u32),
    /// `eglSwapInterval(0)` applies to the surface current: set at its first present.
    interval_set: bool,
    presented: bool,
}

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
    /// The configuration of the surfaces, for the views' surfaces.
    config: egl::Config,
    /// The other views' surfaces (several windows); the main view is `surface`.
    views: Mutex<BTreeMap<i64, ViewSurface>>,
    /// The entry points of the backing stores (None without GLES 3: one view only).
    fns: Option<GlFns>,
    /// The engine composes through backing stores: the main surface is resized when its frame
    /// is presented, not when the root frame starts (it may be another view's).
    multi_view: AtomicBool,
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
            config,
            views: Mutex::new(BTreeMap::new()),
            fns: None,
            multi_view: AtomicBool::new(false),
        };
        gl.fns = GlFns::load(&gl.egl);
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

    /// The engine composes the views through backing stores (`compositor.rs`).
    pub fn set_multi_view(&self) -> Result<(), Error> {
        if self.fns.is_none() {
            return Err(Error::Gl(
                "eglGetProcAddress",
                "several windows need OpenGL ES 3 (glBlitFramebuffer)".to_owned(),
            ));
        }
        self.multi_view.store(true, Ordering::Release);
        Ok(())
    }

    pub(super) fn multi_view(&self) -> bool {
        self.multi_view.load(Ordering::Acquire)
    }

    /// An EGL surface for view `view_id` on its `wl_surface`, of `size` physical pixels.
    pub fn add_view(
        &self,
        view_id: i64,
        surface: &WlSurface,
        size: (u32, u32),
    ) -> Result<(), Error> {
        let (width, height) = (
            i32::try_from(size.0.max(1)).unwrap_or(1),
            i32::try_from(size.1.max(1)).unwrap_or(1),
        );
        let window = WlEglSurface::new(surface.id(), width, height)
            .map_err(|err| Error::Gl("wl_egl_window_create", err.to_string()))?;
        // SAFETY: window.ptr() is the wl_egl_window just created on the view's surface; the
        // ViewSurface keeps it alive longer than the EGL surface (`remove_view`).
        let surface = unsafe {
            self.egl.create_platform_window_surface(
                self.display,
                self.config,
                window.ptr().cast_mut(),
                &[egl::ATTRIB_NONE],
            )
        }
        .map_err(egl_error("eglCreatePlatformWindowSurface"))?;
        let view = ViewSurface {
            surface,
            window,
            size: (size.0.max(1), size.1.max(1)),
            interval_set: false,
            presented: false,
        };
        if let Ok(mut views) = self.views.lock()
            && let Some(old) = views.insert(view_id, view)
        {
            let _ = self.egl.destroy_surface(self.display, old.surface);
        }
        Ok(())
    }

    /// Destroys the EGL surface of view `view_id` (after the engine removed the view).
    pub fn remove_view(&self, view_id: i64) {
        let Ok(mut views) = self.views.lock() else {
            return;
        };
        if let Some(view) = views.remove(&view_id) {
            // A surface still current somewhere is destroyed once released (EGL 1.5 § 3.2);
            // the wl_egl_window goes after it, with `view`.
            let _ = self.egl.destroy_surface(self.display, view.surface);
        }
    }

    /// Whether view `view_id` has presented a frame (the main view: [`Self::presented`]).
    pub fn view_presented(&self, view_id: i64) -> bool {
        if view_id == 0 {
            return self.presented();
        }
        self.views
            .lock()
            .ok()
            .and_then(|views| views.get(&view_id).map(|view| view.presented))
            .unwrap_or(false)
    }

    /// A backing store of `width` × `height` pixels; on the raster thread, context current.
    pub(super) fn create_target(&self, width: u32, height: u32) -> Option<GlTarget> {
        let fns = self.fns?;
        let (w, h) = (i32::try_from(width).ok()?, i32::try_from(height).ok()?);
        let previous = (
            fns.binding(GL_FRAMEBUFFER_BINDING),
            fns.binding(GL_TEXTURE_BINDING_2D),
            fns.binding(GL_RENDERBUFFER_BINDING),
        );
        let mut target = GlTarget {
            fns,
            framebuffer: 0,
            texture: 0,
            depth_stencil: 0,
        };
        // SAFETY: the engine's context is current on this thread (backing stores are created
        // on the raster thread during a frame); the calls create and bind objects of this
        // context with valid arguments, then restore the bindings found.
        let complete = unsafe {
            (fns.gen_textures)(1, &mut target.texture);
            (fns.bind_texture)(GL_TEXTURE_2D, target.texture);
            (fns.tex_image_2d)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA as i32,
                w,
                h,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
            (fns.tex_parameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as i32);
            (fns.tex_parameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as i32);
            (fns.gen_renderbuffers)(1, &mut target.depth_stencil);
            (fns.bind_renderbuffer)(GL_RENDERBUFFER, target.depth_stencil);
            (fns.renderbuffer_storage)(GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, w, h);
            (fns.gen_framebuffers)(1, &mut target.framebuffer);
            (fns.bind_framebuffer)(GL_FRAMEBUFFER, target.framebuffer);
            (fns.framebuffer_texture_2d)(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                target.texture,
                0,
            );
            for attachment in [GL_DEPTH_ATTACHMENT, GL_STENCIL_ATTACHMENT] {
                (fns.framebuffer_renderbuffer)(
                    GL_FRAMEBUFFER,
                    attachment,
                    GL_RENDERBUFFER,
                    target.depth_stencil,
                );
            }
            let status = (fns.check_framebuffer_status)(GL_FRAMEBUFFER);
            (fns.bind_framebuffer)(GL_FRAMEBUFFER, previous.0);
            (fns.bind_texture)(GL_TEXTURE_2D, previous.1);
            (fns.bind_renderbuffer)(GL_RENDERBUFFER, previous.2);
            status == GL_FRAMEBUFFER_COMPLETE
        };
        if !complete {
            tracing::warn!("backing store {width}×{height}: incomplete framebuffer");
            return None;
        }
        Some(target)
    }

    /// Presents a view: blits `framebuffer` (`size` pixels) on the view's EGL surface and
    /// swaps, then makes the main surface current again. On the raster thread.
    pub(super) fn present_view(&self, view_id: i64, framebuffer: u32, size: (u32, u32)) -> bool {
        let Some(fns) = self.fns else {
            return false;
        };
        let (Ok(w), Ok(h)) = (i32::try_from(size.0), i32::try_from(size.1)) else {
            return false;
        };
        let blit = |surface: egl::Surface, first: bool| -> bool {
            if self
                .egl
                .make_current(
                    self.display,
                    Some(surface),
                    Some(surface),
                    Some(self.context),
                )
                .is_err()
            {
                return false;
            }
            if first && let Err(err) = self.egl.swap_interval(self.display, 0) {
                tracing::warn!("eglSwapInterval: {err}");
            }
            let read = fns.binding(GL_READ_FRAMEBUFFER_BINDING);
            let draw = fns.binding(GL_FRAMEBUFFER_BINDING);
            // SAFETY: the context is current with the view's surface; the blit copies the
            // backing store (a complete framebuffer of this context, `size` pixels) on the
            // surface's back buffer, then the bindings and the scissor test are restored.
            unsafe {
                let scissor = (fns.is_enabled)(GL_SCISSOR_TEST) != 0;
                if scissor {
                    (fns.disable)(GL_SCISSOR_TEST);
                }
                (fns.bind_framebuffer)(GL_READ_FRAMEBUFFER, framebuffer);
                (fns.bind_framebuffer)(GL_DRAW_FRAMEBUFFER, 0);
                (fns.blit_framebuffer)(0, 0, w, h, 0, 0, w, h, GL_COLOR_BUFFER_BIT, GL_NEAREST);
                (fns.bind_framebuffer)(GL_READ_FRAMEBUFFER, read);
                (fns.bind_framebuffer)(GL_DRAW_FRAMEBUFFER, draw);
                if scissor {
                    (fns.enable)(GL_SCISSOR_TEST);
                }
            }
            self.egl.swap_buffers(self.display, surface).is_ok()
        };
        let swapped = if view_id == 0 {
            // Mesa sizes the back buffer when it is first drawn: the window first.
            self.resize(size.0, size.1);
            let swapped = blit(self.surface, false);
            if swapped {
                self.presented.store(true, Ordering::Release);
            }
            swapped
        } else {
            let Ok(mut views) = self.views.lock() else {
                return false;
            };
            let Some(view) = views.get_mut(&view_id) else {
                // Removed while this frame was drawn.
                return true;
            };
            if view.size != size {
                view.window.resize(w, h, 0, 0);
                view.size = size;
            }
            let first = !view.interval_set;
            view.interval_set = true;
            let swapped = blit(view.surface, first);
            view.presented |= swapped;
            swapped
        };
        // The engine keeps drawing with its context on the main surface.
        let _ = self.egl.make_current(
            self.display,
            Some(self.surface),
            Some(self.surface),
            Some(self.context),
        );
        if view_id == 0
            && self.resized.swap(false, Ordering::AcqRel)
            && let Ok(redraw) = self.redraw.lock()
            && let Some(redraw) = redraw.as_ref()
        {
            redraw();
        }
        swapped
    }

    /// Presents a fully transparent frame on view `view_id` (the engine draws no layer for
    /// an empty scene, `compositor.rs`). On the raster thread.
    pub(super) fn clear_view(&self, view_id: i64) -> bool {
        let Some(fns) = self.fns else {
            return false;
        };
        let clear = |surface: egl::Surface| -> bool {
            if self
                .egl
                .make_current(
                    self.display,
                    Some(surface),
                    Some(surface),
                    Some(self.context),
                )
                .is_err()
            {
                return false;
            }
            let draw = fns.binding(GL_FRAMEBUFFER_BINDING);
            // SAFETY: the context is current with the view's surface; the default framebuffer
            // is cleared, then the binding and the scissor test are restored.
            unsafe {
                let scissor = (fns.is_enabled)(GL_SCISSOR_TEST) != 0;
                if scissor {
                    (fns.disable)(GL_SCISSOR_TEST);
                }
                (fns.bind_framebuffer)(GL_FRAMEBUFFER, 0);
                (fns.clear_color)(0.0, 0.0, 0.0, 0.0);
                (fns.clear)(GL_COLOR_BUFFER_BIT);
                (fns.bind_framebuffer)(GL_FRAMEBUFFER, draw);
                if scissor {
                    (fns.enable)(GL_SCISSOR_TEST);
                }
            }
            self.egl.swap_buffers(self.display, surface).is_ok()
        };
        let cleared = if view_id == 0 {
            clear(self.surface)
        } else {
            match self.views.lock() {
                Ok(views) => views.get(&view_id).is_none_or(|view| clear(view.surface)),
                Err(_) => false,
            }
        };
        // The engine keeps drawing with its context on the main surface.
        let _ = self.egl.make_current(
            self.display,
            Some(self.surface),
            Some(self.surface),
            Some(self.context),
        );
        cleared
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
        if let Ok(views) = self.views.get_mut() {
            for (_, view) in std::mem::take(views) {
                let _ = self.egl.destroy_surface(self.display, view.surface);
            }
        }
        let _ = self.egl.destroy_surface(self.display, self.surface);
        let _ = self
            .egl
            .destroy_context(self.display, self.resource_context);
        let _ = self.egl.destroy_context(self.display, self.context);
        // The wl_egl_window is destroyed with the `window` field, after the EGL surface.
    }
}
