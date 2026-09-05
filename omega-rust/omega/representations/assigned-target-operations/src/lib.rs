#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Assigned target-operation representation.
//!
//! Start at [`assigned_operations::AssignedOperationPlan`]. Concrete storage,
//! control, calls and value shapes are owned beneath that program root.

pub mod assigned_operations;
pub use assigned_operations::*;
