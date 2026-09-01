//! Binary integer folds, exact policy semantics, and refusal cases.

use super::*;

struct BinarySuccessCase {
    kind: BinaryConstantFixtureKind,
    rule: &'static dyn PsiOptimizationRule,
    value_type: IntegerType,
    count_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
    expected: IntegerValue,
}

fn success(
    kind: BinaryConstantFixtureKind,
    rule: &'static dyn PsiOptimizationRule,
    value_type: IntegerType,
    count_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
    expected: IntegerValue,
) -> BinarySuccessCase {
    BinarySuccessCase {
        kind,
        rule,
        value_type,
        count_type,
        left,
        right,
        expected,
    }
}

#[test]
fn every_binary_integer_rule_folds_its_exact_declared_semantics() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let signed8 = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let unsigned16 = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let u = IntegerValue::Unsigned;
    let s = IntegerValue::Signed;
    let cases = [
        success(
            BinaryConstantFixtureKind::ExactAdd,
            &ExactIntegerAddConstantsRule,
            unsigned8,
            unsigned8,
            u(200),
            u(55),
            u(255),
        ),
        success(
            BinaryConstantFixtureKind::ExactSubtract,
            &ExactIntegerSubtractConstantsRule,
            unsigned8,
            unsigned8,
            u(5),
            u(5),
            u(0),
        ),
        success(
            BinaryConstantFixtureKind::ExactMultiply,
            &ExactIntegerMultiplyConstantsRule,
            unsigned8,
            unsigned8,
            u(51),
            u(5),
            u(255),
        ),
        success(
            BinaryConstantFixtureKind::WrappingAdd,
            &WrappingIntegerAddConstantsRule,
            unsigned8,
            unsigned8,
            u(200),
            u(100),
            u(44),
        ),
        success(
            BinaryConstantFixtureKind::WrappingSubtract,
            &WrappingIntegerSubtractConstantsRule,
            unsigned8,
            unsigned8,
            u(5),
            u(10),
            u(251),
        ),
        success(
            BinaryConstantFixtureKind::WrappingMultiply,
            &WrappingIntegerMultiplyConstantsRule,
            unsigned8,
            unsigned8,
            u(20),
            u(13),
            u(4),
        ),
        success(
            BinaryConstantFixtureKind::SaturatingAdd,
            &SaturatingIntegerAddConstantsRule,
            unsigned8,
            unsigned8,
            u(200),
            u(100),
            u(255),
        ),
        success(
            BinaryConstantFixtureKind::SaturatingSubtract,
            &SaturatingIntegerSubtractConstantsRule,
            unsigned8,
            unsigned8,
            u(5),
            u(10),
            u(0),
        ),
        success(
            BinaryConstantFixtureKind::SaturatingMultiply,
            &SaturatingIntegerMultiplyConstantsRule,
            unsigned8,
            unsigned8,
            u(20),
            u(13),
            u(255),
        ),
        success(
            BinaryConstantFixtureKind::ExactDivide,
            &ExactIntegerDivideConstantsRule,
            signed8,
            signed8,
            s(-127),
            s(-1),
            s(127),
        ),
        success(
            BinaryConstantFixtureKind::ExactRemainder,
            &ExactIntegerRemainderConstantsRule,
            signed8,
            signed8,
            s(-127),
            s(5),
            s(-2),
        ),
        success(
            BinaryConstantFixtureKind::WrappingDivide,
            &WrappingIntegerDivideConstantsRule,
            signed8,
            signed8,
            s(-128),
            s(-1),
            s(-128),
        ),
        success(
            BinaryConstantFixtureKind::WrappingRemainder,
            &WrappingIntegerRemainderConstantsRule,
            signed8,
            signed8,
            s(-128),
            s(-1),
            s(0),
        ),
        success(
            BinaryConstantFixtureKind::SaturatingDivide,
            &SaturatingIntegerDivideConstantsRule,
            signed8,
            signed8,
            s(-128),
            s(-1),
            s(127),
        ),
        success(
            BinaryConstantFixtureKind::SaturatingRemainder,
            &SaturatingIntegerRemainderConstantsRule,
            signed8,
            signed8,
            s(-128),
            s(-1),
            s(0),
        ),
        success(
            BinaryConstantFixtureKind::ExactShiftLeft,
            &ExactIntegerShiftLeftConstantsRule,
            unsigned8,
            unsigned16,
            u(7),
            u(2),
            u(28),
        ),
        success(
            BinaryConstantFixtureKind::ExactShiftRight,
            &ExactIntegerShiftRightConstantsRule,
            signed8,
            unsigned16,
            s(-128),
            u(2),
            s(-32),
        ),
        success(
            BinaryConstantFixtureKind::WrappingShiftLeft,
            &WrappingIntegerShiftLeftConstantsRule,
            unsigned8,
            unsigned16,
            u(250),
            u(10),
            u(232),
        ),
        success(
            BinaryConstantFixtureKind::WrappingShiftRight,
            &WrappingIntegerShiftRightConstantsRule,
            signed8,
            unsigned16,
            s(-8),
            u(10),
            s(-2),
        ),
        success(
            BinaryConstantFixtureKind::BitwiseAnd,
            &IntegerBitwiseAndConstantsRule,
            unsigned8,
            unsigned8,
            u(0b1010),
            u(0b1100),
            u(8),
        ),
        success(
            BinaryConstantFixtureKind::BitwiseOr,
            &IntegerBitwiseOrConstantsRule,
            unsigned8,
            unsigned8,
            u(0b1010),
            u(0b1100),
            u(14),
        ),
        success(
            BinaryConstantFixtureKind::BitwiseXor,
            &IntegerBitwiseXorConstantsRule,
            unsigned8,
            unsigned8,
            u(0b1010),
            u(0b1100),
            u(6),
        ),
    ];
    for case in cases {
        let unit = binary_constant_unit(
            case.kind,
            case.value_type,
            case.count_type,
            case.left,
            case.right,
        );
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = case
            .rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1, "exact binary entrance {:?}", case.kind);
        let expected_safety = if case.kind.proof_certified() {
            OptimizationSafetyClass::ProofCertified
        } else {
            OptimizationSafetyClass::ExactOperationSemantics
        };
        assert_eq!(
            candidates[0].safety_class(),
            expected_safety,
            "{:?}",
            case.kind
        );
        assert_eq!(
            matches!(
                candidates[0].scalar_evaluation_witness().unwrap(),
                IntegerEvaluationWitness::ProofCertifiedBinary { .. }
            ),
            case.kind.proof_certified(),
            "binary witness class for {:?}",
            case.kind,
        );
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(
            matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                AbstractOperation::IntegerConstant {
                    scalar_type: ScalarType::Integer(actual_type),
                    value,
                    ..
                } if actual_type == case.value_type && value == case.expected
            ),
            "validated binary result for {:?}",
            case.kind,
        );
    }
}

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

