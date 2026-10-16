//! The C ABI of `libsilica_assets.so`, documented in `include/silica_assets.h`.
//!
//! Every function checks its pointers for null, converts enums with a checked conversion and
//! runs inside `catch_unwind`, so no input makes it unwind into C. Failures return 0 / NULL /
//! `SILICA_ASSETS_NONE`; `silica_assets_last_error` says why the last `open` failed.

#![allow(unsafe_code)]
// The functions are `unsafe extern "C"`-free on purpose: pointer validity is the caller's
// contract, documented in the header, and each dereference below carries its SAFETY comment.
#![allow(clippy::missing_safety_doc)]
// The pointer arguments are checked for null and documented in the header; making the
// functions `unsafe` would only move the burden to every Rust test, not to C.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use crate::format::*;
use crate::reader::{Assets, EntryId, Query, Variant};

/// Returned by `silica_assets_lookup` for a key that is not in the file.
pub const SILICA_ASSETS_NONE: u32 = u32::MAX;

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn set_error(message: &str) {
    let text = CString::new(message.replace('\0', " ")).unwrap_or_default();
    LAST_ERROR.with(|slot| *slot.borrow_mut() = text);
}

/// An entry as seen from C.
#[repr(C)]
pub struct SilicaEntryInfo {
    /// NUL-terminated UTF-8, inside the mapping.
    pub name: *const c_char,
    pub name_len: usize,
    pub kind: u32,
    pub flags: u32,
    /// The entry an alias points to, or `SILICA_ASSETS_NONE`.
    pub alias_of: u32,
    pub variant_count: u32,
    pub slot_count: u32,
}

/// A variant as seen from C.
#[repr(C)]
pub struct SilicaVariant {
    pub theme: u32,
    pub layer: u32,
    pub encoding: u32,
    pub state: u32,
    pub size_px: u32,
    pub width: u32,
    pub height: u32,
    /// The bytes, inside the mapping, valid until `silica_assets_close`.
    pub data: *const u8,
    pub len: usize,
}

/// A dynamic slot as seen from C.
#[repr(C)]
pub struct SilicaSlot {
    pub kind: u32,
    pub unit: u32,
    pub min_size: u32,
    pub size: u32,
    pub weight: u32,
    pub rect: [f32; 4],
    /// NUL-terminated, or NULL if the slot has no font.
    pub font: *const c_char,
    pub font_len: usize,
    pub color: [u32; 2],
    /// NUL-terminated, or NULL.
    pub format: *const c_char,
    pub format_len: usize,
    pub tracking: f32,
    pub max_width: f32,
}

fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(default)
}

fn fill_variant(v: &Variant<'_>, out: &mut SilicaVariant) {
    out.theme = v.theme as u32;
    out.layer = v.layer as u32;
    out.encoding = v.encoding as u32;
    out.state = v.state as u32;
    out.size_px = u32::from(v.size_px);
    out.width = u32::from(v.width);
    out.height = u32::from(v.height);
    out.data = v.data.as_ptr();
    out.len = v.data.len();
}

/// Opens and checks `path`. Returns NULL on failure (see `silica_assets_last_error`).
/// Must be released with `silica_assets_close`.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_open(path: *const c_char) -> *mut Assets {
    guard(std::ptr::null_mut(), || {
        if path.is_null() {
            set_error("null path");
            return std::ptr::null_mut();
        }
        // SAFETY: the caller passes a NUL-terminated string (documented in the header);
        // null was excluded above.
        let c_path = unsafe { CStr::from_ptr(path) };
        let Ok(text) = c_path.to_str() else {
            set_error("path is not UTF-8");
            return std::ptr::null_mut();
        };
        match Assets::open(Path::new(text)) {
            Ok(assets) => Box::into_raw(Box::new(assets)),
            Err(e) => {
                set_error(&e.to_string());
                std::ptr::null_mut()
            }
        }
    })
}

/// Releases a handle from `silica_assets_open`. NULL is ignored. Every pointer obtained from
/// the handle is invalid afterwards.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_close(handle: *mut Assets) {
    if handle.is_null() {
        return;
    }
    guard((), || {
        // SAFETY: the pointer came from `Box::into_raw` in `silica_assets_open` and the caller
        // closes each handle once (documented in the header).
        drop(unsafe { Box::from_raw(handle) });
    });
}

/// The reason the last `silica_assets_open` of this thread failed (NUL-terminated, valid until
/// the next call of this library on the thread).
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

/// The format version this library reads.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_format_version() -> u32 {
    FORMAT
}

/// The content version of the file (NUL-terminated, valid until close), or NULL.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_content_version(handle: *const Assets) -> *const c_char {
    guard(std::ptr::null(), || {
        // SAFETY: a non-null handle comes from `silica_assets_open` and is not yet closed.
        let Some(assets) = (unsafe { handle.as_ref() }) else {
            return std::ptr::null();
        };
        assets.content_version_ptr()
    })
}

/// The number of entries, including aliases.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_entry_count(handle: *const Assets) -> u32 {
    guard(0, || {
        // SAFETY: a non-null handle comes from `silica_assets_open` and is not yet closed.
        let Some(assets) = (unsafe { handle.as_ref() }) else {
            return 0;
        };
        u32::try_from(assets.entry_count()).unwrap_or(0)
    })
}

