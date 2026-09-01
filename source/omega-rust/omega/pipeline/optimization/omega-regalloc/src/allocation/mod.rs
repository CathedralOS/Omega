//! Optimizer module role: stage group. Decisions that assign homes or select values requiring recovery.

pub(crate) mod home_assignment;
pub(crate) mod logical_spill_operations;
mod post_allocation_manifest;
pub(crate) mod spill_choice;

pub use home_assignment::*;
pub use logical_spill_operations::*;
pub use post_allocation_manifest::*;
pub use spill_choice::*;
