//! The reader: maps `silica.assets` read-only, validates the whole structure once and then
//! serves lookups, variant picks and blobs as borrowed slices.
//!
//! The file is treated as hostile. `open` checks every offset, length and enum once, so the
//! accessors cannot be led out of range by a corrupt file; they still use checked reads and
//! return `None` instead of indexing, so a file that changes under the mapping cannot make the
//! process panic either.

use std::fmt;
use std::fs::File;
use std::path::Path;

use crate::ffi::map;
use crate::format::*;
use crate::mph::Table;

/// Why a file could not be opened.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read or mapped.
    Io(std::io::Error),
    /// Shorter than the structure it declares.
    Truncated,
    /// Not a `silica.assets` file.
    BadMagic,
    /// A format version this reader does not know; the caller falls back.
    UnsupportedFormat(u32),
    /// A structure that contradicts itself (offset out of range, unknown enum value, ...).
    Corrupt(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "cannot read the assets file: {e}"),
            Error::Truncated => f.write_str("the assets file is truncated"),
            Error::BadMagic => f.write_str("not a silica.assets file (bad magic)"),
            Error::UnsupportedFormat(v) => write!(f, "unsupported silica.assets format {v}"),
            Error::Corrupt(what) => write!(f, "corrupt silica.assets file: {what}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e)
    }
}

/// The index of an entry in the file. Only [`Assets`] hands them out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(pub u32);

enum Bytes {
    Mapped(memmap2::Mmap),
    Owned(Vec<u8>),
}

impl Bytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            Bytes::Mapped(map) => map,
            Bytes::Owned(vec) => vec,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Span {
    start: usize,
    len: usize,
}

const SPACES: usize = 10;

/// An opened `silica.assets`.
pub struct Assets {
    bytes: Bytes,
    content_version: String,
    source_hash: [u8; 32],
    strings: Span,
    entries: Span,
    variants: Span,
    slots: Span,
    data: Span,
    tables: [Option<Span>; SPACES],
}

impl fmt::Debug for Assets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Assets")
            .field("content_version", &self.content_version)
            .field("entries", &self.entry_count())
            .finish()
    }
}

/// What a variant is asked for. See [`Assets::pick`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Query {
    pub theme: Theme,
    pub layer: Layer,
    pub state: State,
    /// Wanted pixel side (logical size times device pixel ratio).
    pub side: u32,
}

impl Query {
    /// The whole icon in the normal state.
    pub fn composite(theme: Theme, side: u32) -> Query {
        Query {
            theme,
            layer: Layer::Composite,
            state: State::Normal,
            side,
        }
    }
}

/// One stored rendition of an entry.
#[derive(Clone, Copy, Debug)]
pub struct Variant<'a> {
    pub theme: Theme,
    pub layer: Layer,
    pub encoding: Encoding,
    pub state: State,
    /// Pixel side of a square image, 0 for fonts, sounds and symbols; the width for wallpapers.
    pub size_px: u16,
    pub width: u16,
    pub height: u16,
    /// The bytes, inside the mapping.
    pub data: &'a [u8],
}

/// A dynamic-content record of an entry.
#[derive(Clone, Copy, Debug)]
pub struct Slot<'a> {
    pub kind: SlotKind,
    pub unit: u8,
    /// Smallest icon side, in points, at which the slot is drawn.
    pub min_size: u16,
    /// Pivot `(x, y, 0, 0)` for hands, box `(x, y, w, h)` on the 1024 canvas otherwise.
    pub rect: [f32; 4],
    pub font: Option<&'a str>,
    pub size: u16,
    pub weight: u16,
    /// Light and dark colors as `0xRRGGBBAA`.
    pub color: [u32; 2],
    pub format: Option<&'a str>,
    pub tracking: f32,
    pub max_width: f32,
}

/// An entry of the file.
#[derive(Clone, Copy)]
pub struct Entry<'a> {
    assets: &'a Assets,
    id: EntryId,
    name: &'a str,
    kind: EntryKind,
    flags: u16,
    first_variant: u32,
    variant_count: u16,
    first_slot: u32,
    slot_count: u16,
    alias_of: u32,
}

