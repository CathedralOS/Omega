//! Operational matrix for SCCP proof-derived range rows 30--38.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

use super::custody::{Case, assert_operational_custody};
use crate::PsiOptimizationRule;
use crate::rules::tests::{
    IntegerRangeComparisonKind, IntegerRangePairComparisonKind, ProofRangeKind,
    range_constant_comparison_unit, range_pair_comparison_unit,
};
use crate::{
    IntegerEqualConstantRangeRule, IntegerEqualRangeConstantRule, IntegerEqualRangeRangeRule,
    IntegerLessOrEqualConstantRangeRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessOrEqualRangeRangeRule, IntegerLessThanConstantRangeRule,
    IntegerLessThanRangeConstantRule, IntegerLessThanRangeRangeRule,
};

fn rule(rule: impl PsiOptimizationRule) -> optimization_core::OptimizationRuleIdentity {
    rule.contract().identity()
}

#[test]
fn every_range_rule_has_whole_engine_operational_custody() {
    let u8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u = IntegerValue::Unsigned;

    assert_operational_custody(vec![
        Case::boolean(
            30,
            range_constant_comparison_unit(
                IntegerRangeComparisonKind::RangeLessThanConstant,
                u8,
                ProofRangeKind::ZeroToThree,
                u(4),
            ),
            rule(IntegerLessThanRangeConstantRule),
        ),
        Case::boolean(
            31,
            range_constant_comparison_unit(
                IntegerRangeComparisonKind::ConstantLessThanRange,
                u8,
                ProofRangeKind::Nonzero,
                u(0),
            ),
            rule(IntegerLessThanConstantRangeRule),
        ),
        Case::boolean(
            32,
            range_constant_comparison_unit(
                IntegerRangeComparisonKind::RangeLessOrEqualConstant,
                u8,
                ProofRangeKind::ZeroToThree,
                u(3),
            ),
            rule(IntegerLessOrEqualRangeConstantRule),
        ),
        Case::boolean(
            33,
            range_constant_comparison_unit(
                IntegerRangeComparisonKind::ConstantLessOrEqualRange,
                u8,
                ProofRangeKind::ZeroToThree,
                u(0),
            ),
            rule(IntegerLessOrEqualConstantRangeRule),
        ),
        Case::boolean(
            34,
            range_constant_comparison_unit(
                IntegerRangeComparisonKind::RangeEqualConstant,
                u8,
                ProofRangeKind::Zero,
                u(0),
            ),
            rule(IntegerEqualRangeConstantRule),
        ),
        Case::boolean(
            35,
            range_constant_comparison_unit(
                IntegerRangeComparisonKind::ConstantEqualRange,
                u8,
                ProofRangeKind::Zero,
                u(0),
            ),
            rule(IntegerEqualConstantRangeRule),
        ),
        Case::boolean(
            36,
            range_pair_comparison_unit(
                IntegerRangePairComparisonKind::Equal,
                u8,
                ProofRangeKind::Zero,
                ProofRangeKind::Zero,
                false,
            ),
            rule(IntegerEqualRangeRangeRule),
        ),
        Case::boolean(
            37,
            range_pair_comparison_unit(
                IntegerRangePairComparisonKind::LessThan,
                u8,
                ProofRangeKind::Zero,
                ProofRangeKind::Nonzero,
                false,
            ),
            rule(IntegerLessThanRangeRangeRule),
        ),
        Case::boolean(
            38,
            range_pair_comparison_unit(
                IntegerRangePairComparisonKind::LessOrEqual,
                u8,
                ProofRangeKind::Zero,
                ProofRangeKind::Zero,
                false,
            ),
            rule(IntegerLessOrEqualRangeRangeRule),
        ),
    ]);
}