#[test]
fn wrapping_and_saturating_rules_use_their_exact_declared_policies() {
    for (unit, saturating) in [(wrapping_add_unit(), false), (policy_add_unit(true), true)] {
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let products = vec![constants];
        let candidates = if saturating {
            SaturatingIntegerAddConstantsRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
        } else {
            WrappingIntegerAddConstantsRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
        };
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].safety_class(),
            OptimizationSafetyClass::ExactOperationSemantics
        );
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        let expected = if saturating { 255 } else { 4 };
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(value),
                ..
            } if value == expected
        ));
    }
}

#[test]
fn binary_bitwise_rules_fold_with_typed_psi_semantics() {
    let cases: [(BitwiseFixtureKind, &dyn PsiOptimizationRule, u128); 3] = [
        (BitwiseFixtureKind::And, &IntegerBitwiseAndConstantsRule, 8),
        (BitwiseFixtureKind::Or, &IntegerBitwiseOrConstantsRule, 14),
        (BitwiseFixtureKind::Xor, &IntegerBitwiseXorConstantsRule, 6),
    ];
    for (kind, rule, expected) in cases {
        let unit = bitwise_unit(kind);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].safety_class(),
            OptimizationSafetyClass::ExactOperationSemantics
        );
        assert!(matches!(
            candidates[0].scalar_evaluation_witness().unwrap(),
            IntegerEvaluationWitness::Binary { .. }
        ));
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(value),
                ..
            } if value == expected
        ));
    }
}

#[test]
fn proof_bearing_division_folds_only_when_the_declared_operation_is_defined() {
    let unit = exact_divide_unit(false);
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    let candidates = ExactIntegerDivideConstantsRule
        .propose(&unit, RuleAnalysisView::new(&[constants]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(0),
            ..
        }
    ));

    let zero = exact_divide_unit(true);
    let constants = compute_analysis(&zero, AnalysisKind::ScalarConstants).unwrap();
    assert!(
        ExactIntegerDivideConstantsRule
            .propose(&zero, RuleAnalysisView::new(&[constants]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn exact_and_wrapping_shift_rules_use_psi_integer_semantics() {
    let cases: [(ShiftFixtureKind, &dyn PsiOptimizationRule, u128, u128, u128); 4] = [
        (
            ShiftFixtureKind::ExactLeft,
            &ExactIntegerShiftLeftConstantsRule,
            7,
            2,
            28,
        ),
        (
            ShiftFixtureKind::ExactRight,
            &ExactIntegerShiftRightConstantsRule,
            7,
            2,
            1,
        ),
        (
            ShiftFixtureKind::WrappingLeft,
            &WrappingIntegerShiftLeftConstantsRule,
            250,
            2,
            232,
        ),
        (
            ShiftFixtureKind::WrappingRight,
            &WrappingIntegerShiftRightConstantsRule,
            250,
            2,
            62,
        ),
    ];
    for (kind, rule, value, count, expected) in cases {
        let unit = shift_unit(kind, value, count);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let expected_safety = if matches!(
            kind,
            ShiftFixtureKind::ExactLeft | ShiftFixtureKind::ExactRight
        ) {
            OptimizationSafetyClass::ProofCertified
        } else {
            OptimizationSafetyClass::ExactOperationSemantics
        };
        assert_eq!(candidates[0].safety_class(), expected_safety);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(value),
                ..
            } if value == expected
        ));
    }
}

#[test]
fn exact_shift_left_declines_an_overflowing_constant_evaluation() {
    let unit = shift_unit(ShiftFixtureKind::ExactLeft, 250, 2);
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    assert!(
        ExactIntegerShiftLeftConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap()
            .is_empty()
    );
}
