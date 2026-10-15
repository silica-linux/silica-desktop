//! The engine's compositor callbacks, for several views (windows) in one engine
//! (silica-desktop#146).
//!
//! With a `FlutterCompositor` the engine draws every view into backing stores the embedder
//! hands out, then calls `present_view` with the view's id and its layers. Without one it can
//! only draw the implicit view: `FlutterEngineAddView` needs this path.
//!
//! - Software: a heap buffer per backing store; `present_view` copies it into a [`Frame`] for
//!   the main thread, as the single-view software renderer does.
//! - OpenGL ES (Skia or Impeller): a framebuffer with a texture and a depth-stencil
//!   renderbuffer per backing store; `present_view` blits it on the view's EGL surface and
//!   swaps ([`super::GlSurface::present_view`]).
//! - Vulkan (#203): the backing store is the next image of the view's own swapchain;
//!   `present_view` presents it, with no copy ([`super::VulkanSurface::acquire_target`]).
//!   The engine asks for a new backing store every frame (`avoid_backing_store_cache`): the
//!   image it drew last is the compositor's until the swapchain gives it back. Impeller has
//!   Vulkan backing stores only in an engine with the patch of `crates/embedder/engine-patches`;
//!   otherwise the engine draws them with Skia (`window.rs`, `multi_view_renderer`).
//!
//! With OpenGL ES the root surface is still acquired and submitted by the rasterizer for every
//! view; its present callbacks do nothing in this mode (`present`, `gl_present` in `mod.rs`).
//! With Vulkan the engine does not draw the root surface at all when there is a compositor.

use std::ffi::c_void;

use super::bindings::{
    FlutterBackingStore, FlutterBackingStoreConfig, FlutterBackingStoreType,
    FlutterLayerContentType, FlutterOpenGLBackingStore, FlutterOpenGLBackingStore__bindgen_ty_1,
    FlutterOpenGLFramebuffer, FlutterOpenGLTargetType, FlutterPresentViewInfo,
    FlutterSoftwareBackingStore, FlutterVulkanBackingStore, FlutterVulkanImage,
};
use super::gl::{GL_RGBA8, GlTarget};
use super::vulkan::VulkanTarget;
use super::{Frame, callbacks};

/// A Vulkan backing store: the view's swapchain image and its description for the engine,
/// which points into this allocation (the `user_data` of the `FlutterBackingStore`, freed in
/// `collect_backing_store`).
struct VulkanStore {
    target: VulkanTarget,
    image: FlutterVulkanImage,
}

/// The image stays the swapchain's: nothing to free when the engine lets it go (Skia calls
/// this when its surface goes; the store itself is freed in `collect_backing_store`).
unsafe extern "C" fn keep_vulkan_image(_user_data: *mut c_void) {}

/// A backing store's side in physical pixels: at least 1, at most 16384.
fn side(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 0.5).then(|| value.round().min(16384.0) as u32)
}

/// The heap buffer of a software backing store, freed by the engine's destruction callback.
type Pixels = Box<[u8]>;

unsafe extern "C" fn free_pixels(user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    // SAFETY: user_data is the Box<Pixels> leaked in `create_backing_store`; the engine calls
    // the destruction callback once, when it no longer draws into the buffer.
    drop(unsafe { Box::from_raw(user_data.cast::<Pixels>()) });
}

unsafe extern "C" fn free_gl_target(user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    // SAFETY: user_data is the Box<GlTarget> leaked in `create_backing_store`; the engine calls
    // the destruction callback once, on the raster thread with its context current (the GL
    // objects are deleted in that context).
    drop(unsafe { Box::from_raw(user_data.cast::<GlTarget>()) });
}

