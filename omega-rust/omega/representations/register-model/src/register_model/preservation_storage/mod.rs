//! Exact target-owned grouping of ABI-preserved register storage.
//!
//! These catalogs describe abstract save-storage carriers only. They choose no
//! instruction, stack coordinate, prologue, epilogue, or unwind operation.

mod identity;
mod model;
mod validation;

pub use identity::*;
pub use model::*;
pub use validation::*;

#[cfg(test)]
mod tests;
