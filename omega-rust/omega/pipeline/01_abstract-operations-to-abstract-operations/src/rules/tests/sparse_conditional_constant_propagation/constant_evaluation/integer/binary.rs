//! Binary integer folds, exact policy semantics, and refusal cases.

use super::super::super::super::super::{
    ExactIntegerAddConstantsRule, ExactIntegerDivideConstantsRule,
    ExactIntegerMultiplyConstantsRule, ExactIntegerRemainderConstantsRule,
    ExactIntegerShiftLeftConstantsRule, ExactIntegerShiftRightConstantsRule,
    ExactIntegerSubtractConstantsRule, SaturatingIntegerDivideConstantsRule,
    SaturatingIntegerRemainderConstantsRule, WrappingIntegerDivideConstantsRule,
    WrappingIntegerRemainderConstantsRule,
};
use crate::rules::tests::fixtures::sparse_conditional_constant_propagation::{
    BinaryConstantFixtureKind, binary_constant_unit,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, compute_analysis};
use optimization_core::AnalysisKind;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

#[test]
fn proof_certified_binary_rules_decline_undefined_or_overflowing_constants() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let signed8 = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let unsigned16 = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let u = IntegerValue::Unsigned;
    let s = IntegerValue::Signed;
    let cases: [(
        BinaryConstantFixtureKind,
        &'static dyn PsiOptimizationRule,
        IntegerType,
        IntegerType,
        IntegerValue,
        IntegerValue,
    ); 13] = [
        (
            BinaryConstantFixtureKind::ExactAdd,
            &ExactIntegerAddConstantsRule,
            unsigned8,
            unsigned8,
            u(200),
            u(56),
        ),
        (
            BinaryConstantFixtureKind::ExactSubtract,
            &ExactIntegerSubtractConstantsRule,
            unsigned8,
            unsigned8,
            u(4),
            u(5),
        ),
        (
            BinaryConstantFixtureKind::ExactMultiply,
            &ExactIntegerMultiplyConstantsRule,
            unsigned8,
            unsigned8,
            u(52),
            u(5),
        ),
        (
            BinaryConstantFixtureKind::ExactDivide,
            &ExactIntegerDivideConstantsRule,
            unsigned8,
            unsigned8,
            u(20),
            u(0),
        ),
        (
            BinaryConstantFixtureKind::ExactRemainder,
            &ExactIntegerRemainderConstantsRule,
            unsigned8,
            unsigned8,
            u(20),
            u(0),
        ),
        (
            BinaryConstantFixtureKind::WrappingDivide,
            &WrappingIntegerDivideConstantsRule,
            unsigned8,
            unsigned8,
            u(20),
            u(0),
        ),
        (
            BinaryConstantFixtureKind::WrappingRemainder,
            &WrappingIntegerRemainderConstantsRule,
            unsigned8,
            unsigned8,
            u(20),
            u(0),
        ),
        (
            BinaryConstantFixtureKind::SaturatingDivide,
            &SaturatingIntegerDivideConstantsRule,
            unsigned8,
            unsigned8,
            u(20),
            u(0),
        ),
        (
            BinaryConstantFixtureKind::SaturatingRemainder,
            &SaturatingIntegerRemainderConstantsRule,
            unsigned8,
            unsigned8,
            u(20),
            u(0),
        ),
        (
            BinaryConstantFixtureKind::ExactDivide,
            &ExactIntegerDivideConstantsRule,
            signed8,
            signed8,
            s(-128),
            s(-1),
        ),
        (
            BinaryConstantFixtureKind::ExactRemainder,
            &ExactIntegerRemainderConstantsRule,
            signed8,
            signed8,
            s(-128),
            s(-1),
        ),
        (
            BinaryConstantFixtureKind::ExactShiftLeft,
            &ExactIntegerShiftLeftConstantsRule,
            unsigned8,
            unsigned16,
            u(250),
            u(2),
        ),
        (
            BinaryConstantFixtureKind::ExactShiftRight,
            &ExactIntegerShiftRightConstantsRule,
            unsigned8,
            unsigned16,
            u(7),
            u(8),
        ),
    ];
    for (kind, rule, value_type, count_type, left, right) in cases {
        let unit = binary_constant_unit(kind, value_type, count_type, left, right);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        assert!(
            rule.propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap()
                .is_empty(),
            "undefined or overflowing binary fold must be declined: {kind:?}",
        );
    }
}
