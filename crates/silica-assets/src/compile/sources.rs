//! Reading the sources and feeding the [`Builder`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use xxhash_rust::xxh3::Xxh3;

use crate::format::*;

use super::builder::{Builder, SlotSpec, VariantSpec};
use super::raster::{self, Image};
use super::{CompileError, DEFAULT_TYPES, Options, Report};

const THEMES: [(&str, Theme); 6] = [
    ("default", Theme::Default),
    ("dark", Theme::Dark),
    ("clear", Theme::Clear),
    ("clear-dark", Theme::ClearDark),
    ("tinted", Theme::Tinted),
    ("tinted-dark", Theme::TintedDark),
];

/// Reads files and hashes what it reads, so `source_hash` covers exactly the inputs.
struct Reader {
    a: Xxh3,
    b: Xxh3,
}

impl Reader {
    fn new() -> Reader {
        Reader {
            a: Xxh3::with_seed(0),
            b: Xxh3::with_seed(0x0511_1CA5_5E7A_55E7),
        }
    }

    fn read(&mut self, root: &Path, relative: &str) -> Result<Vec<u8>, CompileError> {
        let path = root.join(relative);
        let bytes = fs::read(&path)
            .map_err(|e| CompileError::new(format!("cannot read {}: {e}", path.display())))?;
        self.note(relative, &bytes);
        Ok(bytes)
    }

    fn note(&mut self, name: &str, bytes: &[u8]) {
        for h in [&mut self.a, &mut self.b] {
            h.update(&(name.len() as u64).to_le_bytes());
            h.update(name.as_bytes());
            h.update(&(bytes.len() as u64).to_le_bytes());
            h.update(bytes);
        }
    }

    fn digest(&self) -> [u8; 32] {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&self.a.digest128().to_le_bytes());
        bytes.extend_from_slice(&self.b.digest128().to_le_bytes());
        bytes.try_into().unwrap_or([0; 32])
    }
}

fn io_error(path: &Path, e: std::io::Error) -> CompileError {
    CompileError::new(format!("cannot list {}: {e}", path.display()))
}

/// Sorted names of the entries of a directory.
fn list(dir: &Path) -> Result<Vec<String>, CompileError> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| io_error(dir, e))? {
        let entry = entry.map_err(|e| io_error(dir, e))?;
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

fn parse_json(bytes: &[u8], what: &str) -> Result<Value, CompileError> {
    serde_json::from_slice(bytes).map_err(|e| CompileError::new(format!("{what}: {e}")))
}

fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn num(value: Option<&Value>, default: f64) -> f64 {
    value.and_then(Value::as_f64).unwrap_or(default)
}

fn rect_of(value: Option<&Value>) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    if let Some(array) = value.and_then(Value::as_array) {
        for (slot, v) in out.iter_mut().zip(array) {
            *slot = v.as_f64().unwrap_or(0.0) as f32;
        }
    }
    out
}

/// `#rrggbb` or `#rrggbbaa` to `0xRRGGBBAA`.
fn color_of(text: Option<&str>) -> u32 {
    let Some(hex) = text.and_then(|t| t.strip_prefix('#')) else {
        return 0;
    };
    match (hex.len(), u32::from_str_radix(hex, 16)) {
        (6, Ok(v)) => (v << 8) | 0xFF,
        (8, Ok(v)) => v,
        _ => 0,
    }
}

struct Ctx<'a> {
    builder: &'a mut Builder,
    reader: &'a mut Reader,
    report: &'a mut Report,
}

impl Ctx<'_> {
    fn add_ladder(
        &mut self,
        entry: u32,
        theme: Theme,
        layer: Layer,
        state: State,
        image: &Image,
    ) -> Result<(), CompileError> {
        for r in raster::ladder(image)? {
            self.builder.add_variant(
                entry,
                VariantSpec {
                    theme,
                    layer,
                    encoding: r.encoding,
                    state,
                    size_px: r.side,
                    width: r.side,
                    height: r.side,
                    data: r.bytes,
                },
            )?;
        }
        Ok(())
    }

    fn svg(&mut self, dir: &Path, relative: &str) -> Result<Image, CompileError> {
        let bytes = self.reader.read(dir, relative)?;
        raster::image_from_svg(&bytes)
            .map_err(|e| CompileError::new(format!("{}/{relative}: {e}", dir.display())))
    }
}

