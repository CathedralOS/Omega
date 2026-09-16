//! Optimizer module role: stage group. Selected-CFG rewrites and their replay evidence.

mod address_fold;
mod allocation_recovery;
mod copy_removal;
mod dead_store;
mod fixed_view;
mod literal_compare;
mod literal_folds;
mod literal_minuend;
mod load_forwarding;
mod local_schedule;
mod redundant_extension;
mod runtime_rematerialization;
mod runtime_spill;
mod selected_lowering;
mod store_motion;
#[cfg(feature = "test-support")]
pub mod test_support;

pub use address_fold::*;
pub use allocation_recovery::*;
pub use copy_removal::*;
pub use dead_store::*;
pub use fixed_view::*;
pub use literal_compare::*;
pub use literal_folds::*;
pub use literal_minuend::*;
pub use load_forwarding::*;
pub use local_schedule::*;
pub use redundant_extension::*;
pub use runtime_rematerialization::*;
pub use runtime_spill::*;
pub use selected_lowering::*;
pub use store_motion::*;

/// Explicit applicability of the currently architecture-independent rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}
