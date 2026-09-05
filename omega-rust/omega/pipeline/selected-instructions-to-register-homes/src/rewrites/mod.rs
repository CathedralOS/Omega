//! Optimizer module role: stage group. Selected-instruction rewrites and retained replay evidence.

mod fixed_view;
mod literal_folds;
mod rematerialization;

pub use fixed_view::*;
pub use literal_folds::*;
pub use rematerialization::*;

mod allocation_recovery;
mod selected_lowering;

pub use allocation_recovery::*;
pub use selected_lowering::*;

/// Register-allocation rules are architecture-independent. The explicit
/// marker keeps portability in each owning catalog instead of implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}