pub(super) fn compile(options: &Options) -> Result<(Vec<u8>, Report), CompileError> {
    let mut reader = Reader::new();
    let mut report = Report::default();
    let version = options
        .content_version
        .clone()
        .unwrap_or_else(|| format!("silica-assets {}", env!("CARGO_PKG_VERSION")));

    let types_text = match &options.types {
        Some(path) => {
            let text = fs::read_to_string(path)
                .map_err(|e| CompileError::new(format!("cannot read {}: {e}", path.display())))?;
            reader.note("types.toml", text.as_bytes());
            text
        }
        None => {
            reader.note("types.toml", DEFAULT_TYPES.as_bytes());
            DEFAULT_TYPES.to_owned()
        }
    };
    let types: toml::Table = types_text
        .parse()
        .map_err(|e| CompileError::new(format!("types.toml: {e}")))?;

    // The builder needs the hash up front, but the hash is known only after reading: build
    // with a placeholder and patch the header at the end.
    let mut builder = Builder::new(&version, [0; 32]);
    let mut ctx = Ctx {
        builder: &mut builder,
        reader: &mut reader,
        report: &mut report,
    };

    let mut type_ids: BTreeMap<String, u32> = BTreeMap::new();
    let mut app_ids: BTreeMap<String, u32> = BTreeMap::new();
    let shell_apps = shell_apps(&types)?;
    apps(
        &mut ctx,
        &options.icons.join("apps"),
        shell_apps.as_deref(),
        &mut app_ids,
    )?;
    files(&mut ctx, &options.icons.join("files"), &mut type_ids)?;
    badges(&mut ctx, &options.icons.join("badges"))?;
    symbols(&mut ctx, &options.icons.join("symbols"))?;
    if let Some(fonts) = &options.fonts {
        fonts_from(&mut ctx, fonts)?;
    }
    let sounds = options.sounds.clone().or_else(|| {
        options
            .brand
            .as_ref()
            .map(|b| b.join("sounds"))
            .filter(|p| p.is_dir())
    });
    if let Some(sounds) = sounds {
        sounds_from(&mut ctx, &sounds)?;
    }
    if let Some(dir) = &options.wallpapers {
        wallpaper_sets(&mut ctx, dir)?;
    }
    aliases(&mut ctx, &types, &type_ids, &app_ids)?;

    let hash = reader.digest();
    let mut bytes = builder.finish()?;
    if let Some(dst) = bytes.get_mut(48..80) {
        dst.copy_from_slice(&hash);
    }
    Ok((bytes, report))
}

// ----------------------------------------------------------------------------------- apps

/// `shell_apps` of types.toml: the only app icons compiled. `None` (no key): every app icon.
fn shell_apps(types: &toml::Table) -> Result<Option<Vec<String>>, CompileError> {
    let Some(value) = types.get("shell_apps") else {
        return Ok(None);
    };
    let bad = || CompileError::new("types.toml shell_apps: not an array of strings");
    value
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or_else(bad))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn apps(
    ctx: &mut Ctx<'_>,
    dir: &Path,
    only: Option<&[String]>,
    ids: &mut BTreeMap<String, u32>,
) -> Result<(), CompileError> {
    for name in list(dir)? {
        let app_dir = dir.join(&name);
        if !app_dir.join("icon.json").is_file() {
            continue;
        }
        // The other Silica apps carry their own icon (docs/assets-format.md "App icons").
        if only.is_some_and(|only| !only.contains(&name)) {
            continue;
        }
        let manifest = parse_json(
            &ctx.reader.read(&app_dir, "icon.json")?,
            &format!("apps/{name}/icon.json"),
        )?;
        let id = str_of(&manifest, "id").unwrap_or(&name).to_owned();
        let dynamic = manifest.get("dynamic").filter(|d| !d.is_null());
        let mut flags = 0;
        if dynamic.is_some() {
            flags |= FLAG_HAS_DYNAMIC;
        }
        let entry = ctx.builder.add_entry(&id, EntryKind::App, 0);
        ctx.builder.register(KeySpace::App, entry)?;
        ids.insert(id.clone(), entry);

        let rendered = manifest.get("rendered").and_then(Value::as_object);
        let mut has_dark = false;
        if let Some(rendered) = rendered.filter(|r| !r.is_empty()) {
            for (key, theme) in THEMES {
                let Some(file) = rendered.get(key).and_then(Value::as_str) else {
                    continue;
                };
                let bytes = ctx.reader.read(&app_dir, file)?;
                let image = raster::image_from_png(&bytes)
                    .map_err(|e| CompileError::new(format!("apps/{name}/{file}: {e}")))?;
                has_dark |= matches!(theme, Theme::Dark | Theme::ClearDark | Theme::TintedDark);
                ctx.add_ladder(entry, theme, Layer::Composite, State::Normal, &image)?;
            }
        } else {
            has_dark = composite_from_layers(ctx, &app_dir, &manifest, entry)?;
        }
        if has_dark {
            flags |= FLAG_HAS_DARK;
        }
        if let Some(dynamic) = dynamic {
            dynamic_slots(ctx, &app_dir, dynamic, entry)?;
        }
        set_flags(ctx, entry, flags);
        ctx.report.apps += 1;
    }
    Ok(())
}

