//! Optimizer module role: stage group. Range-against-constant exact-rule map and shared contract join.

mod evaluation;
mod integer_equal_constant_range;
mod integer_equal_range_constant;
mod integer_less_or_equal_constant_range;
mod integer_less_or_equal_range_constant;
mod integer_less_than_constant_range;
mod integer_less_than_range_constant;
mod proposal;

#[cfg(test)]
pub(in crate::rules::passes) use evaluation::evaluate;
pub use integer_equal_constant_range::IntegerEqualConstantRangeRule;
pub use integer_equal_range_constant::IntegerEqualRangeConstantRule;
pub use integer_less_or_equal_constant_range::IntegerLessOrEqualConstantRangeRule;
pub use integer_less_or_equal_range_constant::IntegerLessOrEqualRangeConstantRule;
pub use integer_less_than_constant_range::IntegerLessThanConstantRangeRule;
pub use integer_less_than_range_constant::IntegerLessThanRangeConstantRule;

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};

use super::super::super::SCCP_PASS_NAME;
use super::model::IntegerRangeComparisonKind;

fn contract(identity: &'static [u8]) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(identity),
        OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::ScalarConstants, AnalysisKind::ValueRanges]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        OptimizationSafetyClass::ProofCertified,
    )
    .expect("built-in rule has nonzero version")
}
