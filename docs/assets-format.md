# Silica system assets: `silica.assets`

One compiled, memory-mapped file holds the system's shared assets. The shell, the Flutter apps
(through `silica_ui`), the daemon, the portal and Ossido all read the same file. No app ships
its own copy of these images, fonts or sounds.

Status: design accepted by the user on 2026-10-16. Format version 1.

## What is in it

| Asset | Source | Notes |
| --- | --- | --- |
| App icons | silica-icons `apps/` | by app id; the Dock, Launcher, Faro, the app switcher, Mustache, Settings |
| File-type icons | silica-icons `files/` | by type; MIME and UTI map to a type |
| Folders, volumes, Trash | silica-icons `files/` | by role (Documents, Downloads…, external, network, optical, disk image, Trash empty/full) |
| Badges | silica-icons `badges/` | alias, cloud, locked, unreadable, Dock stack close… |
| System symbols | silica-icons `symbols/` | Icon Kit, light/regular/semibold |
| Fonts | Inter, JetBrains Mono | loaded with `FontLoader` from the mapped bytes |
| Sounds | silica-brand | alert sounds, screenshot, Trash |
| Wallpapers | silica-brand | the wallpaper layer and Settings |

Not in it: cursors (the system Adwaita theme, user decision), third-party app icons (their
desktop entry `Icon=`, through the XDG icon theme, is the fallback).

## Sources and compilation

The sources stay readable in their repositories: SVG layers, rendered PNGs and `icon.json`
([silica-icons `docs/icon-format.md`](https://github.com/silica-os/silica-icons/blob/main/docs/icon-format.md)).
A compiler turns them into `silica.assets`:

- `cargo xtask assets [--icons DIR] [--brand DIR] [--out FILE]` (crate `silica-assets`, binary
  `silica-assets-compile`). It is deterministic: the same sources give the same bytes.
- System install: `/usr/share/silica/silica.assets` (package `silica-assets`, noarch).
- Development: `target/silica.assets`, rebuilt by `cargo xtask` when a source changes and copied
  by `cargo xtask os deploy`. `SILICA_ASSETS=<file>` overrides the path. `SILICA_ICONS_DIR`
  stays as a fallback for the old loader until the migration ends.

## File layout

Little-endian. Every offset is from the start of the file; every section starts on a 64-byte
boundary so pixel data can be handed to the GPU without copying.

```
Header (128 bytes)
  magic          [u8; 8]  "SILASSET"
  format         u32      1
  flags          u32      0
  content_version [u8; 32] UTF-8, NUL-padded (e.g. "silica-assets 0.1.0")
  source_hash    [u8; 32] hash of the sources, for rebuild checks
  section_count  u32
  sections       offset u64 of the section table
Section table: { kind u32, reserved u32, offset u64, length u64 } × section_count
Sections (any order, by kind):
  STRINGS   UTF-8 strings, NUL-terminated; referenced by offset
  ENTRIES   fixed-size Entry records
  VARIANTS  fixed-size Variant records
  SLOTS     fixed-size dynamic-content records
  LOOKUP_*  one minimal perfect hash table per key space (below)
  DATA      pixel data, font files, sound files (each blob 64-byte aligned)
```

### Lookup tables

One table per key space: `APP` (app id, e.g. `dev.frascella.Mail`), `TYPE` (type id, e.g.
`public.pdf`), `MIME` (`application/pdf` → type id), `UTI` (alias → type id), `ROLE`
(`folder.downloads`, `volume.external`, `badge.alias`, `dock.stack-close`…), `SYMBOL`
(`<weight>/<name>`), `FONT`, `SOUND`, `WALLPAPER`.

Each table is a minimal perfect hash (CHD or PTHash): `{ seed u64, bucket_count u32,
key_count u32, displacements [u32; bucket_count], slots [u32; key_count] }`. A lookup hashes
the key (xxh3-64 with the seed), reads one displacement and one slot, and compares the key
string stored in the entry to reject misses. No parsing, no allocation.

### Entry (64 bytes)

```
name        u32  string offset (the key)
kind        u16  app | file | folder | volume | badge | symbol | font | sound | wallpaper
flags       u16  has_dynamic, tintable, has_dark, …
variants    u32  first Variant index
variant_count u16
slot_count  u16
slots       u32  first Slot index
alias_of    u32  entry index this one points to (MIME/UTI aliases), or u32::MAX
reserved    [u8; 40]
```

### Variant (32 bytes)

```
theme       u8   default | dark | clear | clear-dark | tinted | tinted-dark
layer       u8   composite | hand-hour | hand-minute | hand-second | cap | background | foreground
encoding    u8   rgba8-premul | qoi | ttf | otf | ogg | flac | svg
reserved    u8
size_px     u16  pixel side (square images), 0 for non-images
scale       u16  reserved
data        u64  offset into DATA
length      u32  bytes
reserved    u32
```

Images are stored for pixel sides 16, 32, 64, 128 (raw `rgba8-premul`, ready for
`ImageDescriptor.raw` / GPU upload) and 256, 512, 1024 (`qoi`). A reader picks the smallest
stored side ≥ requested logical size × device pixel ratio and scales down.

### Dynamic slots (32 bytes)

```
kind        u8   clock-hand | clock-cap | calendar-month | calendar-day | folder-emblem | type-label
unit        u8   hour | minute | second (clock)
min_size    u16  smallest icon side (pt) at which the slot is drawn
pivot / box f32 × 4  pivot (x, y) for hands, or box (x, y, w, h) on the 1024 canvas
font        u32  string offset (font entry name) for text slots
size_weight u16 × 2  font size and weight on the 1024 canvas
color       u32 × 2  light and dark RGBA
format      u32  string offset: "short" (localized 3-letter month, capitals), "day", …
```

Clock hands and caps are stored as their own image variants (layer field) and rotated about
the pivot at draw time. Calendar text, file-type labels and folder emblems are drawn at draw
time with the font and symbol entries of the same file.

## Readers

- **Rust**: crate `silica-assets` (in silica-desktop `crates/`): `Assets::open(path)` maps the
  file read-only; lookups return borrowed slices. It never panics on a malformed file: a bad
  magic, an unknown format or an out-of-range offset returns an error, and the caller falls
  back. A C ABI (`libsilica_assets.so`: open, lookup by key space, variant pick, blob pointer
  and length, close) serves `dart:ffi`.
- **silica_ui**: `SilicaAssets` resolves app, type (MIME / UTI / path), folder role, volume,
  badge, symbol, font, sound and wallpaper requests through the C ABI; widgets get pixels as
  `ui.ImageDescriptor.raw` (no decode) or decode QOI; a size-bounded cache keyed by
  (entry, theme, side); theme follows `AmalfiTheme` at runtime; missing file or unknown format
  gives drawn fallbacks (generic document, folder, app), never a crash.

All processes map the same file, so its pages are shared in memory across the whole session.

## Versioning

- `format` changes only when the layout changes; readers accept the formats they know and fall
  back otherwise.
- `content_version` follows the `silica-assets` package version; apps never pin it.
- A newer `silica.assets` with the same format is picked up at the next process start.