impl fmt::Debug for Entry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl<'a> Entry<'a> {
    pub fn id(&self) -> EntryId {
        self.id
    }
    /// The key of the entry.
    pub fn name(&self) -> &'a str {
        self.name
    }
    pub fn kind(&self) -> EntryKind {
        self.kind
    }
    pub fn flags(&self) -> u16 {
        self.flags
    }
    /// The entry an alias points to.
    pub fn alias_of(&self) -> Option<EntryId> {
        (self.alias_of != NONE).then_some(EntryId(self.alias_of))
    }
    pub fn variant_count(&self) -> usize {
        usize::from(self.variant_count)
    }
    pub fn slot_count(&self) -> usize {
        usize::from(self.slot_count)
    }
    /// The variant `index` of this entry.
    pub fn variant(&self, index: usize) -> Option<Variant<'a>> {
        if index >= self.variant_count() {
            return None;
        }
        self.assets
            .variant(self.first_variant.checked_add(u32::try_from(index).ok()?)?)
    }
    pub fn variants(&self) -> impl Iterator<Item = Variant<'a>> + use<'a> {
        let entry = *self;
        (0..entry.variant_count()).filter_map(move |i| entry.variant(i))
    }
    /// The slot `index` of this entry.
    pub fn slot(&self, index: usize) -> Option<Slot<'a>> {
        if index >= self.slot_count() {
            return None;
        }
        self.assets
            .slot(self.first_slot.checked_add(u32::try_from(index).ok()?)?)
    }
    pub fn slots(&self) -> impl Iterator<Item = Slot<'a>> + use<'a> {
        let entry = *self;
        (0..entry.slot_count()).filter_map(move |i| entry.slot(i))
    }
}

fn theme_chain(theme: Theme) -> &'static [Theme] {
    match theme {
        Theme::Default => &[Theme::Default],
        Theme::Dark => &[Theme::Dark, Theme::Default],
        Theme::Clear => &[Theme::Clear, Theme::Default],
        Theme::ClearDark => &[Theme::ClearDark, Theme::Clear, Theme::Dark, Theme::Default],
        Theme::Tinted => &[Theme::Tinted, Theme::Default],
        Theme::TintedDark => &[
            Theme::TintedDark,
            Theme::Tinted,
            Theme::Dark,
            Theme::Default,
        ],
    }
}

fn span_of(bytes: &[u8], offset: u64, length: u64) -> Result<Span, Error> {
    let start = usize::try_from(offset).map_err(|_| Error::Corrupt("offset too large"))?;
    let len = usize::try_from(length).map_err(|_| Error::Corrupt("length too large"))?;
    let end = start
        .checked_add(len)
        .ok_or(Error::Corrupt("section overflows"))?;
    if end > bytes.len() {
        return Err(Error::Truncated);
    }
    Ok(Span { start, len })
}

impl Assets {
    /// Maps the file read-only and checks it.
    pub fn open(path: impl AsRef<Path>) -> Result<Assets, Error> {
        let file = File::open(path)?;
        let map = map::map_file(&file)?;
        Assets::parse(Bytes::Mapped(map))
    }

    /// Checks a file already in memory (tests, tools).
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Assets, Error> {
        Assets::parse(Bytes::Owned(bytes))
    }

