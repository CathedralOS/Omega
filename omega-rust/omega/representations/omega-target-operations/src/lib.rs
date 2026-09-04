#![forbid(unsafe_code)]

//! Optimizer module role: crate map. target operations representation.
//!
//! Start at [`target_operations`], which defines the program root and maps the
//! subordinate control, value, call, storage, and evidence owners.

pub mod target_operations;
pub use target_operations::*;
