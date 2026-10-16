//! Compile a small source tree, open the result and check everything the format promises.
#![cfg(feature = "compile")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod common;

use common::*;
use silica_assets::*;

fn open(fixture: &Fixture) -> (Assets, std::path::PathBuf) {
    let path = fixture.root().join("silica.assets");
    std::fs::write(&path, compile_fixture(fixture)).unwrap();
    // `Assets::open` maps the file; `from_bytes` is covered by the malformed-file tests.
    (Assets::open(&path).unwrap(), path)
}

#[test]
fn every_key_is_found_and_misses_are_rejected() {
    let fixture = fixture();
    let (assets, _) = open(&fixture);
    assert_eq!(assets.content_version(), "silica-assets test");

    let present: &[(KeySpace, &str, &str)] = &[
        (KeySpace::App, "mail", "mail"),
        (KeySpace::App, "clock", "clock"),
        (KeySpace::App, "calendar", "calendar"),
        (KeySpace::App, "dev.frascella.Mail", "mail"),
        (KeySpace::Type, "file-pdf", "file-pdf"),
        (KeySpace::Type, "file-generic", "file-generic"),
        (KeySpace::Mime, "application/pdf", "file-pdf"),
        (KeySpace::Mime, "text/markdown", "file-text"),
        (KeySpace::Mime, "image/*", "file-unknown"),
        (KeySpace::Mime, "application/json", "file-generic"),
        (KeySpace::Uti, "com.adobe.pdf", "file-pdf"),
        (KeySpace::Uti, "public.plain-text", "file-text"),
        (KeySpace::Ext, "pdf", "file-pdf"),
        (KeySpace::Ext, "md", "file-text"),
        (KeySpace::Role, "folder", "folder"),
        (KeySpace::Role, "trash.empty", "trash.empty"),
        (KeySpace::Role, "volume.external", "volume.external"),
        (KeySpace::Role, "dock.stack-close", "dock.stack-close"),
        (KeySpace::Role, "badge.alias", "badge.alias"),
        (KeySpace::Role, "badge.unreadable", "badge.unreadable"),
        (KeySpace::Symbol, "regular/key", "regular/key"),
        (KeySpace::Symbol, "light/key", "light/key"),
        (KeySpace::Symbol, "light/gear", "light/gear"),
        (KeySpace::Font, "Inter-Regular", "Inter-Regular"),
        (KeySpace::Font, "Mono", "Mono"),
        (KeySpace::Sound, "alert-drop", "alert-drop"),
        (KeySpace::Wallpaper, "silica", "silica"),
    ];
    for (space, key, name) in present {
        let id = assets
            .lookup(*space, key)
            .unwrap_or_else(|| panic!("{space:?} {key} not found"));
        let entry = assets.entry(id).unwrap();
        assert_eq!(entry.name(), *name, "{space:?} {key}");
        assert_ne!(entry.kind(), EntryKind::Alias, "aliases are resolved");
    }

    // The same key in the wrong key space, near misses and junk.
    let absent: &[(KeySpace, &str)] = &[
        (KeySpace::App, "file-pdf"),
        (KeySpace::App, "Mail"),
        (KeySpace::App, ""),
        (KeySpace::Type, "mail"),
        (KeySpace::Type, "file-pd"),
        (KeySpace::Mime, "application/PDF"),
        (KeySpace::Mime, "image/png"),
        (KeySpace::Uti, "application/pdf"),
        (KeySpace::Ext, ".pdf"),
        (KeySpace::Ext, "PDF"),
        (KeySpace::Role, "folder.generic"),
        (KeySpace::Role, "trash.full"),
        (KeySpace::Symbol, "bold/key"),
        (KeySpace::Symbol, "key"),
        (KeySpace::Font, "Inter"),
        (KeySpace::Sound, "alert-crystal"),
        (KeySpace::Wallpaper, "other"),
        (KeySpace::Wallpaper, "silica-light"),
    ];
    for (space, key) in absent {
        assert!(
            assets.lookup(*space, key).is_none(),
            "{space:?} {key} must be absent"
        );
    }
    // A generous sweep of keys that are not there: none may be accepted.
    for i in 0..2000 {
        for space in KeySpace::ALL {
            assert!(assets.lookup(*space, &format!("missing-{i}")).is_none());
        }
    }
}

