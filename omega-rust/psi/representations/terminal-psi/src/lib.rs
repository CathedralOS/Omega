#![forbid(unsafe_code)]

//! Self-contained, target-neutral Terminal Psi.
//!
//! Begin at [`terminal_module`]: one current program with concept-owned
//! vocabulary. Optimizations precede publication; target realization follows it.
//! Canonical serialization and independent verification have separate owners.

mod artifacts;
pub mod terminal_module;
pub use artifacts::*;

pub use language_core::BindingRelevance;
pub use terminal_module::*;
