//! Smoke test of the C ABI, called the way `dart:ffi` calls it: through raw pointers and
//! plain structs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::mem::MaybeUninit;

use super::*;
use crate::compile::{Builder, SlotSpec, VariantSpec};

fn variant(theme: Theme, side: u16, data: Vec<u8>) -> VariantSpec {
    VariantSpec {
        theme,
        layer: Layer::Composite,
        encoding: Encoding::Rgba8Premul,
        state: State::Normal,
        size_px: side,
        width: side,
        height: side,
        data,
    }
}

fn file() -> Vec<u8> {
    let mut b = Builder::new("silica-assets test", [1; 32]);
    let mail = b.add_entry("mail", EntryKind::App, FLAG_HAS_DARK);
    b.register(KeySpace::App, mail).unwrap();
    b.add_alias(KeySpace::App, "dev.frascella.Mail", mail)
        .unwrap();
    b.add_variant(mail, variant(Theme::Default, 16, vec![1; 16 * 16 * 4]))
        .unwrap();
    b.add_variant(mail, variant(Theme::Default, 32, vec![2; 32 * 32 * 4]))
        .unwrap();
    b.add_variant(mail, variant(Theme::Dark, 32, vec![3; 32 * 32 * 4]))
        .unwrap();
    b.add_slot(
        mail,
        SlotSpec {
            kind: Some(SlotKind::CalendarMonth),
            rect: [100.0, 206.0, 824.0, 120.0],
            font: Some("Inter".to_owned()),
            format: Some("month:upper".to_owned()),
            size: 92,
            weight: 900,
            ..SlotSpec::default()
        },
    )
    .unwrap();
    let font = b.add_entry("Inter-Regular", EntryKind::Font, 0);
    b.register(KeySpace::Font, font).unwrap();
    let mut f = variant(Theme::Default, 0, b"font bytes".to_vec());
    f.encoding = Encoding::Ttf;
    b.add_variant(font, f).unwrap();
    b.finish().unwrap()
}

fn lookup(handle: *mut Assets, space: u32, key: &str) -> u32 {
    silica_assets_lookup(handle, space, key.as_ptr(), key.len())
}

fn text(ptr: *const c_char, len: usize) -> String {
    assert!(!ptr.is_null());
    // SAFETY: the library returns NUL-terminated strings of `len` bytes that live as long as
    // the handle, which the tests below keep open.
    let c = unsafe { CStr::from_ptr(ptr) };
    assert_eq!(c.to_bytes().len(), len);
    c.to_str().unwrap().to_owned()
}