    fn parse(bytes: Bytes) -> Result<Assets, Error> {
        let b = bytes.as_slice();
        if b.len() < HEADER_SIZE {
            return Err(if b.get(..8).is_some_and(|m| m != MAGIC) {
                Error::BadMagic
            } else {
                Error::Truncated
            });
        }
        if b.get(..8) != Some(&MAGIC[..]) {
            return Err(Error::BadMagic);
        }
        let format = u32_at(b, 8).ok_or(Error::Truncated)?;
        if format != FORMAT {
            return Err(Error::UnsupportedFormat(format));
        }
        let version_bytes = b.get(16..48).ok_or(Error::Truncated)?;
        let version_len = version_bytes
            .iter()
            .position(|&c| c == 0)
            .ok_or(Error::Corrupt("content version is not NUL-terminated"))?;
        let content_version = std::str::from_utf8(version_bytes.get(..version_len).unwrap_or(&[]))
            .map_err(|_| Error::Corrupt("content version is not UTF-8"))?
            .to_owned();
        let mut source_hash = [0u8; 32];
        source_hash.copy_from_slice(b.get(48..80).ok_or(Error::Truncated)?);
        let section_count = u32_at(b, 80).ok_or(Error::Truncated)? as usize;
        let table_offset = u64_at(b, 88).ok_or(Error::Truncated)?;
        let file_length = u64_at(b, 96).ok_or(Error::Truncated)?;
        if file_length != b.len() as u64 {
            return Err(if file_length > b.len() as u64 {
                Error::Truncated
            } else {
                Error::Corrupt("file length does not match the header")
            });
        }
        if section_count > 64 {
            return Err(Error::Corrupt("too many sections"));
        }
        let table = span_of(b, table_offset, (section_count * SECTION_SIZE) as u64)?;

        let mut strings = None;
        let mut entries = None;
        let mut variants = None;
        let mut slots = None;
        let mut data = None;
        let mut tables: [Option<Span>; SPACES] = [None; SPACES];
        for i in 0..section_count {
            let at = table.start + i * SECTION_SIZE;
            let kind = u32_at(b, at).ok_or(Error::Truncated)?;
            let offset = u64_at(b, at + 8).ok_or(Error::Truncated)?;
            let length = u64_at(b, at + 16).ok_or(Error::Truncated)?;
            let span = span_of(b, offset, length)?;
            if !span.start.is_multiple_of(ALIGN) {
                return Err(Error::Corrupt("section is not aligned"));
            }
            let slot = match kind {
                SECTION_STRINGS => &mut strings,
                SECTION_ENTRIES => &mut entries,
                SECTION_VARIANTS => &mut variants,
                SECTION_SLOTS => &mut slots,
                SECTION_DATA => &mut data,
                k if k >= SECTION_LOOKUP && ((k - SECTION_LOOKUP) as usize) < SPACES => tables
                    .get_mut((k - SECTION_LOOKUP) as usize)
                    .ok_or(Error::Corrupt("lookup index"))?,
                // A section this version does not know: skipped (forward compatibility).
                _ => continue,
            };
            if slot.replace(span).is_some() {
                return Err(Error::Corrupt("duplicate section"));
            }
        }
        let missing = Error::Corrupt("missing section");
        let assets = Assets {
            content_version,
            source_hash,
            strings: strings.ok_or(Error::Corrupt("missing section"))?,
            entries: entries.ok_or(Error::Corrupt("missing section"))?,
            variants: variants.ok_or(Error::Corrupt("missing section"))?,
            slots: slots.ok_or(missing)?,
            data: data.ok_or(Error::Corrupt("missing section"))?,
            tables,
            bytes,
        };
        assets.validate()?;
        Ok(assets)
    }

