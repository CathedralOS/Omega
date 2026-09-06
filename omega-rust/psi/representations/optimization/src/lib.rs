#![forbid(unsafe_code)]

//! Optimizer module role: crate map.
//!
//! Target-neutral Psi optimization vocabulary and canonical selections. Psi
//! owns this catalog because every entry executes before Terminal publication;
//! target and physical optimization identities remain Omega-owned.

pub mod optimization_selections;

pub use optimization_selections::*;
