//! Optimizer module role: stage group. Compiler-facing entrances that select and report complete pipeline routes.

mod abstract_operation_optimization;
pub(crate) mod physical_pipeline;
mod report;

pub use abstract_operation_optimization::*;
pub use physical_pipeline::*;
pub use report::*;
