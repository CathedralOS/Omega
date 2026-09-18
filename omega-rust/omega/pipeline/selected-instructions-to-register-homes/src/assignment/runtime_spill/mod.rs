//! Optimizer module role: stage group. Executable spill recovery and its independent replay.

mod model;
mod recovery;
pub(crate) mod replay;

pub(crate) use model::RuntimeSpillAllocation;
pub use model::RuntimeSpillAllocationError;
pub(crate) use recovery::{
    assign_source, recover, recover_after_active_resident_rematerialization,
    recover_after_declined_fixed_view_probe, recover_after_fixed_view_copies,
};