#[test]
fn aliases_and_entries_describe_themselves() {
    let fixture = fixture();
    let (assets, _) = open(&fixture);
    let pdf = assets.lookup(KeySpace::Type, "file-pdf").unwrap();
    assert_eq!(assets.lookup(KeySpace::Mime, "application/pdf"), Some(pdf));
    assert_eq!(assets.lookup(KeySpace::Ext, "pdf"), Some(pdf));
    assert_eq!(assets.lookup(KeySpace::Uti, "com.adobe.pdf"), Some(pdf));
    assert_eq!(assets.entry(pdf).unwrap().kind(), EntryKind::File);
    // The raw alias entries exist in the file with `alias_of` set.
    let aliases = assets
        .entries()
        .filter(|e| e.kind() == EntryKind::Alias)
        .count();
    assert_eq!(aliases, 11);
    for alias in assets.entries().filter(|e| e.kind() == EntryKind::Alias) {
        let target = assets.entry(alias.alias_of().unwrap()).unwrap();
        assert_ne!(target.kind(), EntryKind::Alias);
    }
    let kinds = |key: &str, space| {
        assets
            .entry(assets.lookup(space, key).unwrap())
            .unwrap()
            .kind()
    };
    assert_eq!(kinds("folder", KeySpace::Role), EntryKind::Folder);
    assert_eq!(kinds("volume.external", KeySpace::Role), EntryKind::Volume);
    assert_eq!(kinds("badge.alias", KeySpace::Role), EntryKind::Badge);
    assert_eq!(kinds("regular/key", KeySpace::Symbol), EntryKind::Symbol);
    assert_eq!(kinds("Inter-Regular", KeySpace::Font), EntryKind::Font);
    assert_eq!(kinds("alert-drop", KeySpace::Sound), EntryKind::Sound);
    assert_eq!(kinds("silica", KeySpace::Wallpaper), EntryKind::Wallpaper);
    assert_eq!(kinds("mail", KeySpace::App), EntryKind::App);
}

