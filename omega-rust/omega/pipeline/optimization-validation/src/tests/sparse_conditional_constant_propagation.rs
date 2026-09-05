//! SCCP range and independent-evaluation tests.

use super::*;

#[test]
fn range_equality_rule_orientation_and_evaluation_are_independently_closed() {
    let operation = AbstractOperation::IntegerEqual {
        psi_operation: id(90_001, OperationId::new),
        result: id(90_002, ValueId::new),
        left: id(90_003, ValueId::new),
        right: id(90_004, ValueId::new),
    };
    let range_left = independently_validated_integer_range_comparison_kind(
        OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-equal-range-constant.v1",
        ),
        &operation,
    );
    let range_right = independently_validated_integer_range_comparison_kind(
        OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-equal-constant-range.v1",
        ),
        &operation,
    );
    assert_eq!(
        range_left,
        Some(ValidatedIntegerRangeComparisonKind::RangeEqualConstant)
    );
    assert_eq!(
        range_right,
        Some(ValidatedIntegerRangeComparisonKind::ConstantEqualRange)
    );
    assert_eq!(
        independently_validated_integer_range_comparison_kind(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.integer-less-than-range-constant.v1",
            ),
            &operation,
        ),
        None
    );

    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for kind in [range_left.unwrap(), range_right.unwrap()] {
        assert_eq!(
            independently_evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(7),
            ),
            Some(true)
        );
        assert_eq!(
            independently_evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(6),
            ),
            Some(false)
        );
        assert_eq!(
            independently_evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(10),
            ),
            Some(false)
        );
        assert_eq!(
            independently_evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(8),
            ),
            None
        );
    }
}

#[test]
fn range_pair_rules_reject_operator_and_rule_corruption() {
    let equal = AbstractOperation::IntegerEqual {
        psi_operation: id(91_001, OperationId::new),
        result: id(91_002, ValueId::new),
        left: id(91_003, ValueId::new),
        right: id(91_004, ValueId::new),
    };
    let less_than = AbstractOperation::IntegerLessThan {
        psi_operation: id(91_005, OperationId::new),
        result: id(91_006, ValueId::new),
        left: id(91_003, ValueId::new),
        right: id(91_004, ValueId::new),
    };
    let less_or_equal = AbstractOperation::IntegerLessOrEqual {
        psi_operation: id(91_007, OperationId::new),
        result: id(91_008, ValueId::new),
        left: id(91_003, ValueId::new),
        right: id(91_004, ValueId::new),
    };
    let equal_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.integer-equal-range-range.v1",
    );
    let less_than_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.integer-less-than-range-range.v1",
    );
    let less_or_equal_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.integer-less-or-equal-range-range.v1",
    );

    assert_eq!(
        independently_validated_integer_range_pair_comparison_kind(equal_rule, &equal),
        Some(ValidatedIntegerRangePairComparisonKind::Equal)
    );
    assert_eq!(
        independently_validated_integer_range_pair_comparison_kind(less_than_rule, &less_than,),
        Some(ValidatedIntegerRangePairComparisonKind::LessThan)
    );
    assert_eq!(
        independently_validated_integer_range_pair_comparison_kind(
            less_or_equal_rule,
            &less_or_equal,
        ),
        Some(ValidatedIntegerRangePairComparisonKind::LessOrEqual)
    );
    assert_eq!(
        independently_validated_integer_range_pair_comparison_kind(equal_rule, &less_than),
        None
    );
    assert_eq!(
        independently_validated_integer_range_pair_comparison_kind(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.integer-equal-range-constant.v1",
            ),
            &equal,
        ),
        None
    );
}

#[test]
fn range_pair_interval_evaluation_is_independently_closed() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let value = IntegerValue::Unsigned;
    let evaluate = |kind, left_minimum, left_maximum, right_minimum, right_maximum| {
        independently_evaluate_integer_range_pair_comparison(
            kind,
            scalar_type,
            false,
            value(left_minimum),
            value(left_maximum),
            value(right_minimum),
            value(right_maximum),
        )
    };

    assert_eq!(
        evaluate(ValidatedIntegerRangePairComparisonKind::Equal, 7, 7, 7, 7,),
        Some(true)
    );
    assert_eq!(
        evaluate(ValidatedIntegerRangePairComparisonKind::Equal, 1, 3, 5, 8,),
        Some(false)
    );
    assert_eq!(
        evaluate(ValidatedIntegerRangePairComparisonKind::Equal, 1, 5, 3, 8,),
        None
    );
    assert_eq!(
        evaluate(
            ValidatedIntegerRangePairComparisonKind::LessThan,
            1,
            3,
            5,
            8,
        ),
        Some(true)
    );
    assert_eq!(
        evaluate(
            ValidatedIntegerRangePairComparisonKind::LessThan,
            5,
            8,
            1,
            3,
        ),
        Some(false)
    );
    assert_eq!(
        evaluate(
            ValidatedIntegerRangePairComparisonKind::LessThan,
            1,
            5,
            3,
            8,
        ),
        None
    );
    assert_eq!(
        evaluate(
            ValidatedIntegerRangePairComparisonKind::LessOrEqual,
            1,
            5,
            5,
            8,
        ),
        Some(true)
    );
    assert_eq!(
        evaluate(
            ValidatedIntegerRangePairComparisonKind::LessOrEqual,
            5,
            8,
            1,
            3,
        ),
        Some(false)
    );
    assert_eq!(
        evaluate(
            ValidatedIntegerRangePairComparisonKind::LessOrEqual,
            1,
            6,
            5,
            8,
        ),
        None
    );
    for (kind, expected) in [
        (ValidatedIntegerRangePairComparisonKind::Equal, true),
        (ValidatedIntegerRangePairComparisonKind::LessThan, false),
        (ValidatedIntegerRangePairComparisonKind::LessOrEqual, true),
    ] {
        assert_eq!(
            independently_evaluate_integer_range_pair_comparison(
                kind,
                scalar_type,
                true,
                value(1),
                value(8),
                value(1),
                value(8),
            ),
            Some(expected)
        );
    }
}