pub(super) unsafe extern "C" fn create_backing_store(
    config: *const FlutterBackingStoreConfig,
    out: *mut FlutterBackingStore,
    user_data: *mut c_void,
) -> bool {
    if config.is_null() || out.is_null() {
        return false;
    }
    // SAFETY: user_data is the compositor's, the Callbacks given to FlutterEngineRun.
    let callbacks = unsafe { callbacks(user_data) };
    // SAFETY: the engine passes a config valid for the duration of the call.
    let config = unsafe { &*config };
    let (Some(width), Some(height)) = (side(config.size.width), side(config.size.height)) else {
        return false;
    };
    // SAFETY: the engine passes a backing store to fill, valid for the duration of the call
    // (it set struct_size, which is kept).
    let out = unsafe { &mut *out };
    out.user_data = std::ptr::null_mut();
    out.did_update = true;
    if let Some(vulkan) = &callbacks.vulkan {
        let Some(target) = vulkan.acquire_target(config.view_id, (width, height)) else {
            return false;
        };
        let image = FlutterVulkanImage {
            struct_size: size_of::<FlutterVulkanImage>(),
            image: target.image,
            format: target.format,
        };
        let store = Box::into_raw(Box::new(VulkanStore { target, image }));
        out.type_ = FlutterBackingStoreType::kFlutterBackingStoreTypeVulkan;
        out.user_data = store.cast::<c_void>();
        out.__bindgen_anon_1.vulkan = FlutterVulkanBackingStore {
            struct_size: size_of::<FlutterVulkanBackingStore>(),
            // SAFETY: store was just leaked from a valid Box and stays where it is until
            // collect_backing_store takes it back; the engine reads the image while it creates
            // its render target.
            image: unsafe { &raw const (*store).image },
            user_data: std::ptr::null_mut(),
            destruction_callback: Some(keep_vulkan_image),
        };
        return true;
    }
    if let Some(gl) = &callbacks.gl {
        let Some(target) = gl.create_target(width, height) else {
            return false;
        };
        let name = target.framebuffer();
        out.type_ = FlutterBackingStoreType::kFlutterBackingStoreTypeOpenGL;
        out.__bindgen_anon_1.open_gl = FlutterOpenGLBackingStore {
            type_: FlutterOpenGLTargetType::kFlutterOpenGLTargetTypeFramebuffer,
            __bindgen_anon_1: FlutterOpenGLBackingStore__bindgen_ty_1 {
                framebuffer: FlutterOpenGLFramebuffer {
                    target: GL_RGBA8,
                    name,
                    user_data: Box::into_raw(Box::new(target)).cast::<c_void>(),
                    destruction_callback: Some(free_gl_target),
                },
            },
        };
        return true;
    }
    let (Some(row_bytes), Ok(rows)) = ((width as usize).checked_mul(4), usize::try_from(height))
    else {
        return false;
    };
    let Some(len) = row_bytes.checked_mul(rows) else {
        return false;
    };
    let pixels: Box<Pixels> = Box::new(vec![0u8; len].into_boxed_slice());
    let raw = Box::into_raw(pixels);
    // SAFETY: raw was just leaked from a valid Box; the slice stays where it is until
    // free_pixels takes the Box back.
    let allocation = unsafe { (*raw).as_mut_ptr() };
    out.type_ = FlutterBackingStoreType::kFlutterBackingStoreTypeSoftware;
    out.__bindgen_anon_1.software = FlutterSoftwareBackingStore {
        allocation: allocation.cast_const().cast::<c_void>(),
        row_bytes,
        height: rows,
        user_data: raw.cast::<c_void>(),
        destruction_callback: Some(free_pixels),
    };
    true
}

/// The software and OpenGL resources go with the destruction callbacks of the stores
/// themselves; a Vulkan store gives its image back to the view's swapchain here.
pub(super) unsafe extern "C" fn collect_backing_store(
    store: *const FlutterBackingStore,
    user_data: *mut c_void,
) -> bool {
    if store.is_null() {
        return true;
    }
    // SAFETY: the engine passes one of the stores create_backing_store filled, valid for the
    // call.
    let store = unsafe { &*store };
    if store.type_ != FlutterBackingStoreType::kFlutterBackingStoreTypeVulkan
        || store.user_data.is_null()
    {
        return true;
    }
    // SAFETY: a Vulkan store's user_data is the VulkanStore leaked in create_backing_store; the
    // engine collects every store once.
    let vulkan_store = unsafe { Box::from_raw(store.user_data.cast::<VulkanStore>()) };
    // SAFETY: user_data is the compositor's, the Callbacks given to FlutterEngineRun.
    if let Some(vulkan) = &unsafe { callbacks(user_data) }.vulkan {
        vulkan.release_target(&vulkan_store.target);
    }
    true
}

