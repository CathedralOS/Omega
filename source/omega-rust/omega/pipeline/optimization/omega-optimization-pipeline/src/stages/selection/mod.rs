//! Target-operation lowering, assignment, and selected-instruction custody.

pub(crate) mod assignment;
pub(crate) mod optimized_target_operations;
pub(crate) mod selection;

pub use assignment::*;
pub use optimized_target_operations::*;
pub use selection::*;
