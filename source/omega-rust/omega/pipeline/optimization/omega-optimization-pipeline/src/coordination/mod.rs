//! Compiler-facing entrances that select and report complete pipeline routes.

pub(crate) mod native_continuation;
pub(crate) mod physical_pipeline;
mod psi_optimization;
mod report;

pub use native_continuation::*;
pub use physical_pipeline::*;
pub use psi_optimization::*;
pub use report::*;