#[test]
fn variant_pick_by_requested_side() {
    let fixture = fixture();
    let (assets, _) = open(&fixture);
    let mail = assets.lookup(KeySpace::App, "mail").unwrap();
    let sides = |wanted: u32| {
        assets
            .pick(mail, Query::composite(Theme::Default, wanted))
            .unwrap()
            .size_px
    };
    assert_eq!(sides(0), 16);
    assert_eq!(sides(1), 16);
    assert_eq!(sides(16), 16);
    assert_eq!(sides(17), 32);
    assert_eq!(sides(32), 32);
    assert_eq!(sides(33), 64);
    assert_eq!(sides(100), 128);
    assert_eq!(sides(128), 128);
    assert_eq!(sides(129), 256);
    assert_eq!(sides(300), 512);
    assert_eq!(sides(1024), 1024);
    assert_eq!(
        sides(5000),
        1024,
        "larger than any stored side: the largest"
    );
    assert_eq!(sides(u32::MAX), 1024);

    let v = assets
        .pick(mail, Query::composite(Theme::Default, 64))
        .unwrap();
    assert_eq!(v.encoding, Encoding::Rgba8Premul);
    let v = assets
        .pick(mail, Query::composite(Theme::Default, 256))
        .unwrap();
    assert_eq!(v.encoding, Encoding::Qoi);

    // Seven sides for each of the six themes.
    assert_eq!(assets.entry(mail).unwrap().variants().count(), 42);

    // Clock: dark and default exist; Clear Dark falls back to Dark, Tinted to Default.
    let clock = assets.lookup(KeySpace::App, "clock").unwrap();
    let theme_of = |theme, layer| {
        assets
            .pick(
                clock,
                Query {
                    theme,
                    layer,
                    state: State::Normal,
                    side: 64,
                },
            )
            .map(|v| v.theme)
    };
    assert_eq!(theme_of(Theme::Dark, Layer::Composite), Some(Theme::Dark));
    assert_eq!(
        theme_of(Theme::ClearDark, Layer::Composite),
        Some(Theme::Dark)
    );
    assert_eq!(
        theme_of(Theme::Tinted, Layer::Composite),
        Some(Theme::Default)
    );
    assert_eq!(
        theme_of(Theme::TintedDark, Layer::Composite),
        Some(Theme::Dark)
    );
    // The second hand has no dark file: Dark falls back to Default.
    assert_eq!(
        theme_of(Theme::Dark, Layer::HandSecond),
        Some(Theme::Default)
    );
    assert_eq!(theme_of(Theme::Dark, Layer::HandHour), Some(Theme::Dark));
    // A layer the icon does not have.
    assert_eq!(theme_of(Theme::Default, Layer::Background), None);
    // Mail has no hands at all.
    assert!(
        assets
            .pick(
                mail,
                Query {
                    theme: Theme::Default,
                    layer: Layer::Cap,
                    state: State::Normal,
                    side: 64
                }
            )
            .is_none()
    );

    // The full folder is a state, not a theme.
    let folder = assets.lookup(KeySpace::Role, "folder").unwrap();
    let full = Query {
        theme: Theme::Dark,
        layer: Layer::Composite,
        state: State::Full,
        side: 128,
    };
    let v = assets.pick(folder, full).unwrap();
    assert_eq!((v.state, v.theme), (State::Full, Theme::Dark));

    // A wallpaper: 64 wide and 128 wide; 100 asks for the larger one.
    let wallpaper = assets.lookup(KeySpace::Wallpaper, "silica").unwrap();
    let v = assets
        .pick(wallpaper, Query::composite(Theme::Default, 100))
        .unwrap();
    assert_eq!((v.encoding, v.width, v.height), (Encoding::Png, 128, 72));
    let v = assets
        .pick(wallpaper, Query::composite(Theme::Dark, 100))
        .unwrap();
    assert_eq!((v.theme, v.width, v.height), (Theme::Dark, 64, 36));

    // An id that is not an entry.
    assert!(
        assets
            .pick(EntryId(u32::MAX), Query::composite(Theme::Default, 64))
            .is_none()
    );
    assert!(assets.entry(EntryId(assets.entry_count() as u32)).is_none());
}

#[test]
fn pixel_bytes_are_what_the_sources_say() {
    let fixture = fixture();
    let (assets, _) = open(&fixture);
    let mail = assets.lookup(KeySpace::App, "mail").unwrap();
    let themes = [
        Theme::Default,
        Theme::Dark,
        Theme::Clear,
        Theme::ClearDark,
        Theme::Tinted,
        Theme::TintedDark,
    ];
    for (i, theme) in themes.into_iter().enumerate() {
        let colored = premultiply(rendered_color(i as u8));
        for side in [16u32, 32, 64, 128] {
            let v = assets.pick(mail, Query::composite(theme, side)).unwrap();
            assert_eq!((v.theme, u32::from(v.size_px)), (theme, side));
            assert_eq!(v.data.len(), (side * side * 4) as usize);
            let at = |x: u32, y: u32| &v.data[((y * side + x) * 4) as usize..][..4];
            assert_eq!(at(0, 0), colored, "{theme:?} {side}");
            assert_eq!(at(side / 2 - 1, side - 1), colored);
            assert_eq!(at(side / 2, 0), [0, 0, 0, 0]);
            assert_eq!(at(side - 1, side - 1), [0, 0, 0, 0]);
        }
        for side in [256u32, 512, 1024] {
            let v = assets.pick(mail, Query::composite(theme, side)).unwrap();
            assert_eq!(v.encoding, Encoding::Qoi);
            let (header, pixels) = qoi::decode_to_vec(v.data).unwrap();
            assert_eq!((header.width, header.height), (side, side));
            let mut expected = Vec::new();
            for _ in 0..side {
                for x in 0..side {
                    expected.extend_from_slice(&if x < side / 2 { colored } else { [0, 0, 0, 0] });
                }
            }
            assert_eq!(pixels, expected, "{theme:?} {side}");
        }
    }

    // Composed from SVG layers: the red square of the clock sits at (100..400) on the canvas,
    // so at 128 px it covers pixels 12..50 and the white background is around it.
    let clock = assets.lookup(KeySpace::App, "clock").unwrap();
    let v = assets
        .pick(clock, Query::composite(Theme::Default, 128))
        .unwrap();
    let at = |x: usize, y: usize| &v.data[(y * 128 + x) * 4..][..4];
    assert_eq!(at(30, 30), [255, 0, 0, 255]);
    assert_eq!(at(100, 100), [255, 255, 255, 255]);
    let v = assets
        .pick(clock, Query::composite(Theme::Dark, 128))
        .unwrap();
    let at = |x: usize, y: usize| &v.data[(y * 128 + x) * 4..][..4];
    assert_eq!(at(30, 30), [0, 255, 0, 255]);
    assert_eq!(at(100, 100), [34, 34, 34, 255]);
}

