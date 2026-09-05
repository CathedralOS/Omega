//! Optimizer module role: executable entrance. Fixed/precolored point-interval analysis.
//!
//! This boundary resolves existing fixed constraints to exact half-open point
//! intervals. It does not choose a home, insert a copy, or split a live range.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::fixed_precolored_interval_plan_identity;
pub use model::*;
pub use validate::validate_fixed_precolored_intervals;

pub fn analyze_fixed_precolored_intervals(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    policy: FixedPrecoloredIntervalPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredIntervals, FixedPrecoloredIntervalError> {
    let plan = compute::compute(ranges, legality, policy, budget)?;
    validate_fixed_precolored_intervals(ranges, legality, plan)
}