#[test]
fn open_lookup_pick_blob_slots_close() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("silica.assets");
    std::fs::write(&path, file()).unwrap();
    let c_path = CString::new(path.to_str().unwrap()).unwrap();

    let handle = silica_assets_open(c_path.as_ptr());
    assert!(!handle.is_null());
    assert_eq!(silica_assets_format_version(), 1);
    let version = silica_assets_content_version(handle);
    assert_eq!(
        text(version, "silica-assets test".len()),
        "silica-assets test"
    );
    assert_eq!(silica_assets_entry_count(handle), 3);

    // The desktop id is an alias of Mail's icon: the lookup gives the real entry.
    let mail = lookup(handle, 0, "dev.frascella.Mail");
    assert_ne!(mail, SILICA_ASSETS_NONE);
    assert_eq!(mail, lookup(handle, 0, "mail"));
    let mut info = MaybeUninit::<SilicaEntryInfo>::zeroed();
    assert_eq!(silica_assets_entry_info(handle, mail, info.as_mut_ptr()), 1);
    // SAFETY: the call above succeeded and initialized every field.
    let info = unsafe { info.assume_init() };
    assert_eq!(text(info.name, info.name_len), "mail");
    assert_eq!(
        (
            info.kind,
            info.alias_of,
            info.variant_count,
            info.slot_count
        ),
        (0, SILICA_ASSETS_NONE, 3, 1)
    );

    // Pick: side 20 gives the 32 px variant.
    let mut v = MaybeUninit::<SilicaVariant>::zeroed();
    assert_eq!(
        silica_assets_pick(handle, mail, 0, 0, 0, 20, v.as_mut_ptr()),
        1
    );
    // SAFETY: initialized by the successful call.
    let v = unsafe { v.assume_init() };
    assert_eq!((v.size_px, v.encoding, v.len), (32, 0, 32 * 32 * 4));
    // SAFETY: `data` points to `len` bytes in the mapping, alive until close.
    let pixels = unsafe { std::slice::from_raw_parts(v.data, v.len) };
    assert!(pixels.iter().all(|&b| b == 2));
    let mut dark = MaybeUninit::<SilicaVariant>::zeroed();
    assert_eq!(
        silica_assets_pick(handle, mail, 1, 0, 0, 1, dark.as_mut_ptr()),
        1
    );
    // SAFETY: initialized by the successful call.
    assert_eq!(unsafe { dark.assume_init() }.theme, 1);

    // The same variants by index.
    let mut by_index = MaybeUninit::<SilicaVariant>::zeroed();
    for i in 0..3 {
        assert_eq!(
            silica_assets_variant(handle, mail, i, by_index.as_mut_ptr()),
            1
        );
    }
    assert_eq!(
        silica_assets_variant(handle, mail, 3, by_index.as_mut_ptr()),
        0
    );

    // A font blob.
    let font = lookup(handle, 6, "Inter-Regular");
    let mut v = MaybeUninit::<SilicaVariant>::zeroed();
    assert_eq!(
        silica_assets_pick(handle, font, 0, 0, 0, 0, v.as_mut_ptr()),
        1
    );
    // SAFETY: initialized by the successful call; the data lives in the mapping.
    let (encoding, bytes) = unsafe {
        let v = v.assume_init();
        (v.encoding, std::slice::from_raw_parts(v.data, v.len))
    };
    assert_eq!((encoding, bytes), (2, &b"font bytes"[..]));

    // A slot.
    let mut slot = MaybeUninit::<SilicaSlot>::zeroed();
    assert_eq!(silica_assets_slot(handle, mail, 0, slot.as_mut_ptr()), 1);
    // SAFETY: initialized by the successful call.
    let slot = unsafe { slot.assume_init() };
    assert_eq!((slot.kind, slot.size, slot.weight), (2, 92, 900));
    assert_eq!(text(slot.font, slot.font_len), "Inter");
    assert_eq!(text(slot.format, slot.format_len), "month:upper");
    assert_eq!(slot.rect, [100.0, 206.0, 824.0, 120.0]);
    let mut none = MaybeUninit::<SilicaSlot>::zeroed();
    assert_eq!(silica_assets_slot(handle, mail, 1, none.as_mut_ptr()), 0);

    // Misses and bad arguments never crash.
    assert_eq!(lookup(handle, 0, "nope"), SILICA_ASSETS_NONE);
    assert_eq!(lookup(handle, 99, "mail"), SILICA_ASSETS_NONE);
    assert_eq!(
        silica_assets_lookup(handle, 0, std::ptr::null(), 4),
        SILICA_ASSETS_NONE
    );
    assert_eq!(
        silica_assets_lookup(handle, 0, [0xFFu8, 0xFE].as_ptr(), 2),
        SILICA_ASSETS_NONE
    );
    assert_eq!(
        silica_assets_lookup(std::ptr::null(), 0, b"mail".as_ptr(), 4),
        SILICA_ASSETS_NONE
    );
    let mut v = MaybeUninit::<SilicaVariant>::zeroed();
    assert_eq!(
        silica_assets_pick(handle, mail, 77, 0, 0, 64, v.as_mut_ptr()),
        0
    );
    assert_eq!(
        silica_assets_pick(handle, u32::MAX, 0, 0, 0, 64, v.as_mut_ptr()),
        0
    );
    assert_eq!(
        silica_assets_pick(handle, mail, 0, 0, 0, 64, std::ptr::null_mut()),
        0
    );
    assert_eq!(
        silica_assets_pick(std::ptr::null(), mail, 0, 0, 0, 64, v.as_mut_ptr()),
        0
    );
    assert_eq!(
        silica_assets_entry_info(handle, u32::MAX, std::ptr::null_mut()),
        0
    );
    assert!(silica_assets_content_version(std::ptr::null()).is_null());
    assert_eq!(silica_assets_entry_count(std::ptr::null()), 0);

    silica_assets_close(handle);
    silica_assets_close(std::ptr::null_mut());
}

#[test]
fn open_failures_report_a_reason() {
    assert!(silica_assets_open(std::ptr::null()).is_null());
    let reason = || {
        // SAFETY: the library returns a NUL-terminated string valid until its next call.
        unsafe { CStr::from_ptr(silica_assets_last_error()) }
            .to_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(reason(), "null path");
    let missing = CString::new("/nonexistent/silica.assets").unwrap();
    assert!(silica_assets_open(missing.as_ptr()).is_null());
    assert!(reason().contains("cannot read"), "{}", reason());

    let dir = tempfile::tempdir().unwrap();
    let junk = dir.path().join("junk");
    std::fs::write(&junk, vec![7u8; 500]).unwrap();
    let c_junk = CString::new(junk.to_str().unwrap()).unwrap();
    assert!(silica_assets_open(c_junk.as_ptr()).is_null());
    assert!(reason().contains("magic"), "{}", reason());
}