#[test]
fn blobs_slots_and_dynamic_layers() {
    let fixture = fixture();
    let (assets, _) = open(&fixture);

    let blob = |space, key: &str, encoding| {
        let id = assets.lookup(space, key).unwrap();
        let v = assets.entry(id).unwrap().variant(0).unwrap();
        assert_eq!(v.encoding, encoding);
        v.data.to_vec()
    };
    assert_eq!(
        blob(KeySpace::Font, "Inter-Regular", Encoding::Ttf),
        FONT_BYTES
    );
    assert_eq!(blob(KeySpace::Font, "Mono", Encoding::Otf), b"OTTO mono");
    assert_eq!(
        blob(KeySpace::Sound, "alert-drop", Encoding::Wav),
        SOUND_BYTES
    );
    let svg = blob(KeySpace::Symbol, "regular/key", Encoding::Svg);
    assert!(svg.starts_with(b"<svg"));
    let symbol = assets
        .entry(assets.lookup(KeySpace::Symbol, "regular/key").unwrap())
        .unwrap();
    assert_ne!(symbol.flags() & FLAG_TINTABLE, 0);

    // Blobs are 64-byte aligned inside the file, so they can be handed to the GPU.
    let base = {
        let font = assets
            .entry(assets.lookup(KeySpace::Font, "Inter-Regular").unwrap())
            .unwrap();
        font.variant(0).unwrap().data.as_ptr() as usize
    };
    assert_eq!(base % 64, 0);

    // Clock slots.
    let clock = assets
        .entry(assets.lookup(KeySpace::App, "clock").unwrap())
        .unwrap();
    assert_ne!(clock.flags() & FLAG_HAS_DYNAMIC, 0);
    let slots: Vec<_> = clock.slots().collect();
    assert_eq!(slots.len(), 4);
    assert_eq!(slots[0].kind, SlotKind::ClockHand);
    assert_eq!(slots[0].unit, Unit::Hour as u8);
    assert_eq!(slots[0].rect, [512.0, 512.0, 0.0, 0.0]);
    assert_eq!(slots[2].unit, Unit::Second as u8);
    assert_eq!(slots[2].min_size, 64);
    assert_eq!(slots[3].kind, SlotKind::ClockCap);
    // Each hand is stored as its own layer at every size.
    let hour = assets
        .pick(
            clock.id(),
            Query {
                theme: Theme::Default,
                layer: Layer::HandHour,
                state: State::Normal,
                side: 1024,
            },
        )
        .unwrap();
    let (header, pixels) = qoi::decode_to_vec(hour.data).unwrap();
    assert_eq!((header.width, header.height), (1024, 1024));
    // The hour hand covers x 500..524, y 200..512 in black.
    let px = |x: usize, y: usize| &pixels[(y * 1024 + x) * 4..][..4];
    assert_eq!(px(510, 300), [0, 0, 0, 255]);
    assert_eq!(px(300, 300), [0, 0, 0, 0]);

    // Calendar slots.
    let calendar = assets
        .entry(assets.lookup(KeySpace::App, "calendar").unwrap())
        .unwrap();
    let month = calendar.slot(0).unwrap();
    assert_eq!(month.kind, SlotKind::CalendarMonth);
    assert_eq!(month.rect, [100.0, 206.0, 824.0, 120.0]);
    assert_eq!(
        (month.font, month.size, month.weight),
        (Some("Inter"), 92, 900)
    );
    assert_eq!(month.color, [0xF2382EFF, 0xFF5248FF]);
    assert_eq!(month.format, Some("month:upper"));
    assert_eq!((month.tracking, month.max_width), (4.0, 560.0));
    let day = calendar.slot(1).unwrap();
    assert_eq!((day.kind, day.format), (SlotKind::CalendarDay, Some("day")));
    assert_eq!(day.color, [0xFFFFFFFF, 0xFFFFFFFF]);
    assert!(calendar.slot(2).is_none());

    // Generic file label and category colors.
    let generic = assets
        .entry(assets.lookup(KeySpace::Type, "file-generic").unwrap())
        .unwrap();
    let label = generic.slot(0).unwrap();
    assert_eq!(
        (label.kind, label.unit, label.format),
        (SlotKind::TypeLabel, 5, Some("upper"))
    );
    let categories: Vec<_> = generic
        .slots()
        .filter(|s| s.kind == SlotKind::TypeCategory)
        .collect();
    assert_eq!(categories.len(), 2);
    assert_eq!(categories[0].format, Some("data"));
    assert_eq!(categories[1].format, Some("document"));
    assert_eq!(categories[1].color, [0x2F7CF6FF, 0x5AA0FFFF]);

    // Badge placement.
    let alias = assets
        .entry(assets.lookup(KeySpace::Role, "badge.alias").unwrap())
        .unwrap();
    let slot = alias.slot(0).unwrap();
    assert_eq!(slot.kind, SlotKind::Badge);
    assert_eq!(slot.unit, Corner::BottomLeft as u8);
    assert_eq!(slot.rect[..2], [100.0, 924.0]);
    assert_eq!(slot.min_size, 32);
    assert_eq!(slot.weight, 1, "position in the badge order");
}

