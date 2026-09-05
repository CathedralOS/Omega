//! Operational matrix for SCCP literal Boolean-result rows 25--29.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

use super::custody::{Case, assert_operational_custody};
use crate::PsiOptimizationRule;
use crate::rules::tests::{
    BooleanFixtureKind, ComparisonFixtureKind, boolean_constant_unit,
    integer_comparison_constant_unit,
};
use crate::{
    BooleanEqualConstantsRule, BooleanNotConstantsRule, IntegerEqualConstantsRule,
    IntegerLessOrEqualConstantsRule, IntegerLessThanConstantsRule,
};

fn rule(rule: impl PsiOptimizationRule) -> optimization_core::OptimizationRuleIdentity {
    rule.contract().identity()
}

#[test]
fn every_literal_boolean_result_rule_has_whole_engine_operational_custody() {
    let u8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u = IntegerValue::Unsigned;

    assert_operational_custody(vec![
        Case::boolean(
            25,
            boolean_constant_unit(BooleanFixtureKind::Not, true, false),
            rule(BooleanNotConstantsRule),
        ),
        Case::boolean(
            26,
            boolean_constant_unit(BooleanFixtureKind::Equal, true, false),
            rule(BooleanEqualConstantsRule),
        ),
        Case::boolean(
            27,
            integer_comparison_constant_unit(ComparisonFixtureKind::Equal, u8, u(0), u(255)),
            rule(IntegerEqualConstantsRule),
        ),
        Case::boolean(
            28,
            integer_comparison_constant_unit(ComparisonFixtureKind::LessThan, u8, u(0), u(255)),
            rule(IntegerLessThanConstantsRule),
        ),
        Case::boolean(
            29,
            integer_comparison_constant_unit(
                ComparisonFixtureKind::LessOrEqual,
                u8,
                u(255),
                u(255),
            ),
            rule(IntegerLessOrEqualConstantsRule),
        ),
    ]);
}