/// Looks `key` (`key_len` bytes of UTF-8, not necessarily NUL-terminated) up in key space
/// `space` (0 app, 1 type, 2 mime, 3 uti, 4 role, 5 symbol, 6 font, 7 sound, 8 wallpaper,
/// 9 ext). Aliases are resolved. Returns an entry index or `SILICA_ASSETS_NONE`.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_lookup(
    handle: *const Assets,
    space: u32,
    key: *const u8,
    key_len: usize,
) -> u32 {
    guard(SILICA_ASSETS_NONE, || {
        // SAFETY: a non-null handle comes from `silica_assets_open` and is not yet closed.
        let Some(assets) = (unsafe { handle.as_ref() }) else {
            return SILICA_ASSETS_NONE;
        };
        let Some(space) = KeySpace::from_u32(space) else {
            return SILICA_ASSETS_NONE;
        };
        if key.is_null() {
            return SILICA_ASSETS_NONE;
        }
        // SAFETY: the caller guarantees `key` points to `key_len` readable bytes.
        let bytes = unsafe { std::slice::from_raw_parts(key, key_len) };
        let Ok(text) = std::str::from_utf8(bytes) else {
            return SILICA_ASSETS_NONE;
        };
        assets
            .lookup(space, text)
            .map_or(SILICA_ASSETS_NONE, |id| id.0)
    })
}

/// Describes an entry. Returns 1 on success, 0 if the index is out of range.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_entry_info(
    handle: *const Assets,
    entry: u32,
    out: *mut SilicaEntryInfo,
) -> i32 {
    guard(0, || {
        // SAFETY: a non-null handle comes from `silica_assets_open` and is not yet closed;
        // a non-null `out` points to a writable `SilicaEntryInfo` (header contract).
        let (Some(assets), Some(out)) = (unsafe { handle.as_ref() }, unsafe { out.as_mut() })
        else {
            return 0;
        };
        let Some(e) = assets.entry(EntryId(entry)) else {
            return 0;
        };
        out.name = e.name().as_ptr().cast();
        out.name_len = e.name().len();
        out.kind = e.kind() as u32;
        out.flags = u32::from(e.flags());
        out.alias_of = e.alias_of().map_or(SILICA_ASSETS_NONE, |id| id.0);
        out.variant_count = u32::try_from(e.variant_count()).unwrap_or(0);
        out.slot_count = u32::try_from(e.slot_count()).unwrap_or(0);
        1
    })
}

/// Describes variant `index` of an entry. Returns 1 on success, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_variant(
    handle: *const Assets,
    entry: u32,
    index: u32,
    out: *mut SilicaVariant,
) -> i32 {
    guard(0, || {
        // SAFETY: as in `silica_assets_entry_info`.
        let (Some(assets), Some(out)) = (unsafe { handle.as_ref() }, unsafe { out.as_mut() })
        else {
            return 0;
        };
        let Some(v) = assets
            .entry(EntryId(entry))
            .and_then(|e| e.variant(index as usize))
        else {
            return 0;
        };
        fill_variant(&v, out);
        1
    })
}

/// Picks the variant to draw (see `Assets::pick`): `theme`, `layer`, `state` as in the header,
/// `side` the wanted pixel side. Returns 1 on success, 0 if nothing matches.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_pick(
    handle: *const Assets,
    entry: u32,
    theme: u32,
    layer: u32,
    state: u32,
    side: u32,
    out: *mut SilicaVariant,
) -> i32 {
    guard(0, || {
        // SAFETY: as in `silica_assets_entry_info`.
        let (Some(assets), Some(out)) = (unsafe { handle.as_ref() }, unsafe { out.as_mut() })
        else {
            return 0;
        };
        let (Some(theme), Some(layer), Some(state)) = (
            Theme::from_u32(theme),
            Layer::from_u32(layer),
            State::from_u32(state),
        ) else {
            return 0;
        };
        let query = Query {
            theme,
            layer,
            state,
            side,
        };
        let Some(v) = assets.pick(EntryId(entry), query) else {
            return 0;
        };
        fill_variant(&v, out);
        1
    })
}

/// Describes slot `index` of an entry. Returns 1 on success, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn silica_assets_slot(
    handle: *const Assets,
    entry: u32,
    index: u32,
    out: *mut SilicaSlot,
) -> i32 {
    guard(0, || {
        // SAFETY: as in `silica_assets_entry_info`.
        let (Some(assets), Some(out)) = (unsafe { handle.as_ref() }, unsafe { out.as_mut() })
        else {
            return 0;
        };
        let Some(s) = assets
            .entry(EntryId(entry))
            .and_then(|e| e.slot(index as usize))
        else {
            return 0;
        };
        out.kind = s.kind as u32;
        out.unit = u32::from(s.unit);
        out.min_size = u32::from(s.min_size);
        out.size = u32::from(s.size);
        out.weight = u32::from(s.weight);
        out.rect = s.rect;
        out.font = s.font.map_or(std::ptr::null(), |t| t.as_ptr().cast());
        out.font_len = s.font.map_or(0, str::len);
        out.color = s.color;
        out.format = s.format.map_or(std::ptr::null(), |t| t.as_ptr().cast());
        out.format_len = s.format.map_or(0, str::len);
        out.tracking = s.tracking;
        out.max_width = s.max_width;
        1
    })
}

#[cfg(all(test, feature = "compile"))]
#[path = "c_api_tests.rs"]
mod tests;
