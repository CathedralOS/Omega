#![forbid(unsafe_code)]

//! Portable filesystem authority, canonical metadata, and operation observations.
//! Staging resource accounting and private-directory custody are independent
//! of provider execution; runtime owners perform the authorized operations.

pub(crate) use optimization::codec_bytes;

mod filesystem;
mod filesystem_sponsor;

pub use filesystem::*;
pub use filesystem_sponsor::*;