    fn validate(&self) -> Result<(), Error> {
        if !self.entries.len.is_multiple_of(ENTRY_SIZE)
            || !self.variants.len.is_multiple_of(VARIANT_SIZE)
            || !self.slots.len.is_multiple_of(SLOT_SIZE)
        {
            return Err(Error::Corrupt("record section length"));
        }
        if std::str::from_utf8(self.section(self.strings)).is_err() {
            return Err(Error::Corrupt("strings are not UTF-8"));
        }
        let entry_count = self.entry_count();
        let variant_count = self.variants.len / VARIANT_SIZE;
        let slot_count = self.slots.len / SLOT_SIZE;
        let data_len = self.data.len as u64;
        for index in 0..entry_count {
            let at = self.entries.start + index * ENTRY_SIZE;
            let b = self.bytes.as_slice();
            let name = u32_at(b, at).ok_or(Error::Truncated)?;
            if self.string_at(name).is_none() {
                return Err(Error::Corrupt("entry name"));
            }
            let kind = u16_at(b, at + 4).ok_or(Error::Truncated)?;
            let kind = EntryKind::from_u32(u32::from(kind)).ok_or(Error::Corrupt("entry kind"))?;
            let first_variant = u32_at(b, at + 8).ok_or(Error::Truncated)? as usize;
            let nv = usize::from(u16_at(b, at + 12).ok_or(Error::Truncated)?);
            let nslots = usize::from(u16_at(b, at + 14).ok_or(Error::Truncated)?);
            let first_slot = u32_at(b, at + 16).ok_or(Error::Truncated)? as usize;
            let alias_of = u32_at(b, at + 20).ok_or(Error::Truncated)?;
            if first_variant.saturating_add(nv) > variant_count {
                return Err(Error::Corrupt("entry variants out of range"));
            }
            if first_slot.saturating_add(nslots) > slot_count {
                return Err(Error::Corrupt("entry slots out of range"));
            }
            if kind == EntryKind::Alias {
                let target = alias_of as usize;
                if alias_of == NONE || target >= entry_count {
                    return Err(Error::Corrupt("alias target out of range"));
                }
                let target_kind = u16_at(b, self.entries.start + target * ENTRY_SIZE + 4)
                    .ok_or(Error::Truncated)?;
                if u32::from(target_kind) == EntryKind::Alias as u32 {
                    return Err(Error::Corrupt("alias of an alias"));
                }
            } else if alias_of != NONE {
                return Err(Error::Corrupt("alias_of on a real entry"));
            }
        }
        for index in 0..variant_count {
            let at = self.variants.start + index * VARIANT_SIZE;
            let b = self.bytes.as_slice();
            let known = |offset: usize, ok: bool| -> Result<(), Error> {
                let _ = offset;
                if ok {
                    Ok(())
                } else {
                    Err(Error::Corrupt("variant enum"))
                }
            };
            let byte = |o: usize| b.get(at + o).map(|&v| u32::from(v)).ok_or(Error::Truncated);
            known(0, Theme::from_u32(byte(0)?).is_some())?;
            known(1, Layer::from_u32(byte(1)?).is_some())?;
            known(2, Encoding::from_u32(byte(2)?).is_some())?;
            known(3, State::from_u32(byte(3)?).is_some())?;
            let offset = u64_at(b, at + 8).ok_or(Error::Truncated)?;
            let length = u64::from(u32_at(b, at + 16).ok_or(Error::Truncated)?);
            if offset.checked_add(length).is_none_or(|end| end > data_len) {
                return Err(Error::Corrupt("variant data out of range"));
            }
        }
        for index in 0..slot_count {
            let at = self.slots.start + index * SLOT_SIZE;
            let b = self.bytes.as_slice();
            let kind = u32::from(*b.get(at).ok_or(Error::Truncated)?);
            if SlotKind::from_u32(kind).is_none() {
                return Err(Error::Corrupt("slot kind"));
            }
            for field in [20, 36] {
                let offset = u32_at(b, at + field).ok_or(Error::Truncated)?;
                if offset != NONE && self.string_at(offset).is_none() {
                    return Err(Error::Corrupt("slot string"));
                }
            }
        }
        for table in self.tables.iter().flatten() {
            let parsed =
                Table::parse(self.section(*table)).ok_or(Error::Corrupt("lookup table"))?;
            for slot in 0..parsed.len() {
                let value = parsed
                    .slot_value(slot)
                    .ok_or(Error::Corrupt("lookup table"))?;
                if value as usize >= entry_count {
                    return Err(Error::Corrupt("lookup value out of range"));
                }
            }
        }
        Ok(())
    }

    fn section(&self, span: Span) -> &[u8] {
        self.bytes
            .as_slice()
            .get(span.start..span.start.saturating_add(span.len))
            .unwrap_or(&[])
    }

    fn string_at(&self, offset: u32) -> Option<&str> {
        let strings = self.section(self.strings);
        let rest = strings.get(offset as usize..)?;
        let len = rest.iter().position(|&c| c == 0)?;
        std::str::from_utf8(rest.get(..len)?).ok()
    }

    /// The version of the package that built the file.
    pub fn content_version(&self) -> &str {
        &self.content_version
    }

    /// Pointer to the NUL-terminated content version inside the file (C ABI).
    pub(crate) fn content_version_ptr(&self) -> *const std::ffi::c_char {
        self.bytes
            .as_slice()
            .get(16..)
            .map_or(std::ptr::null(), |s| s.as_ptr().cast())
    }

    /// The hash of the sources the file was built from.
    pub fn source_hash(&self) -> &[u8; 32] {
        &self.source_hash
    }

    /// The number of entries (including aliases).
    pub fn entry_count(&self) -> usize {
        self.entries.len / ENTRY_SIZE
    }

