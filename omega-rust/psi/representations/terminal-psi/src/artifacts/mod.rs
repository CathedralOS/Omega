//! Replaceable proof and presentation companions, separate from module semantics.
//!
//! These are data schemas, not admission results. Canonical codecs and checkers
//! consume them independently; merely constructing them grants no authority.

mod checked_program_entry;
mod debug_map;
mod proof_bundle;
pub use checked_program_entry::*;
pub use debug_map::*;
pub use proof_bundle::*;
