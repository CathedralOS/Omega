//! Optimizer module role: stage group. Target-operation lowering, assignment, and selected-instruction custody.

pub(crate) mod assignment;
pub(crate) mod optimized_target_operations;
#[allow(clippy::module_inception)]
// The inner module is the selection phase within this stage group.
pub(crate) mod selection;

pub use assignment::*;
pub use optimized_target_operations::*;
pub use selection::*;