    /// The entry with this index.
    pub fn entry(&self, id: EntryId) -> Option<Entry<'_>> {
        let entries = self.section(self.entries);
        let at = (id.0 as usize).checked_mul(ENTRY_SIZE)?;
        let record = entries.get(at..at.checked_add(ENTRY_SIZE)?)?;
        Some(Entry {
            assets: self,
            id,
            name: self.string_at(u32_at(record, 0)?)?,
            kind: EntryKind::from_u32(u32::from(u16_at(record, 4)?))?,
            flags: u16_at(record, 6)?,
            first_variant: u32_at(record, 8)?,
            variant_count: u16_at(record, 12)?,
            slot_count: u16_at(record, 14)?,
            first_slot: u32_at(record, 16)?,
            alias_of: u32_at(record, 20)?,
        })
    }

    /// Every entry, in file order.
    pub fn entries(&self) -> impl Iterator<Item = Entry<'_>> {
        (0..self.entry_count()).filter_map(|i| self.entry(EntryId(u32::try_from(i).ok()?)))
    }

    /// Looks up `key` in a key space. An alias is resolved: the result is the entry that holds
    /// the images. A key that is not in the file gives `None` (the hash table says where a key
    /// would be, the stored name decides).
    pub fn lookup(&self, space: KeySpace, key: &str) -> Option<EntryId> {
        let span = (*self.tables.get(space as usize)?)?;
        let table = Table::parse(self.section(span))?;
        let found = EntryId(table.find(key.as_bytes())?);
        let entry = self.entry(found)?;
        if entry.name() != key {
            return None;
        }
        match entry.alias_of() {
            Some(target) => Some(target),
            None => Some(found),
        }
    }

    /// Picks the variant to draw: of the first theme in the fallback chain that has the layer
    /// and state, the smallest stored side that is at least `side`, or the largest one.
    pub fn pick(&self, id: EntryId, query: Query) -> Option<Variant<'_>> {
        let entry = self.entry(id)?;
        for &theme in theme_chain(query.theme) {
            let mut best: Option<Variant<'_>> = None;
            for v in entry.variants() {
                if v.theme != theme || v.layer != query.layer || v.state != query.state {
                    continue;
                }
                best = Some(match best {
                    None => v,
                    Some(b) => {
                        let want = u64::from(query.side);
                        let (bs, vs) = (u64::from(b.size_px), u64::from(v.size_px));
                        let better = match (bs >= want, vs >= want) {
                            (true, true) => vs < bs,
                            (false, true) => true,
                            (true, false) => false,
                            (false, false) => vs > bs,
                        };
                        if better { v } else { b }
                    }
                });
            }
            if best.is_some() {
                return best;
            }
        }
        None
    }

    fn variant(&self, index: u32) -> Option<Variant<'_>> {
        let records = self.section(self.variants);
        let at = (index as usize).checked_mul(VARIANT_SIZE)?;
        let r = records.get(at..at.checked_add(VARIANT_SIZE)?)?;
        let offset = usize::try_from(u64_at(r, 8)?).ok()?;
        let length = u32_at(r, 16)? as usize;
        let data = self
            .section(self.data)
            .get(offset..offset.checked_add(length)?)?;
        let byte = |o: usize| r.get(o).map(|&v| u32::from(v));
        Some(Variant {
            theme: Theme::from_u32(byte(0)?)?,
            layer: Layer::from_u32(byte(1)?)?,
            encoding: Encoding::from_u32(byte(2)?)?,
            state: State::from_u32(byte(3)?)?,
            size_px: u16_at(r, 4)?,
            width: u16_at(r, 20)?,
            height: u16_at(r, 22)?,
            data,
        })
    }

    fn slot(&self, index: u32) -> Option<Slot<'_>> {
        let records = self.section(self.slots);
        let at = (index as usize).checked_mul(SLOT_SIZE)?;
        let r = records.get(at..at.checked_add(SLOT_SIZE)?)?;
        let string = |o: usize| {
            let offset = u32_at(r, o)?;
            if offset == NONE {
                Some(None)
            } else {
                self.string_at(offset).map(Some)
            }
        };
        Some(Slot {
            kind: SlotKind::from_u32(u32::from(*r.first()?))?,
            unit: *r.get(1)?,
            min_size: u16_at(r, 2)?,
            rect: [f32_at(r, 4)?, f32_at(r, 8)?, f32_at(r, 12)?, f32_at(r, 16)?],
            font: string(20)?,
            size: u16_at(r, 24)?,
            weight: u16_at(r, 26)?,
            color: [u32_at(r, 28)?, u32_at(r, 32)?],
            format: string(36)?,
            tracking: f32_at(r, 40)?,
            max_width: f32_at(r, 44)?,
        })
    }
}
