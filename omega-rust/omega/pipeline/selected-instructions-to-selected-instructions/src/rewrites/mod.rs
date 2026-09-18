//! Optimizer module role: stage group. Selected-CFG rewrites and their replay evidence.

mod address_fold;
mod allocation_recovery;
mod arm_relocation;
mod block_edges;
mod bypass_relocation;
mod condition_state;
mod confluence_relocation;
mod constant_boolean;
mod constant_branch;
mod copy_removal;
mod dead_compare;
mod dead_path;
mod dead_store;
mod diamond_relocation;
mod edge_relocation;
mod fixed_view;
mod fork_relocation;
mod join_relocation;
mod literal_arithmetic;
mod literal_compare;
mod literal_folds;
mod literal_minuend;
mod load_forwarding;
mod local_relocation;
mod local_schedule;
mod place_storage;
mod predecessor_relocation;
mod redundant_extension;
mod run_relocation;
mod runtime_rematerialization;
mod runtime_spill;
mod selected_lowering;
mod store_motion;
#[cfg(feature = "test-support")]
pub mod test_support;
mod window_hazards;

pub use address_fold::*;
pub use allocation_recovery::*;
pub use arm_relocation::*;
pub use bypass_relocation::*;
pub use confluence_relocation::*;
pub use constant_boolean::*;
pub use constant_branch::*;
pub use copy_removal::*;
pub use dead_compare::*;
pub use dead_store::*;
pub use diamond_relocation::*;
pub use edge_relocation::*;
pub use fixed_view::*;
pub use fork_relocation::*;
pub use join_relocation::*;
pub use literal_arithmetic::*;
pub use literal_compare::*;
pub use literal_folds::*;
pub use literal_minuend::*;
pub use load_forwarding::*;
pub use local_relocation::*;
pub use local_schedule::*;
pub use predecessor_relocation::*;
pub use redundant_extension::*;
pub use run_relocation::*;
pub use runtime_rematerialization::*;
pub use runtime_spill::*;
pub use selected_lowering::*;
pub use store_motion::*;

/// Explicit applicability of the currently architecture-independent rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}
