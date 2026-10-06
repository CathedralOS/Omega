#![forbid(unsafe_code)]

//! Portable filesystem authority, canonical metadata, and operation observations.
//! Stateful filesystem sponsorship and provider execution remain with their runtime owners.

mod filesystem;
mod filesystem_sponsor;

pub use filesystem::*;
pub use filesystem_sponsor::*;
