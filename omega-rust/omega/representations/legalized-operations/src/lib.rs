#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Target-legal program representation.
//!
//! Start at [`legalized_operations::LegalizedOperationPlan`]. Control flow,
//! calls, values and legality recipes are subordinate representation owners.

pub mod legalized_operations;
pub use legalized_operations::*;
