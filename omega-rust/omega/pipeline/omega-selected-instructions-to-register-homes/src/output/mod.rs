//! Optimizer module role: stage group. Current allocation facts and separate replay evidence.
//!
//! Current program data is owned independently of retained replay inputs.
//! Every route publishes the same program and view; only replay inspects history.

mod baseline;
mod current;
mod fixed_view;
mod literal_folds;
mod model;
mod rematerialization;
mod retained;

pub use model::{AllocationEvidence, AllocationOutput, AllocationReplayError};
pub use retained::RetainedAllocation;

mod sealed {
    pub trait Sealed {}
}

/// Sealed allocation boundary. Implementations reconstruct all source and
/// rewrite evidence before exposing the current program and its allocation.
pub trait AllocationSource: sealed::Sealed {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError>;
}

impl sealed::Sealed for AllocationOutput<'_> {}

impl AllocationSource for AllocationOutput<'_> {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        // Construction has replayed the source. Immutable borrows keep every
        // joined input fixed while subsequent stages use this validated view.
        Ok(self.clone())
    }
}

impl<Source: AllocationSource + ?Sized> sealed::Sealed for Box<Source> {}

impl<Source: AllocationSource + ?Sized> AllocationSource for Box<Source> {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        self.as_ref().replay_allocation()
    }
}

// Projection alone grants no admission. Only replay or the immutable retained
// carrier's checked construction may expose these facts outside this owner.
trait ProjectAllocation {
    fn project_allocation(&self) -> AllocationOutput<'_>;
}