fn set_flags(ctx: &mut Ctx<'_>, entry: u32, flags: u16) {
    ctx.builder.set_flags(entry, flags);
}

/// An app without rendered PNGs: the Default and Dark themes composed from the SVG layers.
fn composite_from_layers(
    ctx: &mut Ctx<'_>,
    dir: &Path,
    manifest: &Value,
    entry: u32,
) -> Result<bool, CompileError> {
    let layers: Vec<(String, String)> = manifest
        .get("layers")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|l| {
                    Some((str_of(l, "role")?.to_owned(), str_of(l, "file")?.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    let dark = manifest
        .get("variants")
        .and_then(|v| v.get("dark"))
        .and_then(Value::as_object);
    let mut has_dark = false;
    for (theme, use_dark) in [(Theme::Default, false), (Theme::Dark, true)] {
        if use_dark && dark.is_none() {
            continue;
        }
        let mut canvas: Option<Image> = None;
        for (role, file) in &layers {
            let file = match (
                use_dark,
                dark.and_then(|d| d.get(role)).and_then(Value::as_str),
            ) {
                (true, Some(replacement)) => replacement,
                _ => file,
            };
            let image = ctx.svg(dir, file)?;
            match canvas.as_mut() {
                None => canvas = Some(image),
                Some(bottom) => raster::over(bottom, &image),
            }
        }
        if let Some(canvas) = canvas {
            has_dark |= use_dark;
            ctx.add_ladder(entry, theme, Layer::Composite, State::Normal, &canvas)?;
        }
    }
    Ok(has_dark)
}

fn dark_name(file: &str) -> String {
    match file.strip_suffix(".svg") {
        Some(stem) => format!("{stem}-dark.svg"),
        None => format!("{file}-dark"),
    }
}

fn dynamic_slots(
    ctx: &mut Ctx<'_>,
    dir: &Path,
    dynamic: &Value,
    entry: u32,
) -> Result<(), CompileError> {
    match str_of(dynamic, "type") {
        Some("clock") => {
            let pivot = rect_of(dynamic.get("pivot"));
            let pivot = [pivot[0], pivot[1], 0.0, 0.0];
            let hands = dynamic.get("hands").and_then(Value::as_array);
            for hand in hands.into_iter().flatten() {
                let (unit, layer) = match str_of(hand, "unit") {
                    Some("hour") => (Unit::Hour, Layer::HandHour),
                    Some("minute") => (Unit::Minute, Layer::HandMinute),
                    Some("second") => (Unit::Second, Layer::HandSecond),
                    other => {
                        return Err(CompileError::new(format!("clock hand with unit {other:?}")));
                    }
                };
                let Some(file) = str_of(hand, "file") else {
                    continue;
                };
                layer_variants(ctx, dir, file, layer, entry)?;
                ctx.builder.add_slot(
                    entry,
                    SlotSpec {
                        kind: Some(SlotKind::ClockHand),
                        unit: unit as u8,
                        min_size: num(hand.get("min_size"), 0.0) as u16,
                        rect: pivot,
                        ..SlotSpec::default()
                    },
                )?;
            }
            if let Some(cap) = str_of(dynamic, "cap") {
                layer_variants(ctx, dir, cap, Layer::Cap, entry)?;
                ctx.builder.add_slot(
                    entry,
                    SlotSpec {
                        kind: Some(SlotKind::ClockCap),
                        rect: pivot,
                        ..SlotSpec::default()
                    },
                )?;
            }
        }
        Some("calendar") => {
            for (key, kind) in [
                ("month", SlotKind::CalendarMonth),
                ("day", SlotKind::CalendarDay),
            ] {
                let Some(text) = dynamic.get(key) else {
                    continue;
                };
                let color = color_of(str_of(text, "color"));
                let dark = str_of(text, "dark_color").map_or(color, |c| color_of(Some(c)));
                let format = match str_of(text, "case") {
                    Some(case) => format!("{key}:{case}"),
                    None => key.to_owned(),
                };
                ctx.builder.add_slot(
                    entry,
                    SlotSpec {
                        kind: Some(kind),
                        rect: rect_of(text.get("box")),
                        font: str_of(text, "font").map(str::to_owned),
                        size: num(text.get("size"), 0.0) as u16,
                        weight: num(text.get("weight"), 400.0) as u16,
                        color: [color, dark],
                        format: Some(format),
                        tracking: num(text.get("tracking"), 0.0) as f32,
                        max_width: num(text.get("max_width"), 0.0) as f32,
                        ..SlotSpec::default()
                    },
                )?;
            }
        }
        Some("file-label") => {
            if let Some(label) = dynamic.get("label") {
                ctx.builder.add_slot(
                    entry,
                    SlotSpec {
                        kind: Some(SlotKind::TypeLabel),
                        unit: num(label.get("max_chars"), 0.0) as u8,
                        rect: rect_of(label.get("box")),
                        font: str_of(label, "font").map(str::to_owned),
                        size: num(label.get("size"), 0.0) as u16,
                        weight: num(label.get("weight"), 400.0) as u16,
                        format: str_of(label, "case").map(str::to_owned),
                        tracking: num(label.get("tracking"), 0.0) as f32,
                        max_width: num(label.get("max_width"), 0.0) as f32,
                        ..SlotSpec::default()
                    },
                )?;
            }
            let colors = dynamic.get("colors").and_then(Value::as_object);
            let mut sorted: Vec<_> = colors.into_iter().flatten().collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));
            for (category, value) in sorted {
                let color = color_of(str_of(value, "color"));
                let dark = str_of(value, "dark_color").map_or(color, |c| color_of(Some(c)));
                ctx.builder.add_slot(
                    entry,
                    SlotSpec {
                        kind: Some(SlotKind::TypeCategory),
                        color: [color, dark],
                        format: Some(category.clone()),
                        ..SlotSpec::default()
                    },
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// A hand or cap, drawn for Default and (when a `-dark` file exists) Dark.
fn layer_variants(
    ctx: &mut Ctx<'_>,
    dir: &Path,
    file: &str,
    layer: Layer,
    entry: u32,
) -> Result<(), CompileError> {
    let image = ctx.svg(dir, file)?;
    ctx.add_ladder(entry, Theme::Default, layer, State::Normal, &image)?;
    let dark = dark_name(file);
    if dir.join(&dark).is_file() {
        let image = ctx.svg(dir, &dark)?;
        ctx.add_ladder(entry, Theme::Dark, layer, State::Normal, &image)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------- files

/// The key space and entry name of a `files/` icon.
fn file_role(id: &str) -> (KeySpace, String, EntryKind) {
    if id.starts_with("file-") {
        return (KeySpace::Type, id.to_owned(), EntryKind::File);
    }
    let kind = if id.starts_with("folder") {
        EntryKind::Folder
    } else if id.starts_with("volume") {
        EntryKind::Volume
    } else {
        EntryKind::File
    };
    let name = match id {
        "folder" => "folder".to_owned(),
        "stack-close" => "dock.stack-close".to_owned(),
        other => match other.split_once('-') {
            Some((head, tail)) => format!("{head}.{tail}"),
            None => other.to_owned(),
        },
    };
    (KeySpace::Role, name, kind)
}

fn files(
    ctx: &mut Ctx<'_>,
    dir: &Path,
    type_ids: &mut BTreeMap<String, u32>,
) -> Result<(), CompileError> {
    for name in list(dir)? {
        let icon_dir = dir.join(&name);
        if !icon_dir.join("icon.json").is_file() {
            continue;
        }
        let manifest = parse_json(
            &ctx.reader.read(&icon_dir, "icon.json")?,
            &format!("files/{name}/icon.json"),
        )?;
        let id = str_of(&manifest, "id").unwrap_or(&name).to_owned();
        let (space, entry_name, kind) = file_role(&id);
        let dynamic = manifest.get("dynamic").filter(|d| !d.is_null());
        let entry = ctx.builder.add_entry(&entry_name, kind, 0);
        ctx.builder.register(space, entry)?;
        if space == KeySpace::Type {
            type_ids.insert(entry_name.clone(), entry);
        }

        let artwork = |layers: Option<&Value>| -> Option<String> {
            layers?.as_array()?.iter().find_map(|l| {
                (str_of(l, "role") == Some("artwork"))
                    .then(|| str_of(l, "file").map(str::to_owned))?
            })
        };
        let mut sources = Vec::new();
        if let Some(file) = artwork(manifest.get("layers")) {
            sources.push((file, Theme::Default, State::Normal));
        }
        let variants = manifest.get("variants");
        for (key, theme, state) in [
            ("dark", Theme::Dark, State::Normal),
            ("full", Theme::Default, State::Full),
            ("full-dark", Theme::Dark, State::Full),
        ] {
            if let Some(file) = variants
                .and_then(|v| v.get(key))
                .and_then(|v| str_of(v, "artwork"))
            {
                sources.push((file.to_owned(), theme, state));
            }
        }
        let mut flags = 0;
        for (file, theme, state) in sources {
            let image = ctx.svg(&icon_dir, &file)?;
            if theme == Theme::Dark {
                flags |= FLAG_HAS_DARK;
            }
            ctx.add_ladder(entry, theme, Layer::Composite, state, &image)?;
        }
        if let Some(dynamic) = dynamic {
            flags |= FLAG_HAS_DYNAMIC;
            dynamic_slots(ctx, &icon_dir, dynamic, entry)?;
        }
        set_flags(ctx, entry, flags);
        ctx.report.files += 1;
    }
    Ok(())
}

// --------------------------------------------------------------------------------- badges

fn badges(ctx: &mut Ctx<'_>, dir: &Path) -> Result<(), CompileError> {
    if !dir.join("badges.json").is_file() {
        return Ok(());
    }
    let manifest = parse_json(&ctx.reader.read(dir, "badges.json")?, "badges.json")?;
    let corners = manifest.get("corners");
    let sizes = manifest.get("sizes").and_then(Value::as_array);
    let fraction_at = |small: bool| -> f32 {
        sizes
            .into_iter()
            .flatten()
            .find(|s| s.get("single").and_then(Value::as_bool).unwrap_or(false) == small)
            .map_or(0.0, |s| num(s.get("fraction"), 0.0) as f32)
    };
    let min_size = sizes
        .into_iter()
        .flatten()
        .map(|s| num(s.get("min_size"), 0.0) as u16)
        .max()
        .unwrap_or(0);
    let order: Vec<&str> = manifest
        .get("order")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let Some(badges) = manifest.get("badges").and_then(Value::as_object) else {
        return Ok(());
    };
    let mut names: Vec<&String> = badges.keys().collect();
    names.sort();
    for name in names {
        let Some(badge) = badges.get(name) else {
            continue;
        };
        let entry = ctx
            .builder
            .add_entry(&format!("badge.{name}"), EntryKind::Badge, 0);
        ctx.builder.register(KeySpace::Role, entry)?;
        let mut flags = 0;
        if let Some(file) = str_of(badge, "file") {
            let image = ctx.svg(dir, file)?;
            ctx.add_ladder(
                entry,
                Theme::Default,
                Layer::Composite,
                State::Normal,
                &image,
            )?;
        }
        if let Some(file) = str_of(badge, "dark") {
            flags |= FLAG_HAS_DARK;
            let image = ctx.svg(dir, file)?;
            ctx.add_ladder(entry, Theme::Dark, Layer::Composite, State::Normal, &image)?;
        }
        let corner_name = str_of(badge, "corner").unwrap_or("bottom-right");
        let corner = match corner_name {
            "top-left" => Corner::TopLeft,
            "top-right" => Corner::TopRight,
            "bottom-left" => Corner::BottomLeft,
            _ => Corner::BottomRight,
        };
        let point = rect_of(corners.and_then(|c| c.get(corner_name)));
        let position = order
            .iter()
            .position(|o| *o == name.as_str())
            .unwrap_or(order.len());
        ctx.builder.add_slot(
            entry,
            SlotSpec {
                kind: Some(SlotKind::Badge),
                unit: corner as u8,
                min_size,
                rect: [point[0], point[1], fraction_at(false), fraction_at(true)],
                size: num(badge.get("priority"), 0.0) as u16,
                weight: position as u16,
                ..SlotSpec::default()
            },
        )?;
        set_flags(ctx, entry, flags | FLAG_HAS_DYNAMIC);
        ctx.report.badges += 1;
    }
    Ok(())
}

// -------------------------------------------------------------------------------- symbols

fn symbols(ctx: &mut Ctx<'_>, dir: &Path) -> Result<(), CompileError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for weight in list(dir)? {
        let weight_dir = dir.join(&weight);
        if !weight_dir.is_dir() {
            continue;
        }
        for file in list(&weight_dir)? {
            let Some(name) = file.strip_suffix(".svg") else {
                continue;
            };
            let bytes = ctx.reader.read(&weight_dir, &file)?;
            let entry = ctx.builder.add_entry(
                &format!("{weight}/{name}"),
                EntryKind::Symbol,
                FLAG_TINTABLE,
            );
            ctx.builder.register(KeySpace::Symbol, entry)?;
            ctx.builder.add_variant(
                entry,
                VariantSpec {
                    theme: Theme::Default,
                    layer: Layer::Composite,
                    encoding: Encoding::Svg,
                    state: State::Normal,
                    size_px: 0,
                    width: 0,
                    height: 0,
                    data: bytes,
                },
            )?;
            ctx.report.symbols += 1;
        }
    }
    Ok(())
}

// ------------------------------------------------------------- fonts, sounds, wallpapers

fn blob_entries(
    ctx: &mut Ctx<'_>,
    dir: &Path,
    kind: EntryKind,
    space: KeySpace,
    encode: impl Fn(&str) -> Option<Encoding>,
) -> Result<usize, CompileError> {
    let mut count = 0;
    for file in list(dir)? {
        let path = PathBuf::from(&file);
        let (Some(stem), Some(ext)) = (
            path.file_stem().and_then(|s| s.to_str()),
            path.extension().and_then(|s| s.to_str()),
        ) else {
            continue;
        };
        let Some(encoding) = encode(&ext.to_ascii_lowercase()) else {
            continue;
        };
        let bytes = ctx.reader.read(dir, &file)?;
        let entry = ctx.builder.add_entry(stem, kind, 0);
        ctx.builder.register(space, entry)?;
        ctx.builder.add_variant(
            entry,
            VariantSpec {
                theme: Theme::Default,
                layer: Layer::Composite,
                encoding,
                state: State::Normal,
                size_px: 0,
                width: 0,
                height: 0,
                data: bytes,
            },
        )?;
        count += 1;
    }
    Ok(count)
}

fn fonts_from(ctx: &mut Ctx<'_>, dir: &Path) -> Result<(), CompileError> {
    let n = blob_entries(ctx, dir, EntryKind::Font, KeySpace::Font, |ext| match ext {
        "ttf" => Some(Encoding::Ttf),
        "otf" => Some(Encoding::Otf),
        _ => None,
    })?;
    ctx.report.fonts = n;
    Ok(())
}

fn sounds_from(ctx: &mut Ctx<'_>, dir: &Path) -> Result<(), CompileError> {
    let n = blob_entries(
        ctx,
        dir,
        EntryKind::Sound,
        KeySpace::Sound,
        |ext| match ext {
            "wav" => Some(Encoding::Wav),
            "ogg" | "oga" => Some(Encoding::Ogg),
            "flac" => Some(Encoding::Flac),
            _ => None,
        },
    )?;
    ctx.report.sounds = n;
    Ok(())
}

/// The size of a PNG (IHDR) or JPEG (the first SOF marker) file.
fn image_size(bytes: &[u8], file: &str) -> Result<(u32, u32), CompileError> {
    let not_image = || CompileError::new(format!("{file}: not a PNG or JPEG"));
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let width = bytes
            .get(16..20)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_be_bytes);
        let height = bytes
            .get(20..24)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_be_bytes);
        return width.zip(height).ok_or_else(not_image);
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        let be16 = |at: usize| -> Option<usize> {
            let pair = bytes.get(at..at + 2)?;
            Some(usize::from(u16::from_be_bytes(pair.try_into().ok()?)))
        };
        let mut at = 2;
        while bytes.get(at) == Some(&0xff) {
            let marker = *bytes.get(at + 1).ok_or_else(not_image)?;
            let length = be16(at + 2).ok_or_else(not_image)?;
            // SOF0..SOF15, but not DHT (c4), JPG (c8) and DAC (cc).
            if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                let height = be16(at + 5).ok_or_else(not_image)?;
                let width = be16(at + 7).ok_or_else(not_image)?;
                return Ok((
                    u32::try_from(width).map_err(|_| not_image())?,
                    u32::try_from(height).map_err(|_| not_image())?,
                ));
            }
            at = at.checked_add(2 + length).ok_or_else(not_image)?;
        }
    }
    Err(not_image())
}

/// The wallpaper sets of a silica-wallpapers checkout: every folder with a `wallpapers.json`
/// (`{"wallpapers": [{"id", "title", "variants": {"light": {"file"}, "dark": {"file"}}}]}`).
/// One entry per id, a light and a dark PNG as they are, the preview
/// `previews/<id>-<light|dark>.jpg` (when present) as the small variant of each theme, and the
/// title as a `Title` slot.
fn wallpaper_sets(ctx: &mut Ctx<'_>, dir: &Path) -> Result<(), CompileError> {
    for set in list(dir)? {
        let folder = dir.join(&set);
        if !folder.join("wallpapers.json").is_file() {
            continue;
        }
        let manifest = ctx.reader.read(&folder, "wallpapers.json")?;
        let json = parse_json(&manifest, &format!("{set}/wallpapers.json"))?;
        let list = json
            .get("wallpapers")
            .and_then(Value::as_array)
            .ok_or_else(|| CompileError::new(format!("{set}/wallpapers.json: no wallpapers")))?;
        for item in list {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| CompileError::new(format!("{set}/wallpapers.json: no id")))?;
            let title = item.get("title").and_then(Value::as_str).unwrap_or(id);
            let entry = ctx
                .builder
                .add_entry(id, EntryKind::Wallpaper, FLAG_HAS_DARK);
            ctx.builder.register(KeySpace::Wallpaper, entry)?;
            ctx.builder.add_slot(
                entry,
                SlotSpec {
                    kind: Some(SlotKind::Title),
                    format: Some(title.to_owned()),
                    ..SlotSpec::default()
                },
            )?;
            for (name, theme) in [("light", Theme::Default), ("dark", Theme::Dark)] {
                let Some(file) = item
                    .pointer(&format!("/variants/{name}/file"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let add = |ctx: &mut Ctx<'_>,
                           relative: String,
                           encoding: Encoding|
                 -> Result<(), CompileError> {
                    let data = ctx.reader.read(&folder, &relative)?;
                    let (width, height) = image_size(&data, &relative)?;
                    let (Ok(w), Ok(h)) = (u16::try_from(width), u16::try_from(height)) else {
                        return Err(CompileError::new(format!(
                            "wallpaper {id}: {width}x{height} is too large"
                        )));
                    };
                    ctx.builder.add_variant(
                        entry,
                        VariantSpec {
                            theme,
                            layer: Layer::Composite,
                            encoding,
                            state: State::Normal,
                            size_px: w,
                            width: w,
                            height: h,
                            data,
                        },
                    )
                };
                add(ctx, file.to_owned(), Encoding::Png)?;
                let preview = format!("previews/{id}-{name}.jpg");
                if folder.join(&preview).is_file() {
                    add(ctx, preview, Encoding::Jpeg)?;
                }
            }
            ctx.report.wallpapers += 1;
        }
    }
    Ok(())
}

// -------------------------------------------------------------------------------- aliases

fn aliases(
    ctx: &mut Ctx<'_>,
    types: &toml::Table,
    type_ids: &BTreeMap<String, u32>,
    app_ids: &BTreeMap<String, u32>,
) -> Result<(), CompileError> {
    for (table, space, targets) in [
        ("app", KeySpace::App, app_ids),
        ("mime", KeySpace::Mime, type_ids),
        ("uti", KeySpace::Uti, type_ids),
        ("ext", KeySpace::Ext, type_ids),
    ] {
        let Some(map) = types.get(table).and_then(toml::Value::as_table) else {
            continue;
        };
        for (key, value) in map {
            let target_name = value.as_str().ok_or_else(|| {
                CompileError::new(format!("types.toml [{table}] {key}: not a string"))
            })?;
            let target = targets.get(target_name).ok_or_else(|| {
                CompileError::new(format!(
                    "types.toml [{table}] {key}: no icon named {target_name:?} in the sources"
                ))
            })?;
            ctx.builder.add_alias(space, key, *target)?;
            ctx.report.aliases += 1;
        }
    }
    Ok(())
}
