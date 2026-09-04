//! Optimizer module role: stage group. Compiler-facing entrances that select and report complete pipeline routes.

pub(crate) mod physical_pipeline;
mod psi_optimization;
mod report;

pub use physical_pipeline::*;
pub use psi_optimization::*;
pub use report::*;
