//! Optimizer module role: stage group. Selected-CFG rewrites and their replay evidence.

mod allocation_recovery;
mod fixed_view;
mod literal_folds;
mod selected_lowering;

pub use allocation_recovery::*;
pub use fixed_view::*;
pub use literal_folds::*;
pub use selected_lowering::*;

/// Explicit applicability of the currently architecture-independent rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}
