#![forbid(unsafe_code)]

//! Optimizer module role: crate map. abstract operations representation.
//!
//! Start at [`abstract_operations`], which defines the program root and maps the
//! subordinate control, value, call, storage, and evidence owners.

pub mod abstract_operations;
pub use abstract_operations::*;
