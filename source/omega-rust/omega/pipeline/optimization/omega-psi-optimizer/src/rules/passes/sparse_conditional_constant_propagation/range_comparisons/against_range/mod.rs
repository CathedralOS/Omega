//! Optimizer module role: stage group. Range-pair exact-rule map and shared contract join.

mod evaluation;
mod integer_equal_range_range;
mod integer_less_or_equal_range_range;
mod integer_less_than_range_range;
mod proposal;

#[cfg(test)]
pub(in crate::rules::passes) use evaluation::evaluate;
pub use integer_equal_range_range::IntegerEqualRangeRangeRule;
pub use integer_less_or_equal_range_range::IntegerLessOrEqualRangeRangeRule;
pub use integer_less_than_range_range::IntegerLessThanRangeRangeRule;

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};

use super::super::super::SCCP_PASS_NAME;
use super::model::IntegerRangePairComparisonKind;

fn contract(identity: &'static [u8]) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(identity),
        OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::ValueRanges]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        OptimizationSafetyClass::ProofCertified,
    )
    .expect("built-in rule has nonzero version")
}
