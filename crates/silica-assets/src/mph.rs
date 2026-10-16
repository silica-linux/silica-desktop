//! Minimal perfect hash tables (compress, hash and displace), one per key space.
//!
//! Table layout (little-endian): `seed u64, bucket_count u32, key_count u32,
//! displacements [u32; bucket_count], slots [u32; key_count]`.
//!
//! A lookup hashes the key with xxh3-64 and the table seed to `h`; the bucket is
//! `(h >> 32) % bucket_count`; the slot is `mix(h ^ displacement * GOLDEN) % key_count`, and the
//! slot array holds the value (an entry index). The caller compares the key stored with the
//! entry, because a key that is not in the table lands on some slot anyway.

use xxhash_rust::xxh3::xxh3_64_with_seed;

use crate::format::{u32_at, u64_at};

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
const HEADER: usize = 16;

fn mix(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn bucket_of(h: u64, buckets: u32) -> u32 {
    let wide = (h >> 32) as u32;
    wide.checked_rem(buckets).unwrap_or(0)
}

fn slot_of(h: u64, displacement: u32, keys: u32) -> u32 {
    let slot = mix(h ^ u64::from(displacement).wrapping_mul(GOLDEN));
    // The remainder is below `keys`, a u32.
    slot.checked_rem(u64::from(keys)).unwrap_or(0) as u32
}

/// A lookup table borrowed from the file. Built only by [`Table::parse`], which checks that
/// the arrays are inside the section.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Table<'a> {
    seed: u64,
    buckets: u32,
    keys: u32,
    displacements: &'a [u8],
    slots: &'a [u8],
}

impl<'a> Table<'a> {
    /// The table in `section`, or `None` if the section is too short for what it declares.
    pub(crate) fn parse(section: &'a [u8]) -> Option<Table<'a>> {
        let seed = u64_at(section, 0)?;
        let buckets = u32_at(section, 8)?;
        let keys = u32_at(section, 12)?;
        let disp_len = (buckets as usize).checked_mul(4)?;
        let slot_len = (keys as usize).checked_mul(4)?;
        let disp_end = HEADER.checked_add(disp_len)?;
        let slot_end = disp_end.checked_add(slot_len)?;
        if (keys == 0) != (buckets == 0) {
            return None;
        }
        Some(Table {
            seed,
            buckets,
            keys,
            displacements: section.get(HEADER..disp_end)?,
            slots: section.get(disp_end..slot_end)?,
        })
    }

    /// The number of keys the table was built for.
    pub(crate) fn len(&self) -> u32 {
        self.keys
    }

    /// The value in slot `index` (used to validate the table).
    pub(crate) fn slot_value(&self, index: u32) -> Option<u32> {
        u32_at(self.slots, (index as usize).checked_mul(4)?)
    }

    /// The value the key hashes to; it belongs to `key` only if the caller's comparison agrees.
    pub(crate) fn find(&self, key: &[u8]) -> Option<u32> {
        if self.keys == 0 {
            return None;
        }
        let h = xxh3_64_with_seed(key, self.seed);
        let bucket = bucket_of(h, self.buckets);
        let displacement = u32_at(self.displacements, (bucket as usize).checked_mul(4)?)?;
        self.slot_value(slot_of(h, displacement, self.keys))
    }
}

/// Why a table could not be built.
#[cfg(feature = "compile")]
#[derive(Debug)]
pub enum BuildError {
    /// The same key twice.
    Duplicate(String),
    /// No seed gave a perfect hash (never seen with real data).
    Exhausted,
}

#[cfg(feature = "compile")]
impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Duplicate(key) => write!(f, "duplicate key {key:?}"),
            BuildError::Exhausted => f.write_str("no seed gives a perfect hash"),
        }
    }
}

