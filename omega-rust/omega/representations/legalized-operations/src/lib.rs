#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Target-legal program representation.
//!
//! Start at [`legalized_operations::LegalizedOperationPlan`]. Control flow,
//! calls and legality are subordinate representation owners.

pub mod legalized_operations;
pub use legalized_operations::*;
