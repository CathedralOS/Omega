//! Optimizer module role: stage group. Exact selected-lowering and post-allocation machine custody stages.

pub(crate) mod post_allocation_optimizations;

pub use post_allocation_optimizations::*;
