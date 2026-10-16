//! The only `unsafe` of the reader besides the C ABI: mapping the file.

#![allow(unsafe_code)]

use std::fs::File;
use std::io;

/// Maps `file` read-only.
pub(crate) fn map_file(file: &File) -> io::Result<memmap2::Mmap> {
    // SAFETY: the mapping is read-only and private. The file is a system asset that is
    // replaced by renaming a new file over it, never rewritten in place, so its bytes do not
    // change under the mapping; and the reader never trusts the bytes anyway (every access is
    // range-checked), so even a file truncated by someone else cannot cause more than a
    // SIGBUS on access, which is the usual contract of `mmap`.
    unsafe { memmap2::Mmap::map(file) }
}