#[test]
fn compilation_is_deterministic() {
    let fixture = fixture();
    let first = compile_fixture(&fixture);
    let second = compile_fixture(&fixture);
    assert_eq!(first, second);

    // The same sources in another directory give the same bytes.
    let other = common::fixture();
    assert_eq!(first, compile_fixture(&other));

    // Changing a source changes the hash and the bytes.
    let before = Assets::from_bytes(first.clone()).unwrap();
    std::fs::write(
        fixture.options.fonts.as_ref().unwrap().join("Mono.otf"),
        b"OTTO other",
    )
    .unwrap();
    let changed = compile_fixture(&fixture);
    let after = Assets::from_bytes(changed.clone()).unwrap();
    assert_ne!(first, changed);
    assert_ne!(before.source_hash(), after.source_hash());
    assert_ne!(before.source_hash(), &[0u8; 32]);
}

#[test]
fn a_type_table_naming_a_missing_icon_is_an_error() {
    let fixture = fixture();
    let table = fixture.root().join("bad-types.toml");
    std::fs::write(
        &table,
        "[mime]\n\"application/x-nothing\" = \"file-nothing\"\n",
    )
    .unwrap();
    let mut options = fixture.options.clone();
    options.types = Some(table);
    let error = silica_assets::compile::compile(&options).err().unwrap();
    assert!(error.to_string().contains("file-nothing"), "{error}");
}

#[test]
fn the_shipped_type_table_parses_and_covers_what_mustache_knows() {
    let table: toml::Table = silica_assets::compile::DEFAULT_TYPES.parse().unwrap();
    let mime = table["mime"].as_table().unwrap();
    for needed in [
        "application/pdf",
        "text/plain",
        "text/markdown",
        "text/csv",
        "application/zip",
        "application/x-tar",
        "application/gzip",
        "application/json",
        "text/html",
        "image/*",
        "audio/*",
        "video/*",
        "font/*",
        "application/x-shellscript",
        "application/vnd.rar",
        "application/x-7z-compressed",
        "application/zstd",
        "application/x-bzip2",
        "application/x-xz",
    ] {
        assert!(mime.contains_key(needed), "{needed}");
    }
    assert!(
        table["uti"]
            .as_table()
            .unwrap()
            .contains_key("com.adobe.pdf")
    );
    assert!(table["ext"].as_table().unwrap().contains_key("pdf"));
}
