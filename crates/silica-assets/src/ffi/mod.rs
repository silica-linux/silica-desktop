//! The only places with `unsafe` (cargo xtask unsafe): mapping the file and the C ABI.

pub mod c_api;
pub(crate) mod map;