pub(super) unsafe extern "C" fn present_view(info: *const FlutterPresentViewInfo) -> bool {
    if info.is_null() {
        return false;
    }
    // SAFETY: the engine passes an info valid for the duration of the call.
    let info = unsafe { &*info };
    // SAFETY: info.user_data is the compositor's, the Callbacks given to FlutterEngineRun.
    let callbacks = unsafe { callbacks(info.user_data) };
    let layers = if info.layers.is_null() || info.layers_count == 0 {
        &[][..]
    } else {
        // SAFETY: the engine passes layers_count valid layer pointers for the call.
        unsafe { std::slice::from_raw_parts(info.layers, info.layers_count) }
    };
    // The layer Flutter drew (there are no platform views: one layer per frame).
    let store = layers
        .iter()
        .filter(|layer| !layer.is_null())
        // SAFETY: every non-null layer pointer is valid during the call.
        .map(|layer| unsafe { &**layer })
        .find(|layer| layer.type_ == FlutterLayerContentType::kFlutterLayerContentTypeBackingStore)
        // SAFETY: the content type says the union holds a backing store pointer.
        .map(|layer| (unsafe { layer.__bindgen_anon_1.backing_store }, layer.size));
    let Some((store, size)) = store.filter(|(store, _)| !store.is_null()) else {
        return true;
    };
    // SAFETY: a non-null backing store of a layer is valid during the call; it is one of
    // ours (create_backing_store), filled with the type it says.
    let store = unsafe { &*store };
    if store.type_ == FlutterBackingStoreType::kFlutterBackingStoreTypeVulkan {
        let (Some(vulkan), false) = (&callbacks.vulkan, store.user_data.is_null()) else {
            return false;
        };
        // SAFETY: a Vulkan store's user_data is our VulkanStore, alive until it is collected
        // (after this call).
        let vulkan_store = unsafe { &*store.user_data.cast::<VulkanStore>() };
        // The present commits the view's surface: what goes with this frame is sent before it.
        callbacks.frames.before_present(info.view_id);
        return vulkan.present_target(&vulkan_store.target);
    }
    if store.type_ == FlutterBackingStoreType::kFlutterBackingStoreTypeOpenGL {
        let Some(gl) = &callbacks.gl else {
            return false;
        };
        // SAFETY: an OpenGL store of ours always holds a framebuffer.
        let framebuffer = unsafe { store.__bindgen_anon_1.open_gl.__bindgen_anon_1.framebuffer };
        let (Some(width), Some(height)) = (side(size.width), side(size.height)) else {
            return false;
        };
        // The swap commits the view's surface: what goes with this frame is sent before it.
        callbacks.frames.before_present(info.view_id);
        return gl.present_view(info.view_id, framebuffer.name, (width, height));
    }
    if store.type_ != FlutterBackingStoreType::kFlutterBackingStoreTypeSoftware {
        return false;
    }
    // SAFETY: the type says the union holds a software store.
    let software = unsafe { store.__bindgen_anon_1.software };
    let Some(len) = software.row_bytes.checked_mul(software.height) else {
        return false;
    };
    if software.allocation.is_null() || len == 0 {
        return false;
    }
    // SAFETY: the allocation is the buffer of row_bytes × height bytes created in
    // create_backing_store, alive until its destruction callback; it is copied here.
    let pixels =
        unsafe { std::slice::from_raw_parts(software.allocation.cast::<u8>(), len) }.to_vec();
    let (Ok(width), Ok(height)) = (
        u32::try_from(software.row_bytes / 4),
        u32::try_from(software.height),
    ) else {
        return false;
    };
    (callbacks.present)(Frame {
        view_id: info.view_id,
        width,
        height,
        stride: software.row_bytes,
        pixels,
        raster_seq: callbacks.frames.running(),
    });
    true
}

#[cfg(test)]
mod tests {
    use super::side;

    #[test]
    fn backing_store_sides_are_bounded() {
        assert_eq!(side(300.0), Some(300));
        assert_eq!(side(299.6), Some(300));
        assert_eq!(side(0.2), None);
        assert_eq!(side(f64::NAN), None);
        assert_eq!(side(-4.0), None);
        assert_eq!(side(1e9), Some(16384));
    }
}