/// Builds the table for `entries` (key, value). The result is deterministic: the seed is the
/// first one that works.
#[cfg(feature = "compile")]
pub fn build(entries: &[(&str, u32)]) -> Result<Vec<u8>, BuildError> {
    let n = u32::try_from(entries.len()).map_err(|_| BuildError::Exhausted)?;
    let mut sorted: Vec<&str> = entries.iter().map(|(key, _)| *key).collect();
    sorted.sort_unstable();
    if let Some(pair) = sorted.windows(2).find(|pair| pair.first() == pair.get(1)) {
        return Err(BuildError::Duplicate(
            pair.first().copied().unwrap_or("").to_owned(),
        ));
    }
    let mut out = Vec::new();
    if n == 0 {
        out.extend_from_slice(&0u64.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        return Ok(out);
    }
    let bucket_count = n.div_ceil(4).max(1);
    for seed in 0..64u64 {
        if let Some((displacements, slots)) = try_seed(entries, seed, bucket_count, n) {
            out.extend_from_slice(&seed.to_le_bytes());
            out.extend_from_slice(&bucket_count.to_le_bytes());
            out.extend_from_slice(&n.to_le_bytes());
            for d in displacements {
                out.extend_from_slice(&d.to_le_bytes());
            }
            for s in slots {
                out.extend_from_slice(&s.to_le_bytes());
            }
            return Ok(out);
        }
    }
    Err(BuildError::Exhausted)
}

#[cfg(feature = "compile")]
fn try_seed(
    entries: &[(&str, u32)],
    seed: u64,
    bucket_count: u32,
    n: u32,
) -> Option<(Vec<u32>, Vec<u32>)> {
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); bucket_count as usize];
    let hashes: Vec<u64> = entries
        .iter()
        .map(|(key, _)| xxh3_64_with_seed(key.as_bytes(), seed))
        .collect();
    for (index, h) in hashes.iter().enumerate() {
        buckets
            .get_mut(bucket_of(*h, bucket_count) as usize)?
            .push(index);
    }
    let mut order: Vec<usize> = (0..buckets.len()).collect();
    order.sort_by_key(|&b| (std::cmp::Reverse(buckets.get(b).map_or(0, Vec::len)), b));
    let mut displacements = vec![0u32; bucket_count as usize];
    let mut taken = vec![false; n as usize];
    let mut slots = vec![0u32; n as usize];
    for b in order {
        let members = buckets.get(b)?;
        if members.is_empty() {
            break;
        }
        let mut found = None;
        'search: for displacement in 0..(1u32 << 22) {
            let mut chosen: Vec<u32> = Vec::with_capacity(members.len());
            for &m in members {
                let slot = slot_of(*hashes.get(m)?, displacement, n);
                if *taken.get(slot as usize)? || chosen.contains(&slot) {
                    continue 'search;
                }
                chosen.push(slot);
            }
            found = Some((displacement, chosen));
            break;
        }
        let (displacement, chosen) = found?;
        *displacements.get_mut(b)? = displacement;
        for (&m, &slot) in members.iter().zip(&chosen) {
            *taken.get_mut(slot as usize)? = true;
            *slots.get_mut(slot as usize)? = entries.get(m)?.1;
        }
    }
    Some((displacements, slots))
}

#[cfg(all(test, feature = "compile"))]
mod tests {
    use super::*;

    #[test]
    fn perfect_for_many_sizes() {
        for n in [0usize, 1, 2, 3, 5, 17, 100, 1000, 5000] {
            let keys: Vec<String> = (0..n).map(|i| format!("key-{i}")).collect();
            let entries: Vec<(&str, u32)> = keys
                .iter()
                .enumerate()
                .map(|(i, k)| (k.as_str(), i as u32))
                .collect();
            let bytes = build(&entries).unwrap();
            let table = Table::parse(&bytes).unwrap();
            assert_eq!(table.len() as usize, n);
            for (key, value) in &entries {
                assert_eq!(table.find(key.as_bytes()), Some(*value));
            }
        }
    }

    #[test]
    fn duplicates_are_rejected() {
        assert!(matches!(
            build(&[("a", 0), ("a", 1)]),
            Err(BuildError::Duplicate(_))
        ));
    }

    #[test]
    fn truncated_tables_are_rejected() {
        let bytes = build(&[("a", 0), ("b", 1), ("c", 2)]).unwrap();
        for cut in 0..bytes.len() {
            assert!(Table::parse(&bytes[..cut]).is_none(), "cut at {cut}");
        }
    }
}
