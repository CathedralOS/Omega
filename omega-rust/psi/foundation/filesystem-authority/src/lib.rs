#![forbid(unsafe_code)]

//! Portable filesystem authority, canonical metadata, and operation observations.
//! Staging resource accounting and private-directory custody are independent
//! of provider execution; runtime owners perform the authorized operations.

pub(crate) use optimization::codec_bytes;

mod filesystem;
mod filesystem_sponsor;

pub use filesystem::*;
pub use filesystem_sponsor::*;

/// Schema for retained filesystem operation-attempt observations, independent
/// of the interpreter that issues them. Observations grant no filesystem access.
pub const FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION: u32 = 20;
