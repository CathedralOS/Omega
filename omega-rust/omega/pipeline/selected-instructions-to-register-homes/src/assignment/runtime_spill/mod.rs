//! Optimizer module role: stage group. Executable spill recovery and its independent replay.

mod model;
mod recovery;
pub(crate) mod replay;

pub(crate) use model::RuntimeSpillAllocation;
pub use model::RuntimeSpillAllocationError;
pub(crate) use recovery::{assign_source, recover};
