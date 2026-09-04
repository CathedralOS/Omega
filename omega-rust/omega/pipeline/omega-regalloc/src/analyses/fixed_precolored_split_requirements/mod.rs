//! Optimizer module role: executable entrance. Fixed-driven split requirements.
//!
//! This boundary partitions authenticated source live points and records where
//! an incoming and fixed-use exact-view domain are incompatible. It creates no
//! home, recovery strategy, transformed interval, or physical program.

mod compute;
mod error;
mod identity;
mod model;
mod replay;
mod validation;

pub use error::FixedPrecoloredSplitRequirementError;
pub use identity::fixed_precolored_split_requirement_plan_identity;
pub use model::*;
pub use validation::validate_fixed_precolored_split_requirements;

pub fn analyze_fixed_precolored_split_requirements(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    fixed: &crate::ValidatedFixedPrecoloredIntervals,
    policy: FixedPrecoloredSplitRequirementPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError> {
    let plan = compute::compute(ranges, legality, fixed, policy, budget)?;
    validate_fixed_precolored_split_requirements(ranges, legality, fixed, plan)
}
