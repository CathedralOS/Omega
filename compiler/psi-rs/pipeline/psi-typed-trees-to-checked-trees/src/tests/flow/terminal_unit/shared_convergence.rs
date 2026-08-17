use super::*;
use crate::flow::{
    exact_affine_cast_affine_runtime_parameter_positions_for_test,
    exact_affine_chain_cast_runtime_parameter_positions_for_test,
    exact_affine_chain_runtime_parameter_positions_for_test,
    exact_affine_fork_join_runtime_parameter_positions_for_test,
    exact_affine_shift_cast_sandwich_runtime_parameter_positions_for_test,
    exact_arithmetic_then_shift_runtime_parameter_positions_for_test,
    exact_cast_chain_runtime_parameter_positions_for_test,
    exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test,
    exact_cast_then_affine_runtime_parameter_positions_for_test,
    exact_cast_then_divide_remainder_runtime_parameter_positions_for_test,
    exact_cast_then_mixed_shift_runtime_parameter_positions_for_test,
    exact_cast_then_multiply_runtime_parameter_positions_for_test,
    exact_cast_then_offset_runtime_parameter_positions_for_test,
    exact_cast_then_shift_left_runtime_parameter_positions_for_test,
    exact_cast_then_shift_right_runtime_parameter_positions_for_test,
    exact_cast_then_signed_affine_runtime_parameter_positions_for_test,
    exact_cast_then_signed_multiply_runtime_parameter_positions_for_test,
    exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test,
    exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test,
    exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameter_positions_for_test,
    exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameter_positions_for_test,
    exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test,
    exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test,
    exact_divide_remainder_cast_sandwich_runtime_parameter_positions_for_test,
    exact_divide_remainder_chain_cast_runtime_parameter_positions_for_test,
    exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test,
    exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test,
    exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test,
    exact_mixed_shift_chain_cast_runtime_parameter_positions_for_test,
    exact_mixed_shift_chain_runtime_parameter_positions_for_test,
    exact_multiply_chain_cast_runtime_parameter_positions_for_test,
    exact_offset_chain_cast_runtime_parameter_positions_for_test,
    exact_runtime_divisor_chain_runtime_parameter_positions_for_test,
    exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test,
    exact_same_root_affine_product_join_runtime_parameter_positions_for_test,
    exact_shift_cast_shift_runtime_parameter_positions_for_test,
    exact_shift_left_chain_cast_runtime_parameter_positions_for_test,
    exact_shift_left_chain_runtime_parameter_positions_for_test,
    exact_shift_right_chain_cast_runtime_parameter_positions_for_test,
    exact_shift_then_arithmetic_runtime_parameter_positions_for_test,
    exact_signed_affine_cast_affine_runtime_parameter_positions_for_test,
    exact_signed_affine_chain_cast_runtime_parameter_positions_for_test,
    exact_signed_affine_chain_runtime_parameter_positions_for_test,
    exact_signed_multiply_chain_cast_runtime_parameter_positions_for_test,
    exact_signed_multiply_chain_runtime_parameter_positions_for_test,
};
use psi_checked_trees::{
    CheckedIntegerBinaryKind, CheckedScalarExpression, CheckedScalarExpressionRole,
};

#[test]
fn exact_shift_left_chain_classifier_covers_u64_and_fences_other_exact_roots() {
    let count = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U64,
    };
    let shift_left = |value, count| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftLeft,
        primitive_type: PrimitiveType::U64,
        left: Box::new(value),
        right: Box::new(count),
    };
    let u64_chain = shift_left(
        shift_left(
            shift_left(
                parameter(),
                count(1i64, psi_numerics::literals::LandedIntegerType::U8),
            ),
            count(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        count(3i64, psi_numerics::literals::LandedIntegerType::U32),
    );
    assert_eq!(
        exact_shift_left_chain_runtime_parameter_positions_for_test(&u64_chain, 1),
        Some(vec![0])
    );

    let shifted_root = CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftRight,
        primitive_type: PrimitiveType::U64,
        left: Box::new(parameter()),
        right: Box::new(count(1i64, psi_numerics::literals::LandedIntegerType::U8)),
    };
    let fenced = shift_left(
        shift_left(
            shifted_root,
            count(0i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        count(0i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_shift_left_chain_runtime_parameter_positions_for_test(&fenced, 1),
        None
    );
}

#[test]
fn mixed_exact_add_subtract_chain_classifier_is_left_associated_and_same_carrier() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U64,
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U64,
        left: Box::new(left),
        right: Box::new(right),
    };
    let mixed = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                parameter(),
                literal(5i64, psi_numerics::literals::LandedIntegerType::U64),
            ),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&mixed, 1),
        Some(vec![0])
    );

    let right_associated = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&right_associated, 1),
        None
    );

    let mismatched_literal = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I64),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&mismatched_literal, 1,),
        None
    );

    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
        parameter(),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&runtime_sibling, 1),
        None
    );

    let all_add = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&all_add, 1),
        None
    );
}

#[test]
fn offset_chain_exact_cast_classifier_requires_one_direct_same_carrier_left_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let mixed = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U16,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(5i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::U8, &mixed, 1,),
        Some(vec![0])
    );
    let one_add = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        parameter(0, PrimitiveType::I8),
        literal(-1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &one_add,
            1,
        ),
        Some(vec![0])
    );

    let right_associated = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U16,
        literal(5i64, psi_numerics::literals::LandedIntegerType::U16),
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &right_associated,
            1,
        ),
        None
    );
    let mismatched_literal = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &mismatched_literal,
            1,
        ),
        None
    );
    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        parameter(0, PrimitiveType::U16),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &runtime_sibling,
            1,
        ),
        None
    );
    let local_root = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U16,
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U16,
        },
        literal(1i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &local_root,
            1,
        ),
        None
    );
}

#[test]
fn multiply_chain_exact_cast_classifier_requires_one_direct_same_carrier_left_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let operation = |primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactMultiply,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let finite = operation(
        PrimitiveType::U16,
        operation(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &finite,
            1,
        ),
        Some(vec![0])
    );
    let zero = operation(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        literal(0i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::I8, &zero, 1,),
        Some(vec![0])
    );
    let signed = operation(
        PrimitiveType::I16,
        parameter(0, PrimitiveType::I16),
        literal(2i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &signed,
            1,
        ),
        Some(vec![0])
    );

    let reversed = operation(
        PrimitiveType::U16,
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        parameter(0, PrimitiveType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &reversed,
            1,
        ),
        None
    );
    let runtime_sibling = operation(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        parameter(0, PrimitiveType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &runtime_sibling,
            1,
        ),
        None
    );
    let negative = operation(
        PrimitiveType::I16,
        parameter(0, PrimitiveType::I16),
        literal(-1i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &negative,
            1,
        ),
        None
    );
    let mixed = operation(
        PrimitiveType::U16,
        operation(
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &mixed,
            1,
        ),
        None
    );
    let right_associated = operation(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        operation(
            PrimitiveType::U16,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &right_associated,
            1,
        ),
        None
    );
    let local_root = operation(
        PrimitiveType::U16,
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U16,
        },
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &local_root,
            1,
        ),
        None
    );
}

#[test]
fn exact_cast_then_offset_classifier_accepts_one_finite_left_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let operation = |kind, target_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: target_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    for kind in [
        CheckedIntegerBinaryKind::ExactAdd,
        CheckedIntegerBinaryKind::ExactSubtract,
    ] {
        let accepted = operation(
            kind,
            PrimitiveType::U8,
            cast(PrimitiveType::U16, PrimitiveType::U8, 0),
            literal(5i64, psi_numerics::literals::LandedIntegerType::U8),
        );
        assert_eq!(
            exact_cast_then_offset_runtime_parameter_positions_for_test(&accepted, 1),
            Some(vec![0])
        );
    }
    let cross_sign = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        cast(PrimitiveType::I8, PrimitiveType::U8, 0),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&cross_sign, 1),
        Some(vec![0])
    );

    let reversed_add = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        literal(5i64, psi_numerics::literals::LandedIntegerType::U8),
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&reversed_add, 1),
        None
    );
    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U8,
        },
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&runtime_sibling, 1),
        None
    );
    let mismatched_target = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&mismatched_target, 1),
        None
    );
    let nested = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U8,
            cast(PrimitiveType::U16, PrimitiveType::U8, 0),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&nested, 1),
        Some(vec![0])
    );
    let deep_mixed = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U8,
        nested,
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&deep_mixed, 1),
        Some(vec![0])
    );
    let right_associated = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U8,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&right_associated, 1),
        None
    );
}

#[test]
fn exact_cast_then_multiply_classifier_accepts_one_finite_left_nonnegative_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let multiply = |target_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactMultiply,
        primitive_type: target_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let accepted = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0])
    );
    let finite_with_zero = multiply(
        PrimitiveType::U8,
        multiply(
            PrimitiveType::U8,
            accepted,
            literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(0i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&finite_with_zero, 1),
        Some(vec![0])
    );
    let signed = multiply(
        PrimitiveType::I8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(2i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&signed, 1),
        Some(vec![0])
    );

    let reversed = multiply(
        PrimitiveType::U8,
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&reversed, 1),
        None
    );
    let runtime_sibling = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U8,
        },
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&runtime_sibling, 1),
        None
    );
    let negative = multiply(
        PrimitiveType::I8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(-1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&negative, 1),
        None
    );
    let mismatched = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&mismatched, 1),
        None
    );
    let right_associated = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        multiply(
            PrimitiveType::U8,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&right_associated, 1),
        None
    );
}

#[test]
fn signed_multiply_classifiers_accept_checked_negative_products_in_all_three_placements() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let multiply = |primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactMultiply,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let cast = |source_type, target_type| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(parameter(source_type)),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };

    let direct = multiply(
        PrimitiveType::I8,
        multiply(
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(-2i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(-3i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_signed_multiply_chain_runtime_parameter_positions_for_test(&direct, 1),
        Some(vec![0]),
    );
    let pre_cast = multiply(
        PrimitiveType::I16,
        parameter(PrimitiveType::I16),
        literal(-2i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_signed_multiply_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &pre_cast,
            1,
        ),
        Some(vec![0]),
    );
    let post_cast = multiply(
        PrimitiveType::I8,
        multiply(
            PrimitiveType::I8,
            cast(PrimitiveType::I16, PrimitiveType::I8),
            literal(-2i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(0i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_signed_multiply_runtime_parameter_positions_for_test(&post_cast, 1),
        Some(vec![0]),
    );

    let nonnegative = multiply(
        PrimitiveType::I8,
        multiply(
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(2i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_signed_multiply_chain_runtime_parameter_positions_for_test(&nonnegative, 1),
        None,
    );
    let overflow = multiply(
        PrimitiveType::I64,
        multiply(
            PrimitiveType::I64,
            multiply(
                PrimitiveType::I64,
                parameter(PrimitiveType::I64),
                literal(i64::MIN, psi_numerics::literals::LandedIntegerType::I64),
            ),
            literal(i64::MIN, psi_numerics::literals::LandedIntegerType::I64),
        ),
        literal(4i64, psi_numerics::literals::LandedIntegerType::I64),
    );
    assert_eq!(
        exact_signed_multiply_chain_runtime_parameter_positions_for_test(&overflow, 1),
        None,
    );
    let zero_before_large_factors = multiply(
        PrimitiveType::I64,
        multiply(
            PrimitiveType::I64,
            multiply(
                PrimitiveType::I64,
                multiply(
                    PrimitiveType::I64,
                    parameter(PrimitiveType::I64),
                    literal(0i64, psi_numerics::literals::LandedIntegerType::I64),
                ),
                literal(i64::MIN, psi_numerics::literals::LandedIntegerType::I64),
            ),
            literal(i64::MIN, psi_numerics::literals::LandedIntegerType::I64),
        ),
        literal(4i64, psi_numerics::literals::LandedIntegerType::I64),
    );
    assert_eq!(
        exact_signed_multiply_chain_runtime_parameter_positions_for_test(
            &zero_before_large_factors,
            1,
        ),
        Some(vec![0]),
        "an earlier executed zero resets a reversed-walk product overflow",
    );
}

#[test]
fn signed_affine_classifiers_cover_direct_pre_cast_and_post_cast_without_widening_narrower_paths() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let cast = |source_type, target_type| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(parameter(source_type)),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let i8_literal = |value| literal(value, psi_numerics::literals::LandedIntegerType::I8);
    let signed_affine = |root| {
        binary(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::I8,
            binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I8,
                binary(
                    CheckedIntegerBinaryKind::ExactAdd,
                    PrimitiveType::I8,
                    root,
                    i8_literal(3),
                ),
                i8_literal(-2),
            ),
            i8_literal(1),
        )
    };

    let direct = signed_affine(parameter(PrimitiveType::I8));
    assert_eq!(
        exact_signed_affine_chain_runtime_parameter_positions_for_test(&direct, 1),
        Some(vec![0]),
    );
    assert_eq!(
        exact_signed_affine_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &direct,
            1,
        ),
        Some(vec![0]),
    );
    let post_cast = signed_affine(cast(PrimitiveType::I16, PrimitiveType::I8));
    assert_eq!(
        exact_cast_then_signed_affine_runtime_parameter_positions_for_test(&post_cast, 1),
        Some(vec![0]),
    );

    let homogeneous_product = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I8,
        binary(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            i8_literal(-2),
        ),
        i8_literal(3),
    );
    assert_eq!(
        exact_signed_affine_chain_runtime_parameter_positions_for_test(&homogeneous_product, 1),
        None,
        "the homogeneous signed-product path keeps priority",
    );
    let nonnegative_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I8,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            i8_literal(1),
        ),
        i8_literal(2),
    );
    assert_eq!(
        exact_signed_affine_chain_runtime_parameter_positions_for_test(&nonnegative_affine, 1),
        None,
        "the established nonnegative-affine path stays distinct",
    );

    let i64_literal = |value| literal(value, psi_numerics::literals::LandedIntegerType::I64);
    let overflow = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I64,
        binary(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I64,
            binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I64,
                binary(
                    CheckedIntegerBinaryKind::ExactAdd,
                    PrimitiveType::I64,
                    parameter(PrimitiveType::I64),
                    i64_literal(1),
                ),
                i64_literal(i64::MIN),
            ),
            i64_literal(i64::MIN),
        ),
        i64_literal(4),
    );
    assert_eq!(
        exact_signed_affine_chain_runtime_parameter_positions_for_test(&overflow, 1),
        None,
        "checked coefficient overflow is not admitted",
    );
}

#[test]
fn exact_cast_chain_classifier_accepts_only_ordered_partial_native_casts() {
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let cast = |target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(operand),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let chain = cast(
        PrimitiveType::I32,
        cast(PrimitiveType::U64, parameter(PrimitiveType::I64)),
    );
    assert_eq!(
        exact_cast_chain_runtime_parameter_positions_for_test(PrimitiveType::U8, &chain, 1),
        Some(vec![0]),
        "heterogeneous cross-sign partial casts retain the direct root",
    );
    assert_eq!(
        exact_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::U64,
            &parameter(PrimitiveType::I64),
            1,
        ),
        None,
        "the first direct cast stays on its existing path",
    );

    let widening_inner = cast(PrimitiveType::I16, parameter(PrimitiveType::U8));
    assert_eq!(
        exact_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &widening_inner,
            1,
        ),
        None,
        "a widening edge is not reclassified as an exact-cast chain",
    );
    let intervening = CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactAdd,
        primitive_type: PrimitiveType::I32,
        left: Box::new(chain),
        right: Box::new(parameter(PrimitiveType::I32)),
    };
    assert_eq!(
        exact_cast_chain_runtime_parameter_positions_for_test(PrimitiveType::U8, &intervening, 1,),
        None,
        "intervening arithmetic remains outside the cast-only chain",
    );
    let local_root = cast(
        PrimitiveType::I32,
        cast(
            PrimitiveType::U64,
            CheckedScalarExpression::Local {
                position: 0,
                primitive_type: PrimitiveType::I64,
            },
        ),
    );
    assert_eq!(
        exact_cast_chain_runtime_parameter_positions_for_test(PrimitiveType::U8, &local_root, 1,),
        None,
        "local roots remain fenced",
    );
}

#[test]
fn computed_prefix_cast_chain_classifier_reuses_only_existing_pre_cast_families() {
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let local = |primitive_type| CheckedScalarExpression::Local {
        position: 0,
        primitive_type,
    };
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let cast = |target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(operand),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let wrap = |source| cast(PrimitiveType::I32, cast(PrimitiveType::U64, source));

    let affine = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I64,
        binary(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I64,
            parameter(PrimitiveType::I64),
            literal(2i64, psi_numerics::literals::LandedIntegerType::I64),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I64),
    );
    assert_eq!(
        exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &wrap(affine.clone()),
            1,
        ),
        Some(vec![0]),
    );
    let signed_product = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I64,
        parameter(PrimitiveType::I64),
        literal(-2i64, psi_numerics::literals::LandedIntegerType::I64),
    );
    assert_eq!(
        exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &wrap(signed_product),
            1,
        ),
        Some(vec![0]),
    );
    let shifts = binary(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::I64,
        binary(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::I64,
            parameter(PrimitiveType::I64),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &wrap(shifts),
            1,
        ),
        Some(vec![0]),
    );
    let divide_remainder = binary(
        CheckedIntegerBinaryKind::ExactRemainder,
        PrimitiveType::U64,
        binary(
            CheckedIntegerBinaryKind::ExactDivide,
            PrimitiveType::U64,
            parameter(PrimitiveType::U64),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    let unsigned_chain = cast(
        PrimitiveType::U32,
        cast(PrimitiveType::I64, divide_remainder),
    );
    assert_eq!(
        exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::I16,
            &unsigned_chain,
            1,
        ),
        Some(vec![0]),
    );

    assert_eq!(
        exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::U64,
            &affine,
            1,
        ),
        None,
        "one computed cast remains on its established path",
    );
    let widened_source = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        parameter(PrimitiveType::U8),
        literal(0i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    let widening_first_edge = cast(PrimitiveType::U8, cast(PrimitiveType::I16, widened_source));
    assert_eq!(
        exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &widening_first_edge,
            1,
        ),
        None,
        "the innermost computed cast must also be partial",
    );
    let local_affine = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I64,
        local(PrimitiveType::I64),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I64),
    );
    assert_eq!(
        exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &wrap(local_affine),
            1,
        ),
        None,
        "local roots remain fenced",
    );
}

#[test]
fn cast_chain_then_computed_suffix_classifier_reuses_only_existing_post_cast_families() {
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let local = |primitive_type| CheckedScalarExpression::Local {
        position: 0,
        primitive_type,
    };
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let cast = |target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(operand),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let signed_chain = || {
        cast(
            PrimitiveType::I32,
            cast(PrimitiveType::U64, parameter(PrimitiveType::I64)),
        )
    };

    let affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I32,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I32,
            signed_chain(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(&affine, 1),
        Some(vec![0]),
    );

    let signed_product = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I32,
        signed_chain(),
        literal(-2i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(
            &signed_product,
            1,
        ),
        Some(vec![0]),
    );

    let shifts = binary(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::I32,
        binary(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::I32,
            signed_chain(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(&shifts, 1),
        Some(vec![0]),
    );

    let unsigned_chain = cast(
        PrimitiveType::U8,
        cast(PrimitiveType::I8, parameter(PrimitiveType::U32)),
    );
    let divide_remainder = binary(
        CheckedIntegerBinaryKind::ExactRemainder,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactDivide,
            PrimitiveType::U8,
            unsigned_chain,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(
            &divide_remainder,
            1,
        ),
        Some(vec![0]),
    );

    let one_cast = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I32,
        cast(PrimitiveType::I32, parameter(PrimitiveType::I64)),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(&one_cast, 1),
        None,
        "one cast remains on its established post-cast path",
    );

    let widening_edge = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I32,
        cast(
            PrimitiveType::I32,
            cast(PrimitiveType::I16, parameter(PrimitiveType::I8)),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(
            &widening_edge,
            1,
        ),
        None,
        "every cast-chain edge remains partial",
    );

    let local_root = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I32,
        cast(
            PrimitiveType::I32,
            cast(PrimitiveType::U64, local(PrimitiveType::I64)),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(&local_root, 1),
        None,
        "local roots remain fenced",
    );

    let cross_family = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I32,
        binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::I32,
            signed_chain(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(
            &cross_family,
            1,
        ),
        None,
        "cross-family suffixes remain fenced",
    );
}

#[test]
fn computed_prefix_cast_chain_computed_suffix_classifier_covers_the_four_by_four_matrix() {
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let cast = |target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(operand),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let signed_cast_chain = |source| cast(PrimitiveType::I32, cast(PrimitiveType::U64, source));
    let small_signed_cast_chain = |source| cast(PrimitiveType::I8, cast(PrimitiveType::U8, source));
    let sources = vec![
        (
            PrimitiveType::I32,
            signed_cast_chain(binary(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I64,
                parameter(PrimitiveType::I64),
                literal(1i64, psi_numerics::literals::LandedIntegerType::I64),
            )),
        ),
        (
            PrimitiveType::I32,
            signed_cast_chain(binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I64,
                parameter(PrimitiveType::I64),
                literal(-2i64, psi_numerics::literals::LandedIntegerType::I64),
            )),
        ),
        (
            PrimitiveType::I32,
            signed_cast_chain(binary(
                CheckedIntegerBinaryKind::ExactShiftRight,
                PrimitiveType::I64,
                parameter(PrimitiveType::I64),
                literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            )),
        ),
        (
            PrimitiveType::I8,
            small_signed_cast_chain(binary(
                CheckedIntegerBinaryKind::ExactRemainder,
                PrimitiveType::U32,
                parameter(PrimitiveType::U32),
                literal(3i64, psi_numerics::literals::LandedIntegerType::U32),
            )),
        ),
    ];
    for (target_type, source) in sources {
        let landed_type = match target_type {
            PrimitiveType::I32 => psi_numerics::literals::LandedIntegerType::I32,
            PrimitiveType::I8 => psi_numerics::literals::LandedIntegerType::I8,
            _ => unreachable!("fixture uses signed target carriers"),
        };
        let count_type = psi_numerics::literals::LandedIntegerType::U8;
        let targets = [
            binary(
                CheckedIntegerBinaryKind::ExactAdd,
                target_type,
                source.clone(),
                literal(1i64, landed_type),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                target_type,
                source.clone(),
                literal(-2i64, landed_type),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactShiftLeft,
                target_type,
                source.clone(),
                literal(1i64, count_type),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactDivide,
                target_type,
                source,
                literal(2i64, landed_type),
            ),
        ];
        for target in targets {
            assert_eq!(
                exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(
                    &target, 1,
                ),
                Some(vec![0]),
            );
        }
    }

    let one_cast = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I32,
        cast(
            PrimitiveType::I32,
            binary(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I64,
                parameter(PrimitiveType::I64),
                literal(1i64, psi_numerics::literals::LandedIntegerType::I64),
            ),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(
            &one_cast, 1,
        ),
        None,
        "one cast remains on its established sandwich paths",
    );
}

#[test]
fn computed_prefix_widen_chain_computed_suffix_classifier_covers_the_four_by_four_matrix() {
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let widen = |target_type, operand| CheckedScalarExpression::IntegerWiden {
        primitive_type: target_type,
        operand: Box::new(operand),
    };
    let signed_widen_chain = |source| widen(PrimitiveType::I32, widen(PrimitiveType::I16, source));
    let unsigned_widen_chain =
        |source| widen(PrimitiveType::I32, widen(PrimitiveType::I16, source));
    let sources = [
        signed_widen_chain(binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        )),
        signed_widen_chain(binary(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(-2i64, psi_numerics::literals::LandedIntegerType::I8),
        )),
        signed_widen_chain(binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        )),
        unsigned_widen_chain(binary(
            CheckedIntegerBinaryKind::ExactRemainder,
            PrimitiveType::U8,
            parameter(PrimitiveType::U8),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
        )),
    ];
    for source in sources {
        let targets = [
            binary(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I32,
                source.clone(),
                literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I32,
                source.clone(),
                literal(-2i64, psi_numerics::literals::LandedIntegerType::I32),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactShiftLeft,
                PrimitiveType::I32,
                source.clone(),
                literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactDivide,
                PrimitiveType::I32,
                source,
                literal(2i64, psi_numerics::literals::LandedIntegerType::I32),
            ),
        ];
        for target in targets {
            assert_eq!(
                exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameter_positions_for_test(
                    &target, 1,
                ),
                Some(vec![0]),
            );
        }
    }

    let missing_widen = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameter_positions_for_test(
            &missing_widen,
            1,
        ),
        None,
        "direct compositions retain their narrower paths",
    );

    let invalid_widen = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U32,
        widen(
            PrimitiveType::U32,
            binary(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
            ),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U32),
    );
    assert_eq!(
        exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameter_positions_for_test(
            &invalid_widen,
            1,
        ),
        None,
        "signed-to-unsigned conversion is not a valid widening edge",
    );
}

#[test]
fn computed_prefix_mixed_conversion_chain_computed_suffix_classifier_covers_the_four_by_four_matrix()
 {
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let widen = |target_type, operand| CheckedScalarExpression::IntegerWiden {
        primitive_type: target_type,
        operand: Box::new(operand),
    };
    let cast = |target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(operand),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let widen_then_cast = |source| cast(PrimitiveType::I16, widen(PrimitiveType::I32, source));
    let cast_then_widen = |source| widen(PrimitiveType::I32, cast(PrimitiveType::I16, source));
    let sources = vec![
        (
            PrimitiveType::I16,
            psi_numerics::literals::LandedIntegerType::I16,
            widen_then_cast(binary(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
            )),
        ),
        (
            PrimitiveType::I16,
            psi_numerics::literals::LandedIntegerType::I16,
            widen_then_cast(binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                literal(-2i64, psi_numerics::literals::LandedIntegerType::I16),
            )),
        ),
        (
            PrimitiveType::I16,
            psi_numerics::literals::LandedIntegerType::I16,
            widen_then_cast(binary(
                CheckedIntegerBinaryKind::ExactShiftRight,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            )),
        ),
        (
            PrimitiveType::I32,
            psi_numerics::literals::LandedIntegerType::I32,
            cast_then_widen(binary(
                CheckedIntegerBinaryKind::ExactRemainder,
                PrimitiveType::U16,
                parameter(PrimitiveType::U16),
                literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
            )),
        ),
    ];
    for (target_type, landed_type, source) in sources {
        let targets = [
            binary(
                CheckedIntegerBinaryKind::ExactAdd,
                target_type,
                source.clone(),
                literal(1i64, landed_type),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                target_type,
                source.clone(),
                literal(-2i64, landed_type),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactShiftLeft,
                target_type,
                source.clone(),
                literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactDivide,
                target_type,
                source,
                literal(2i64, landed_type),
            ),
        ];
        for target in targets {
            assert_eq!(
                exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameter_positions_for_test(
                    &target, 1,
                ),
                Some(vec![0]),
            );
        }
    }

    let alternating = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        cast(
            PrimitiveType::U8,
            widen(
                PrimitiveType::I16,
                cast(
                    PrimitiveType::U8,
                    widen(
                        PrimitiveType::I16,
                        binary(
                            CheckedIntegerBinaryKind::ExactAdd,
                            PrimitiveType::I8,
                            parameter(PrimitiveType::I8),
                            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
                        ),
                    ),
                ),
            ),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameter_positions_for_test(
            &alternating,
            1,
        ),
        Some(vec![0]),
        "alternating strict-widen and partial-cast edges remain one ordered spine",
    );

    for homogeneous in [
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            widen(
                PrimitiveType::I16,
                binary(
                    CheckedIntegerBinaryKind::ExactAdd,
                    PrimitiveType::I8,
                    parameter(PrimitiveType::I8),
                    literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
                ),
            ),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
        ),
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I8,
            cast(
                PrimitiveType::I8,
                binary(
                    CheckedIntegerBinaryKind::ExactAdd,
                    PrimitiveType::I16,
                    parameter(PrimitiveType::I16),
                    literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
                ),
            ),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
    ] {
        assert_eq!(
            exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameter_positions_for_test(
                &homogeneous,
                1,
            ),
            None,
            "pure conversion spines retain their narrower dispatch paths",
        );
    }
}

#[test]
fn exact_cast_then_shift_left_classifier_accepts_one_finite_heterogeneous_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let shift = |value_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftLeft,
        primitive_type: value_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let accepted = shift(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0])
    );
    let heterogeneous = shift(
        PrimitiveType::U8,
        shift(
            PrimitiveType::U8,
            accepted,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(0i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(&heterogeneous, 1),
        Some(vec![0])
    );

    let runtime_count = shift(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U8,
        },
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(&runtime_count, 1),
        None
    );
    for count in [-1i64, 8i64] {
        let invalid_count = shift(
            PrimitiveType::U8,
            cast(PrimitiveType::U16, PrimitiveType::U8, 0),
            literal(count, psi_numerics::literals::LandedIntegerType::I16),
        );
        assert_eq!(
            exact_cast_then_shift_left_runtime_parameter_positions_for_test(&invalid_count, 1),
            None
        );
    }
    let mismatched_value_carrier = shift(
        PrimitiveType::U8,
        shift(
            PrimitiveType::U16,
            cast(PrimitiveType::U32, PrimitiveType::U16, 0),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(
            &mismatched_value_carrier,
            1,
        ),
        None
    );
    let right_associated = shift(
        PrimitiveType::U8,
        cast(PrimitiveType::U16, PrimitiveType::U8, 0),
        shift(
            PrimitiveType::U8,
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
    );
    assert_eq!(
        exact_cast_then_shift_left_runtime_parameter_positions_for_test(&right_associated, 1),
        None
    );
}

#[test]
fn exact_cast_then_shift_right_classifier_accepts_one_finite_heterogeneous_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = || CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U16,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let shift = |left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftRight,
        primitive_type: PrimitiveType::U8,
        left: Box::new(left),
        right: Box::new(right),
    };
    let accepted = shift(
        shift(
            cast(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_cast_then_shift_right_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );
    for invalid in [
        shift(
            cast(),
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U8,
            },
        ),
        shift(
            cast(),
            literal(-1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        shift(
            cast(),
            literal(8i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        shift(
            cast(),
            shift(
                literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
                literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            ),
        ),
    ] {
        assert_eq!(
            exact_cast_then_shift_right_runtime_parameter_positions_for_test(&invalid, 1),
            None,
        );
    }
}

#[test]
fn exact_cast_then_divide_remainder_classifier_accepts_one_unified_safe_literal_chain() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::I8,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = || CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::I8,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I16,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::I8,
        left: Box::new(left),
        right: Box::new(right),
    };
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        operation(CheckedIntegerBinaryKind::ExactDivide, cast(), literal(2i64)),
        literal(-3i64),
    );
    assert_eq!(
        exact_cast_then_divide_remainder_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );
    for divisor in [0i64, -1i64] {
        let invalid = operation(
            CheckedIntegerBinaryKind::ExactDivide,
            cast(),
            literal(divisor),
        );
        assert_eq!(
            exact_cast_then_divide_remainder_runtime_parameter_positions_for_test(&invalid, 1),
            None,
        );
    }
    let runtime_divisor = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        cast(),
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I8,
        },
    );
    assert_eq!(
        exact_cast_then_divide_remainder_runtime_parameter_positions_for_test(&runtime_divisor, 1,),
        None,
    );
}

#[test]
fn exact_runtime_divisor_chain_classifier_unifies_direct_and_partial_cast_roots() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::U8,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U8,
        left: Box::new(left),
        right: Box::new(right),
    };
    let direct = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        operation(
            CheckedIntegerBinaryKind::ExactDivide,
            parameter(0, PrimitiveType::U8),
            parameter(1, PrimitiveType::U8),
        ),
        literal(2i64),
    );
    assert_eq!(
        exact_runtime_divisor_chain_runtime_parameter_positions_for_test(&direct, 3),
        Some(vec![0, 1]),
    );
    let partial_cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(parameter(0, PrimitiveType::U16)),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let post_cast = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        operation(
            CheckedIntegerBinaryKind::ExactDivide,
            partial_cast,
            parameter(1, PrimitiveType::U8),
        ),
        parameter(2, PrimitiveType::U8),
    );
    assert_eq!(
        exact_runtime_divisor_chain_runtime_parameter_positions_for_test(&post_cast, 3),
        Some(vec![0, 1, 2]),
    );
    let one_operation = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        parameter(0, PrimitiveType::U8),
        parameter(1, PrimitiveType::U8),
    );
    assert_eq!(
        exact_runtime_divisor_chain_runtime_parameter_positions_for_test(&one_operation, 2),
        None,
    );
    let literal_only = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        operation(
            CheckedIntegerBinaryKind::ExactDivide,
            parameter(0, PrimitiveType::U8),
            literal(2i64),
        ),
        literal(3i64),
    );
    assert_eq!(
        exact_runtime_divisor_chain_runtime_parameter_positions_for_test(&literal_only, 1),
        None,
    );
    let mistyped_divisor = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        operation(
            CheckedIntegerBinaryKind::ExactDivide,
            parameter(0, PrimitiveType::U8),
            parameter(1, PrimitiveType::U16),
        ),
        literal(2i64),
    );
    assert_eq!(
        exact_runtime_divisor_chain_runtime_parameter_positions_for_test(&mistyped_divisor, 2),
        None,
    );
    let address_operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::Addr,
        left: Box::new(left),
        right: Box::new(right),
    };
    let address_chain = address_operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        address_operation(
            CheckedIntegerBinaryKind::ExactDivide,
            parameter(0, PrimitiveType::Addr),
            parameter(1, PrimitiveType::Addr),
        ),
        parameter(2, PrimitiveType::Addr),
    );
    assert_eq!(
        exact_runtime_divisor_chain_runtime_parameter_positions_for_test(&address_chain, 3),
        None,
    );
    let identity_cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(parameter(0, PrimitiveType::U8)),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let invalid_cast_root = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        identity_cast,
        parameter(1, PrimitiveType::U8),
    );
    assert_eq!(
        exact_runtime_divisor_chain_runtime_parameter_positions_for_test(&invalid_cast_root, 2),
        None,
    );
}

#[test]
fn exact_mixed_shift_classifier_accepts_finite_ordered_alternation() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let shift = |kind, value_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: value_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let right = shift(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    let mixed = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            right.clone(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&mixed, 1),
        Some(vec![0]),
    );

    let homogeneous = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&homogeneous, 1),
        None,
    );
    let left_then_right = shift(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&left_then_right, 1),
        Some(vec![0]),
    );
    let alternating = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            shift(
                CheckedIntegerBinaryKind::ExactShiftLeft,
                PrimitiveType::U8,
                parameter(0, PrimitiveType::U8),
                literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
            ),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&alternating, 1),
        Some(vec![0]),
    );
    let partial_cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(parameter(0, PrimitiveType::I16)),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let post_cast_alternating = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            shift(
                CheckedIntegerBinaryKind::ExactShiftLeft,
                PrimitiveType::U8,
                partial_cast.clone(),
                literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
            ),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_then_mixed_shift_runtime_parameter_positions_for_test(&post_cast_alternating, 1,),
        Some(vec![0]),
    );
    let post_cast_left_then_right = shift(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: PrimitiveType::U8,
                operand: Box::new(parameter(0, PrimitiveType::I16)),
                range: psi_checked_trees::CheckedIntegerRange::default(),
            },
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_cast_then_mixed_shift_runtime_parameter_positions_for_test(
            &post_cast_left_then_right,
            1,
        ),
        Some(vec![0]),
    );
    let homogeneous_post_cast = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            partial_cast,
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_mixed_shift_runtime_parameter_positions_for_test(&homogeneous_post_cast, 1,),
        None,
        "homogeneous post-cast shifts stay on their existing classifier paths",
    );
    assert_eq!(
        exact_mixed_shift_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &alternating,
            1,
        ),
        Some(vec![0]),
    );
    assert_eq!(
        exact_mixed_shift_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &homogeneous,
            1,
        ),
        None,
        "homogeneous shift-cast chains stay on their existing classifier paths",
    );
    assert_eq!(
        exact_mixed_shift_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::Addr,
            &alternating,
            1,
        ),
        None,
    );
    let runtime_count = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            parameter(1, PrimitiveType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&runtime_count, 2),
        None,
    );
    let mismatched = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&mismatched, 1),
        None,
    );
    let address = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::Addr,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::Addr,
            parameter(0, PrimitiveType::Addr),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&address, 1),
        None,
    );
}

#[test]
fn exact_shift_cast_shift_classifier_unifies_both_nonempty_sides() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let shift = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let source = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U16,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U16,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    let cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(source),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let accepted = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            cast,
            literal(1i64, psi_numerics::literals::LandedIntegerType::I32),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_shift_cast_shift_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );

    let homogeneous_source = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U16,
        parameter(),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    let right_only_target = shift(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        CheckedScalarExpression::IntegerExactCast {
            primitive_type: PrimitiveType::U8,
            operand: Box::new(homogeneous_source),
            range: psi_checked_trees::CheckedIntegerRange::default(),
        },
        literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_shift_cast_shift_runtime_parameter_positions_for_test(&right_only_target, 1),
        Some(vec![0]),
    );

    let empty_source = shift(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        CheckedScalarExpression::IntegerExactCast {
            primitive_type: PrimitiveType::U8,
            operand: Box::new(parameter()),
            range: psi_checked_trees::CheckedIntegerRange::default(),
        },
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_shift_cast_shift_runtime_parameter_positions_for_test(&empty_source, 1),
        None,
    );

    let runtime_count = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        right_only_target,
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::U8,
        },
    );
    assert_eq!(
        exact_shift_cast_shift_runtime_parameter_positions_for_test(&runtime_count, 2),
        None,
    );
}

#[test]
fn exact_affine_shift_cast_sandwich_classifier_accepts_both_directions() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let cast = |operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(operand),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };

    let source_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    let affine_cast_shift = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            cast(source_affine),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U32),
    );
    assert_eq!(
        exact_affine_shift_cast_sandwich_runtime_parameter_positions_for_test(
            &affine_cast_shift,
            1,
        ),
        Some(vec![0]),
    );

    let source_shift = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U16,
        binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U16,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    let shift_cast_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U8,
            cast(source_shift),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_affine_shift_cast_sandwich_runtime_parameter_positions_for_test(
            &shift_cast_affine,
            1,
        ),
        Some(vec![0]),
    );

    let empty_source = binary(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        cast(parameter()),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_affine_shift_cast_sandwich_runtime_parameter_positions_for_test(&empty_source, 1),
        None,
    );

    let runtime_count = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        affine_cast_shift,
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::U8,
        },
    );
    assert_eq!(
        exact_affine_shift_cast_sandwich_runtime_parameter_positions_for_test(&runtime_count, 2),
        None,
    );
}

#[test]
fn exact_divide_remainder_cross_cast_classifier_accepts_all_four_compositions() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let cast = |operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(operand),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let source_divide_remainder = || {
        binary(
            CheckedIntegerBinaryKind::ExactRemainder,
            PrimitiveType::U16,
            binary(
                CheckedIntegerBinaryKind::ExactDivide,
                PrimitiveType::U16,
                parameter(),
                literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
            ),
            literal(64i64, psi_numerics::literals::LandedIntegerType::U16),
        )
    };
    let divide_cast_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U8,
            cast(source_divide_remainder()),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(
            &divide_cast_affine,
            1,
        ),
        Some(vec![0]),
    );
    let divide_cast_shift = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            cast(source_divide_remainder()),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U32),
    );
    assert_eq!(
        exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(
            &divide_cast_shift,
            1,
        ),
        Some(vec![0]),
    );

    let source_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    let affine_cast_divide = binary(
        CheckedIntegerBinaryKind::ExactRemainder,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactDivide,
            PrimitiveType::U8,
            cast(source_affine),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(
            &affine_cast_divide,
            1,
        ),
        Some(vec![0]),
    );
    let source_shift = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U16,
        binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U16,
            parameter(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    let shift_cast_remainder = binary(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U8,
        cast(source_shift),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(
            &shift_cast_remainder,
            1,
        ),
        Some(vec![0]),
    );

    let runtime_divisor = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        cast(binary(
            CheckedIntegerBinaryKind::ExactDivide,
            PrimitiveType::U16,
            parameter(),
            CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U16,
            },
        )),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(&runtime_divisor, 2,),
        None,
    );
    let empty_source = binary(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U8,
        cast(parameter()),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(&empty_source, 1),
        None,
    );
}

#[test]
fn exact_divide_remainder_cast_sandwich_classifier_requires_both_safe_chains() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let binary = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let source = || {
        binary(
            CheckedIntegerBinaryKind::ExactRemainder,
            PrimitiveType::U16,
            binary(
                CheckedIntegerBinaryKind::ExactDivide,
                PrimitiveType::U16,
                parameter(),
                literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
            ),
            literal(64i64, psi_numerics::literals::LandedIntegerType::U16),
        )
    };
    let cast = |operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(operand),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let sandwich = binary(
        CheckedIntegerBinaryKind::ExactRemainder,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactDivide,
            PrimitiveType::U8,
            cast(source()),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cast_sandwich_runtime_parameter_positions_for_test(&sandwich, 1),
        Some(vec![0]),
    );

    let unsafe_target = binary(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U8,
        cast(source()),
        literal(0i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cast_sandwich_runtime_parameter_positions_for_test(
            &unsafe_target,
            1,
        ),
        None,
    );
    let empty_source = binary(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U8,
        cast(parameter()),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cast_sandwich_runtime_parameter_positions_for_test(&empty_source, 1,),
        None,
    );
}

#[test]
fn exact_divide_remainder_cross_chain_classifier_accepts_all_four_compositions() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position| CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::U8,
    };
    let binary = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U8,
        left: Box::new(left),
        right: Box::new(right),
    };
    let u8_literal = |value| literal(value, psi_numerics::literals::LandedIntegerType::U8);
    let divide_remainder = || {
        binary(
            CheckedIntegerBinaryKind::ExactRemainder,
            binary(
                CheckedIntegerBinaryKind::ExactDivide,
                parameter(0),
                u8_literal(2i64),
            ),
            u8_literal(64i64),
        )
    };
    let divide_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            divide_remainder(),
            u8_literal(1i64),
        ),
        u8_literal(2i64),
    );
    assert_eq!(
        exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test(&divide_affine, 1,),
        Some(vec![0]),
    );
    let divide_shift = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            divide_remainder(),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test(&divide_shift, 1,),
        Some(vec![0]),
    );

    let affine_divide = binary(
        CheckedIntegerBinaryKind::ExactRemainder,
        binary(
            CheckedIntegerBinaryKind::ExactDivide,
            binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                binary(
                    CheckedIntegerBinaryKind::ExactAdd,
                    parameter(0),
                    u8_literal(1i64),
                ),
                u8_literal(2i64),
            ),
            u8_literal(2i64),
        ),
        u8_literal(3i64),
    );
    assert_eq!(
        exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test(&affine_divide, 1,),
        Some(vec![0]),
    );
    let shift_divide = binary(
        CheckedIntegerBinaryKind::ExactDivide,
        binary(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            binary(
                CheckedIntegerBinaryKind::ExactShiftRight,
                parameter(0),
                u8_literal(1i64),
            ),
            u8_literal(2i64),
        ),
        u8_literal(2i64),
    );
    assert_eq!(
        exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test(&shift_divide, 1,),
        Some(vec![0]),
    );

    let runtime_divisor = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        binary(
            CheckedIntegerBinaryKind::ExactDivide,
            parameter(0),
            parameter(1),
        ),
        u8_literal(1i64),
    );
    assert_eq!(
        exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test(
            &runtime_divisor,
            2,
        ),
        None,
    );
    assert_eq!(
        exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test(
            &divide_remainder(),
            1,
        ),
        None,
    );
}

#[test]
fn exact_arithmetic_then_shift_classifier_unifies_affine_prefix_shapes() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let affine = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            affine,
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_arithmetic_then_shift_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );

    let homogeneous_offset = operation(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(0i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_arithmetic_then_shift_runtime_parameter_positions_for_test(&homogeneous_offset, 1),
        Some(vec![0]),
    );
    let outer_right = operation(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        homogeneous_offset,
        literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_arithmetic_then_shift_runtime_parameter_positions_for_test(&outer_right, 1),
        Some(vec![0]),
    );

    let right_only = operation(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_arithmetic_then_shift_runtime_parameter_positions_for_test(&right_only, 1),
        None,
    );

    let runtime_count = operation(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        parameter(1, PrimitiveType::U8),
    );
    assert_eq!(
        exact_arithmetic_then_shift_runtime_parameter_positions_for_test(&runtime_count, 2),
        None,
    );

    let negative_factor = operation(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::I8,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I8,
            parameter(0, PrimitiveType::I8),
            literal(-1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_arithmetic_then_shift_runtime_parameter_positions_for_test(&negative_factor, 1),
        None,
    );
}

#[test]
fn exact_shift_then_arithmetic_classifier_unifies_affine_suffix_shapes() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let shift_prefix = operation(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U8,
            shift_prefix,
            literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_shift_then_arithmetic_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );

    let homogeneous = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_shift_then_arithmetic_runtime_parameter_positions_for_test(&homogeneous, 1),
        Some(vec![0]),
    );

    let runtime_count = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            parameter(1, PrimitiveType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_shift_then_arithmetic_runtime_parameter_positions_for_test(&runtime_count, 2),
        None,
    );

    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        parameter(1, PrimitiveType::U8),
    );
    assert_eq!(
        exact_shift_then_arithmetic_runtime_parameter_positions_for_test(&runtime_sibling, 2),
        None,
    );

    let negative_factor = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I8,
        operation(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::I8,
            parameter(0, PrimitiveType::I8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(-1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_shift_then_arithmetic_runtime_parameter_positions_for_test(&negative_factor, 1),
        None,
    );
}

#[test]
fn shift_left_chain_exact_cast_classifier_accepts_one_finite_heterogeneous_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let shift = |value_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftLeft,
        primitive_type: value_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let one = shift(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &one,
            1,
        ),
        Some(vec![0])
    );
    let heterogeneous = shift(
        PrimitiveType::U16,
        shift(
            PrimitiveType::U16,
            one,
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(0i64, psi_numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::I8,
            &heterogeneous,
            1,
        ),
        Some(vec![0])
    );

    let runtime_count = shift(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        parameter(0, PrimitiveType::U16),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &runtime_count,
            1,
        ),
        None
    );
    for count in [-1i64, 16i64] {
        let invalid_count = shift(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(count, psi_numerics::literals::LandedIntegerType::I16),
        );
        assert_eq!(
            exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
                PrimitiveType::U8,
                &invalid_count,
                1,
            ),
            None
        );
    }
    let mismatched_value_carrier = shift(
        PrimitiveType::U16,
        shift(
            PrimitiveType::U8,
            parameter(0, PrimitiveType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &mismatched_value_carrier,
            1,
        ),
        None
    );
    let right_associated = shift(
        PrimitiveType::U16,
        parameter(0, PrimitiveType::U16),
        shift(
            PrimitiveType::U16,
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &right_associated,
            1,
        ),
        None
    );
    let local_root = shift(
        PrimitiveType::U16,
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U16,
        },
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &local_root,
            1,
        ),
        None
    );
}

#[test]
fn shift_right_chain_exact_cast_classifier_accepts_heterogeneous_legal_counts_only() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let shift = |value_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftRight,
        primitive_type: value_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let accepted = shift(
        PrimitiveType::U16,
        shift(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_shift_right_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &accepted,
            1,
        ),
        Some(vec![0])
    );
    for invalid in [
        shift(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            parameter(0, PrimitiveType::U16),
        ),
        shift(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(-1i64, psi_numerics::literals::LandedIntegerType::I8),
        ),
        shift(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(16i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
    ] {
        assert_eq!(
            exact_shift_right_chain_cast_runtime_parameter_positions_for_test(
                PrimitiveType::U8,
                &invalid,
                1,
            ),
            None
        );
    }
}

#[test]
fn divide_remainder_chain_exact_cast_classifier_accepts_only_carrier_total_hulls() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U16,
        left: Box::new(left),
        right: Box::new(right),
    };
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        operation(
            CheckedIntegerBinaryKind::ExactDivide,
            parameter(),
            literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_divide_remainder_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &accepted,
            1,
        ),
        Some(vec![0]),
    );
    let non_total = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        parameter(),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_divide_remainder_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &non_total,
            1,
        ),
        None,
    );
    for invalid_divisor in [0i64, -1i64] {
        let invalid = CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactRemainder,
            primitive_type: PrimitiveType::I16,
            left: Box::new(CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::I16,
            }),
            right: Box::new(literal(
                invalid_divisor,
                psi_numerics::literals::LandedIntegerType::I16,
            )),
        };
        assert_eq!(
            exact_divide_remainder_chain_cast_runtime_parameter_positions_for_test(
                PrimitiveType::I8,
                &invalid,
                1,
            ),
            None,
        );
    }
    let mistyped = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        parameter(),
        literal(3i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::U8,
            &mistyped,
            1,
        ),
        None,
    );
}

#[test]
fn exact_affine_chain_classifier_accepts_only_left_associated_landed_mixed_operations() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::U8,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position| CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::U8,
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U8,
        left: Box::new(left),
        right: Box::new(right),
    };
    let added = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        parameter(0),
        literal(3i64),
    );
    let multiplied = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        added.clone(),
        literal(2i64),
    );
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        multiplied,
        literal(1i64),
    );
    assert_eq!(
        exact_affine_chain_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0])
    );
    for invalid in [
        added,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            parameter(0),
            literal(2i64),
        ),
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                parameter(0),
                literal(1i64),
            ),
            parameter(0),
        ),
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            literal(2i64),
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                parameter(0),
                literal(1i64),
            ),
        ),
    ] {
        assert_eq!(
            exact_affine_chain_runtime_parameter_positions_for_test(&invalid, 1),
            None
        );
    }
}

#[test]
fn affine_chain_exact_cast_classifier_reuses_only_the_unified_mixed_family() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::U16,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U16,
        left: Box::new(left),
        right: Box::new(right),
    };
    let added = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        parameter(),
        literal(3i64),
    );
    let affine = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            added.clone(),
            literal(2i64),
        ),
        literal(1i64),
    );
    assert_eq!(
        exact_affine_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::U8, &affine, 1,),
        Some(vec![0]),
    );
    assert_eq!(
        exact_affine_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::U8, &added, 1,),
        None,
        "homogeneous chains remain on the narrower computed-cast classifiers",
    );
    assert_eq!(
        exact_affine_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::Addr,
            &affine,
            1,
        ),
        None,
    );
}

#[test]
fn exact_cast_then_affine_classifier_accepts_only_the_unified_mixed_family() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::U8,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = || CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U16,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U8,
        left: Box::new(left),
        right: Box::new(right),
    };
    let added = operation(CheckedIntegerBinaryKind::ExactAdd, cast(), literal(3i64));
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            added.clone(),
            literal(2i64),
        ),
        literal(1i64),
    );
    assert_eq!(
        exact_cast_then_affine_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );
    for invalid in [
        added,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            cast(),
            literal(2i64),
        ),
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            operation(CheckedIntegerBinaryKind::ExactAdd, cast(), literal(1i64)),
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U8,
            },
        ),
    ] {
        assert_eq!(
            exact_cast_then_affine_runtime_parameter_positions_for_test(&invalid, 1),
            None,
        );
    }
}

#[test]
fn exact_affine_cast_affine_classifier_unifies_both_nonempty_sides() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let source = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U16,
            },
            literal(3i64, psi_numerics::literals::LandedIntegerType::U16),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U16),
    );
    let cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(source),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U8,
            cast,
            literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_affine_cast_affine_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );

    let direct_cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U16,
        }),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let empty_source_side = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        direct_cast,
        literal(1i64, psi_numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_affine_cast_affine_runtime_parameter_positions_for_test(&empty_source_side, 1),
        None,
    );

    let negative_source = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I16,
        },
        literal(-1i64, psi_numerics::literals::LandedIntegerType::I16),
    );
    let fenced = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        CheckedScalarExpression::IntegerExactCast {
            primitive_type: PrimitiveType::I8,
            operand: Box::new(negative_source),
            range: psi_checked_trees::CheckedIntegerRange::default(),
        },
        literal(1i64, psi_numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_affine_cast_affine_runtime_parameter_positions_for_test(&fenced, 1),
        None,
    );
}

#[test]
fn signed_affine_cast_affine_classifier_preserves_two_branch_priority() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let i16_literal = |value| literal(value, psi_numerics::literals::LandedIntegerType::I16);
    let i8_literal = |value| literal(value, psi_numerics::literals::LandedIntegerType::I8);
    let cast = |source| CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::I8,
        operand: Box::new(source),
        range: psi_checked_trees::CheckedIntegerRange::default(),
    };
    let source = |factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                i16_literal(3),
            ),
            i16_literal(factor),
        )
    };
    let target = |root, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I8,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I8,
                root,
                i8_literal(3),
            ),
            i8_literal(factor),
        )
    };

    let source_negative = target(cast(source(-2)), 2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&source_negative, 1,),
        Some(vec![0]),
    );
    let target_negative = target(cast(source(2)), -2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&target_negative, 1,),
        Some(vec![0]),
    );
    let both_negative = target(cast(source(-2)), -2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&both_negative, 1),
        Some(vec![0]),
    );

    let all_nonnegative = target(cast(source(2)), 2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&all_nonnegative, 1,),
        None,
        "the established nonnegative sandwich keeps priority",
    );
    assert_eq!(
        exact_affine_cast_affine_runtime_parameter_positions_for_test(&all_nonnegative, 1),
        Some(vec![0]),
    );
    let empty_source = target(cast(parameter(PrimitiveType::I16)), -2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&empty_source, 1),
        None,
        "the one-sided post-cast signed-affine path keeps priority",
    );
    let source_product_without_offset = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        parameter(PrimitiveType::I16),
        i16_literal(-2),
    );
    let target_offset_only = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        cast(source_product_without_offset),
        i8_literal(1),
    );
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(
            &target_offset_only,
            1,
        ),
        None,
        "a thin homogeneous-product/offset permutation remains fenced",
    );

    let i64_literal = |value| literal(value, psi_numerics::literals::LandedIntegerType::I64);
    let overflow_source = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I64,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I64,
            operation(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I64,
                operation(
                    CheckedIntegerBinaryKind::ExactAdd,
                    PrimitiveType::I64,
                    parameter(PrimitiveType::I64),
                    i64_literal(0),
                ),
                i64_literal(i64::MIN),
            ),
            i64_literal(i64::MIN),
        ),
        i64_literal(4),
    );
    let overflow = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        cast(overflow_source),
        i8_literal(1),
    );
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&overflow, 1),
        None,
        "checked coefficient overflow admits no shared runtime-input family",
    );
}

#[test]
fn affine_fork_join_classifier_requires_two_disjoint_branches_on_one_root() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let i16_literal = |value| literal(value, psi_numerics::literals::LandedIntegerType::I16);
    let branch = |position, offset, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                i16_literal(offset),
            ),
            i16_literal(factor),
        )
    };

    let joined = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        branch(0, 1, 2),
        branch(0, -1, 3),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&joined, 2),
        Some(vec![0]),
    );
    let cancellation = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::I16,
        branch(0, 3, -2),
        branch(0, -4, -2),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&cancellation, 2),
        Some(vec![0]),
        "a zero combined coefficient remains a valid join-local result",
    );

    let distinct_roots = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        branch(0, 1, 2),
        branch(1, -1, 3),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&distinct_roots, 2),
        None,
        "distinct roots require multivariate proof design",
    );
    let empty_right = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        branch(0, 1, 2),
        parameter(0, PrimitiveType::I16),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&empty_right, 2),
        None,
        "both proof-bearing branches must be nonempty",
    );
    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            parameter(0, PrimitiveType::I16),
            parameter(0, PrimitiveType::I16),
        ),
        branch(0, 1, 2),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&runtime_sibling, 2),
        None,
        "a branch sibling must be an independently landed literal",
    );
    let linear_chain = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        branch(0, 1, 2),
        i16_literal(1),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&linear_chain, 2),
        None,
        "one-sided chains retain their existing classifier priority",
    );

    let i64_literal = |value| literal(value, psi_numerics::literals::LandedIntegerType::I64);
    let overflow_branch = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I64,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I64,
            operation(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I64,
                parameter(0, PrimitiveType::I64),
                i64_literal(i64::MAX),
            ),
            i64_literal(i64::MAX),
        ),
        i64_literal(i64::MAX),
    );
    let overflow = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I64,
        overflow_branch,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I64,
            parameter(0, PrimitiveType::I64),
            i64_literal(1),
        ),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&overflow, 2),
        None,
        "checked branch coefficient overflow admits no family",
    );
}

#[test]
fn distinct_root_affine_fork_join_classifier_requires_two_direct_roots() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type: psi_numerics::literals::LandedIntegerType::I16,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::I16,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position| CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::I16,
    };
    let branch = |position, offset, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                parameter(position),
                literal(offset),
            ),
            literal(factor),
        )
    };
    let joined = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        branch(0, 1, 2),
        branch(1, -1, 3),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(&joined, 2),
        Some(vec![0, 1]),
    );
    let signed_subtract = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        branch(0, 3, -2),
        branch(1, -4, -2),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(
            &signed_subtract,
            2,
        ),
        Some(vec![0, 1]),
    );
    let same_root = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        branch(0, 1, 2),
        branch(0, -1, 3),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(&same_root, 2),
        None,
        "the correlated same-root family retains dispatch priority",
    );
    let empty_branch = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        branch(0, 1, 2),
        parameter(1),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(&empty_branch, 2,),
        None,
    );
    let computed_sibling = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        branch(0, 1, 2),
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            parameter(1),
            parameter(0),
        ),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(
            &computed_sibling,
            2,
        ),
        None,
        "every branch edge requires an independently landed literal sibling",
    );
}

#[test]
fn distinct_root_affine_product_join_classifier_requires_signed_direct_roots() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let signed_branch = |position, offset, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                literal(offset, psi_numerics::literals::LandedIntegerType::I16),
            ),
            literal(factor, psi_numerics::literals::LandedIntegerType::I16),
        )
    };
    let joined = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(1, -1, 3),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(&joined, 2),
        Some(vec![0, 1]),
    );
    let negative = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 3, -2),
        signed_branch(1, -4, -2),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(&negative, 2),
        Some(vec![0, 1]),
        "negative affine coefficients reverse their own forward endpoints",
    );
    let same_root = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(0, -1, 3),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(&same_root, 2,),
        None,
        "same-root multiplication requires correlated quadratic algebra",
    );
    let direct_side = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        parameter(1, PrimitiveType::I16),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(
            &direct_side,
            2,
        ),
        None,
    );
    let computed_root = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            parameter(1, PrimitiveType::I16),
            parameter(0, PrimitiveType::I16),
        ),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(
            &computed_root,
            2,
        ),
        None,
    );
    let unsigned_branch = |position| {
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(position, PrimitiveType::U16),
            literal(1, psi_numerics::literals::LandedIntegerType::U16),
        )
    };
    let unsigned = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        unsigned_branch(0),
        unsigned_branch(1),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(&unsigned, 2),
        None,
        "the initial product-rectangle family is signed-only",
    );
}

#[test]
fn same_root_affine_product_join_classifier_requires_a_genuine_signed_quadratic() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let signed_branch = |position, offset, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                literal(offset, psi_numerics::literals::LandedIntegerType::I16),
            ),
            literal(factor, psi_numerics::literals::LandedIntegerType::I16),
        )
    };
    let joined = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(0, -1, 3),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&joined, 1),
        Some(vec![0]),
    );
    let concave = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 3, -2),
        signed_branch(0, -4, 2),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&concave, 1),
        Some(vec![0]),
        "opposite branch signs retain a genuine concave quadratic",
    );
    let distinct_roots = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(1, -1, 3),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(
            &distinct_roots,
            2,
        ),
        None,
        "distinct roots remain on the independent rectangle path",
    );
    let zero_branch = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(0, -1, 0),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&zero_branch, 1),
        None,
        "a constant collapse is not a quadratic family",
    );
    let direct_side = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        parameter(0, PrimitiveType::I16),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&direct_side, 1),
        None,
    );
    let computed_branch = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            parameter(0, PrimitiveType::I16),
            parameter(0, PrimitiveType::I16),
        ),
        literal(1, psi_numerics::literals::LandedIntegerType::I16),
    );
    let computed_root = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        computed_branch,
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&computed_root, 1,),
        None,
        "a computed root is not signature-bound authority",
    );
    let unsigned_branch = |offset| {
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(offset, psi_numerics::literals::LandedIntegerType::U16),
        )
    };
    let unsigned = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        unsigned_branch(1),
        unsigned_branch(2),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&unsigned, 1),
        None,
        "the initial correlated quadratic family is signed-only",
    );
}

#[test]
fn same_root_affine_divide_remainder_classifier_requires_two_genuine_signed_branches() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
            psi_numerics::literals::IntegerLanding {
                landed_type,
                domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let branch = |position, offset, factor, offset_kind| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                offset_kind,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                literal(offset, psi_numerics::literals::LandedIntegerType::I16),
            ),
            literal(factor, psi_numerics::literals::LandedIntegerType::I16),
        )
    };
    let divisor = |position| {
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                literal(2, psi_numerics::literals::LandedIntegerType::I16),
            ),
            literal(1, psi_numerics::literals::LandedIntegerType::I16),
        )
    };
    for kind in [
        CheckedIntegerBinaryKind::ExactDivide,
        CheckedIntegerBinaryKind::ExactRemainder,
    ] {
        let joined = operation(
            kind,
            PrimitiveType::I16,
            branch(0, 16384, -2, CheckedIntegerBinaryKind::ExactAdd),
            divisor(0),
        );
        assert_eq!(
            exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
                &joined, 1,
            ),
            Some(vec![0]),
        );
    }
    let distinct = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::I16,
        branch(0, 16384, -2, CheckedIntegerBinaryKind::ExactAdd),
        divisor(1),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &distinct, 2,
        ),
        None,
        "distinct roots cannot borrow correlation",
    );
    let zero_branch = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::I16,
        branch(0, 16384, 0, CheckedIntegerBinaryKind::ExactAdd),
        divisor(0),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &zero_branch,
            1,
        ),
        None,
        "constant collapse stays on narrower paths",
    );
    let direct_side = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        PrimitiveType::I16,
        parameter(0, PrimitiveType::I16),
        divisor(0),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &direct_side,
            1,
        ),
        None,
    );
    let computed_root = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::I16,
        branch(0, 16384, -2, CheckedIntegerBinaryKind::ExactAdd),
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(0, PrimitiveType::I16),
                parameter(0, PrimitiveType::I16),
            ),
            literal(1, psi_numerics::literals::LandedIntegerType::I16),
        ),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &computed_root,
            1,
        ),
        None,
        "computed roots are not signature-bound authority",
    );
    let unsigned_branch = |offset| {
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(offset, psi_numerics::literals::LandedIntegerType::U16),
        )
    };
    let unsigned = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U16,
        unsigned_branch(1),
        unsigned_branch(2),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &unsigned, 1,
        ),
        None,
        "the lattice-correlation family is signed-only",
    );
}

#[test]
fn nominal_scalar_cleanup_accepts_finite_short_circuit_continuation_chain() {
    let checked = checked(
        r#"
        data Token { observed: bool; other: bool; }
        machine Token::drop(&mut self) {}
        data Helper {}
        machine Helper::value() -> u64 { 1u64 }
        machine Helper::touch() {}
        data Root {}

        machine Root::short_circuit(token: Token) -> bool {
            let staged: bool = true && false;
            staged
        }
        machine Root::shared_convergence(token: Token, input: bool) -> bool {
            let staged: bool = input && true;
            staged
        }
        machine Root::nested_shared_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (input && true) || false;
            staged
        }
        machine Root::computed_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (!input && true) || false;
            staged
        }
        machine Root::comparison_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (input == false) && true;
            staged
        }
        machine Root::reversed_comparison_leaf_convergence(token: Token, input: bool) -> bool {
            let staged: bool = (true == input) || false;
            staged
        }
        machine Root::multiple_input_convergence(
            token: Token,
            left: bool,
            right: bool
        ) -> bool {
            let staged: bool = left && right;
            staged
        }
        machine Root::multiple_input_comparison_convergence(
            token: Token,
            left: bool,
            right: bool
        ) -> bool {
            let staged: bool = (left == right) && true;
            staged
        }
        machine Root::member_convergence(token: Token, input: bool) -> bool {
            let staged: bool = token.observed && input;
            staged
        }
        machine Root::repeated_member_convergence(token: Token, input: bool) -> bool {
            let staged: bool = token.observed && (input || token.observed);
            staged
        }
        machine Root::member_only_convergence(token: Token) -> bool {
            let staged: bool = token.observed && true;
            staged
        }
        machine Root::multiple_member_convergence(token: Token) -> bool {
            let staged: bool = token.observed && token.other;
            staged
        }
        machine Root::integer_comparison_convergence(token: Token, input: u64) -> bool {
            let staged: bool = (input < 1u64) && true;
            staged
        }
        machine Root::computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = ((input + 1u64) < 4u64) && true;
            staged
        }
        machine Root::nested_computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = (((input + 1u64) + 1u64) < 4u64) && true;
            staged
        }
        machine Root::triple_computed_integer_comparison_convergence(
            token: Token,
            input: u64 in Wrapping
        ) -> bool {
            let staged: bool = ((((input + 1u64) + 1u64) + 1u64) < 4u64) && true;
            staged
        }
        machine Root::bitwise_not_integer_comparison_convergence(
            token: Token,
            input: u64
        ) -> bool {
            let staged: bool = ((~input) < 4u64) && true;
            staged
        }
        machine Root::nested_bitwise_not_integer_comparison_convergence(
            token: Token,
            input: u64
        ) -> bool {
            let staged: bool = ((~(~input)) < 4u64) && true;
            staged
        }
        machine Root::widened_integer_comparison_convergence(
            token: Token,
            input: u8
        ) -> bool {
            let staged: bool = ((input as u16) < 4u16) && true;
            staged
        }
        machine Root::nested_widened_integer_comparison_convergence(
            token: Token,
            input: u8
        ) -> bool {
            let staged: bool = (((input as u16) as u32) < 4u32) && true;
            staged
        }
        machine Root::exact_cast_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 255u64
        {
            let staged: bool = ((input as u8) < 4u8) && enabled;
            staged
        }
        machine Root::signed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        requires -128i64 <= input, input <= 127i64
        {
            let staged: bool = ((input as i8) < 4i8) && enabled;
            staged
        }
        machine Root::unsigned_to_signed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input as i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_to_unsigned_exact_cast_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input
        {
            let staged: bool = ((input as u8) < 4u8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_add_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires input <= 126i8
        {
            let staged: bool = ((input + 1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_add_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input
        {
            let staged: bool = ((input + -1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input
        {
            let staged: bool = ((input - 1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires input <= 126i8
        {
            let staged: bool = ((input - -1i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_positive_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = ((input * 3i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_negative_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = ((input * -3i8) < 4i8) && enabled;
            staged
        }
        machine Root::exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = ((input + 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_add_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires left <= 255u8 - right
        {
            let staged: bool = ((left + right) <= 255u8) && enabled;
            staged
        }
        machine Root::runtime_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= right, left <= 255u8 / right
        {
            let staged: bool = ((left * right) <= 255u8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= right, -128i8 / right <= left, left <= 127i8 / right
        {
            let staged: bool = ((left * right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_multiply_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= -2i8, 127i8 / right <= left, left <= -128i8 / right
        {
            let staged: bool = ((left * right) <= 127i8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_add_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= right, left <= 127i8 - right
        {
            let staged: bool = ((left + right) <= 127i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_add_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= 0i8, -128i8 - right <= left
        {
            let staged: bool = ((left + right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_positive_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= right, right + -128i8 <= left
        {
            let staged: bool = ((left - right) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_negative_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: i8,
            right: i8,
            enabled: bool
        ) -> bool
        requires right <= 0i8, left <= right + 127i8
        {
            let staged: bool = ((left - right) <= 127i8) && enabled;
            staged
        }
        machine Root::exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((127u8 - input) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_subtract_integer_comparison_convergence(
            token: Token,
            left: u8,
            right: u8,
            enabled: bool
        ) -> bool
        requires right <= left
        {
            let staged: bool = ((left - right) < 4u8) && enabled;
            staged
        }
        machine Root::exact_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input * 2u8) < 4u8) && enabled;
            staged
        }
        machine Root::exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input / 2u8) < 4u8) && enabled;
            staged
        }
        machine Root::exact_remainder_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input % 2u8) < 1u8) && enabled;
            staged
        }
        machine Root::signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input / 2i8) < 4i8) && enabled;
            staged
        }
        machine Root::signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((input % -2i8) < 1i8) && enabled;
            staged
        }
        machine Root::runtime_exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            divisor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= divisor
        {
            let staged: bool = ((input / divisor) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= divisor
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires 1i8 <= divisor
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_negative_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires divisor <= -2i8
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_negative_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires divisor <= -2i8
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_bounded_negative_signed_exact_divide_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, divisor <= -1i8
        {
            let staged: bool = ((input / divisor) < 4i8) && enabled;
            staged
        }
        machine Root::runtime_bounded_negative_signed_exact_remainder_integer_comparison_convergence(
            token: Token,
            input: i8,
            divisor: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, divisor <= -1i8
        {
            let staged: bool = ((input % divisor) < 4i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires count <= 7u8
        {
            let staged: bool = ((input >> count) < 4u8) && enabled;
            staged
        }
        machine Root::signed_count_exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: i8,
            count: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= count, count <= 7i8
        {
            let staged: bool = ((input >> count) < 4i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((input << 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::runtime_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires input <= 3u8, count <= 6u8
        {
            let staged: bool = ((input << count) < 4u8) && enabled;
            staged
        }
        machine Root::signed_count_runtime_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: i8,
            enabled: bool
        ) -> bool
        requires input <= 63u8, 0i8 <= count, count <= 2i8
        {
            let staged: bool = ((input << count) < 255u8) && enabled;
            staged
        }
        machine Root::signed_value_exact_shift_left_integer_comparison_convergence(
            token: Token,
            input: i8,
            count: u8,
            signed_count: i8,
            enabled: bool
        ) -> bool
        requires -32i8 <= input, input <= 31i8, count <= 2u8,
            0i8 <= signed_count, signed_count <= 2i8
        {
            let staged: bool = ((input << 1u8) < 64i8)
                && ((input << count) < 127i8)
                && ((input << signed_count) < 127i8)
                && enabled;
            staged
        }
        machine Root::bitwise_not_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((~(input + 3u8)) < 255u8) && enabled;
            staged
        }
        machine Root::widen_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let staged: bool = (((input - 3u8) as u16) < 255u16) && enabled;
            staged
        }
        machine Root::binary_right_exact_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((15u8 & (input * 2u8)) < 16u8) && enabled;
            staged
        }
        machine Root::two_shell_nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((~((input + 3u8) as u16)) < 65535u16) && enabled;
            staged
        }
        machine Root::sibling_exact_operations_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8, input <= 127u8
        {
            let staged: bool = (((input + 1u8) & (input * 2u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((input + 1u8) + 1u8) < 4u8) && enabled;
            staged
        }
        machine Root::deep_nested_exact_add_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((((input + 1u8) + 1u8) + 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_add_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let retained: u8 = input;
            let staged: bool = ((((retained + 1u8) + 1u8) + 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::deep_nested_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let staged: bool = ((((input - 1u8) - 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::reversed_nested_exact_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 2u8 <= input
        {
            let staged: bool = ((255u8 - ((input - 1u8) - 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_subtract_computed_sibling_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= input, (input & 0u8) <= input - 1u8
        {
            let staged: bool = (((input - 1u8) - (input & 0u8)) < 5u8) && enabled;
            staged
        }
        machine Root::nested_exact_subtract_feeds_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 2u8 <= input, input <= 128u8
        {
            let staged: bool = ((((input - 1u8) - 1u8) * 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = (((input + 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 3u8 <= input
        {
            let retained: u8 = input;
            let staged: bool = ((((retained - 1u8) - 1u8) - 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 42u8
        {
            let staged: bool = ((((input * 2u8) * 3u8) * 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1u16) * 1u16) * 1u16) < 5u16) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1u32) * 1u32) * 1u32) < 5u32) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 0u64
        {
            let staged: bool = ((((input * 2u64) * 2u64) * 2u64) < 5u64) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, -21i8 <= input, input <= 21i8
        {
            let staged: bool = ((((input * 2i8) * 3i8) * 1i8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i16) * 1i16) * 1i16) < 5i16) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i32) * 1i32) * 1i32) < 5i32) && enabled;
            staged
        }
        machine Root::exact_multiply_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input * 1i64) * 1i64) * 1i64) < 5i64) && enabled;
            staged
        }
        machine Root::zero_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((((input * 2u8) * 0u8) * 7u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_multiply_chain_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16, input <= 42u16
        {
            let staged: bool = (((((input as u8) * 2u8) * 3u8) < 255u8) && enabled);
            staged
        }
        machine Root::zero_factor_exact_cast_then_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16
        {
            let staged: bool = (((((input as u8) * 2u8) * 0u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -64i16 <= input, input <= 63i16,
            -21i16 <= input, input <= 21i16
        {
            let staged: bool = (((((input as i8) * 2i8) * 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, input <= 42i8
        {
            let staged: bool = (((((input as u8) * 2u8) * 3u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_multiply_chain_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8, input <= 21u8
        {
            let staged: bool = (((((input as i8) * 2i8) * 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16, input <= 10922u16, input <= 42u16
        {
            let staged: bool = (((((input * 2u16) * 3u16) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::zero_factor_exact_multiply_chain_then_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16
        {
            let staged: bool = (((((input * 2u16) * 0u16) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -16384i16 <= input, input <= 16383i16,
            -5461i16 <= input, input <= 5461i16,
            -21i16 <= input, input <= 21i16
        {
            let staged: bool = (((((input * 2i16) * 3i16) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, 0i8 <= input
        {
            let staged: bool = ((((input * 2i8) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_multiply_chain_then_cast_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8
        {
            let staged: bool = ((((input * 2u8) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::runtime_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            factor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= factor, input <= 255u8 / factor
        {
            let staged: bool = (((input * factor) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::negative_factor_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -42i8 <= input, input <= 42i8
        {
            let staged: bool = (((input * 1i8) * -3i8) < 5i8) && enabled;
            staged
        }
        machine Root::reversed_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((2u8 * ((input * 1u8) * 1u8)) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let retained: u8 = input;
            let staged: bool = (((retained * 1u8) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_add_feeds_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = ((((input + 1u8) * 1u8) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_multiply_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((((input * 1u8) as u16) * 1u16) * 1u16) < 5u16) && enabled;
            staged
        }
        machine Root::two_computed_exact_multiply_operands_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input & 0u8) * (input & 0u8)) * 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u8) % 3u8) / 2u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u16) % 3u16) / 2u16) < 5u16) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u32) % 3u32) / 2u32) < 5u32) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u64) % 3u64) / 2u64) < 5u64) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i8) % 3i8) / 2i8) < 5i8) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i16) % 3i16) / 2i16) < 5i16) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i32) % 3i32) / 2i32) < 5i32) && enabled;
            staged
        }
        machine Root::mixed_exact_divide_remainder_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2i64) % 3i64) / 2i64) < 5i64) && enabled;
            staged
        }
        machine Root::runtime_divisor_exact_divide_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            divisor: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= divisor
        {
            let staged: bool = (((input / 2u8) / divisor) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_divide_remainder_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let retained: u8 = input;
            let staged: bool = ((((retained / 2u8) % 3u8) / 2u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_add_feeds_divide_remainder_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8
        {
            let staged: bool = (((((input + 1u8) / 2u8) % 3u8) < 5u8) && enabled);
            staged
        }
        machine Root::computed_right_exact_divide_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((input / ((input % 2u8) + 1u8)) < 5u8) && enabled;
            staged
        }
        machine Root::signed_negative_one_exact_divide_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = (((input / 2i8) / -1i8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i8) >> 2u16) >> 0i32) < 5u8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u8) >> 2i16) >> 3u32) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i64) >> 2u8) >> 3i16) < 5u32) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_u64_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u32) >> 2i8) >> 3u64) < 5u64) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u16) >> 2i32) >> 3u8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i8) >> 2u32) >> 3i64) < 5i16) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1u64) >> 2i16) >> 3u8) < 5i32) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 1i32) >> 2u16) >> 3u64) < 5i64) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token, input: u16, enabled: bool
        ) -> bool
        requires input <= 2047u16
        {
            let staged: bool = ((((input >> 1i8) >> 2u16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_shift_right_chain_then_cast_i16_to_i8_integer_comparison_convergence(
            token: Token, input: i16, enabled: bool
        ) -> bool
        requires -1024i16 <= input, input <= 1023i16
        {
            let staged: bool = ((((input >> 1u8) >> 2i32) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::width_exact_shift_right_chain_then_cast_i8_to_u8_integer_comparison_convergence(
            token: Token, input: i8, enabled: bool
        ) -> bool
        requires 0i8 <= input
        {
            let staged: bool = ((((input >> 4u8) >> 4i16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::width_exact_shift_right_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token, input: u16, enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input >> 8u8) >> 8i16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::runtime_count_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires count <= 7u8
        {
            let staged: bool = (((input >> 1u8) >> count) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let retained: u8 = input;
            let staged: bool = ((((retained >> 1u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_divide_feeds_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((((input / 2u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::right_associated_exact_shift_right_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = ((input >> (input % 8u8)) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        {
            let staged: bool = (((((input >> 1u8) as u16) >> 1u8) >> 1u8) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_left_feeds_shift_right_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = ((((input << 1u8) >> 1u8) >> 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 31u8
        {
            let staged: bool = ((((input << 1i8) << 2u16) << 0i32) < 255u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u16_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0u8) << 0i16) << 0u32) < 5u16) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_u32_integer_comparison_convergence(
            token: Token,
            input: u32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i64) << 0u8) << 0i16) < 5u32) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8, -16i8 <= input, input <= 15i8
        {
            let staged: bool = ((((input << 1u16) << 2i32) << 0u8) < 5i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i16_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i8) << 0u32) << 0i64) < 5i16) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i32_integer_comparison_convergence(
            token: Token,
            input: i32,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0u64) << 0i16) << 0u8) < 5i32) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_i64_integer_comparison_convergence(
            token: Token,
            input: i64,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input << 0i32) << 0u16) << 0u64) < 5i64) && enabled;
            staged
        }
        machine Root::width_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((input << 4u8) << 4i8) < 5u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 127u16, input <= 31u16
        {
            let staged: bool = (((((input as u8) << 1i8) << 2u16) << 0i32) < 255u8) && enabled;
            staged
        }
        machine Root::width_exact_cast_then_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 15u16, input <= 0u16
        {
            let staged: bool = ((((input as u8) << 4u8) << 4i8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -64i16 <= input, input <= 63i16,
            -16i16 <= input, input <= 15i16
        {
            let staged: bool = ((((input as i8) << 1u16) << 2i32) < 127i8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, input <= 31i8
        {
            let staged: bool = ((((input as u8) << 1i8) << 2u16) < 255u8) && enabled;
            staged
        }
        machine Root::exact_cast_then_shift_left_chain_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 63u8, input <= 15u8
        {
            let staged: bool = ((((input as i8) << 1u16) << 2i32) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 32767u16, input <= 8191u16, input <= 31u16
        {
            let staged: bool = (((((input << 1i8) << 2u16) << 0i32) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::width_exact_shift_left_chain_then_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 15u8, input <= 0u8
        {
            let staged: bool = ((((input << 4u8) << 4i8) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -16384i16 <= input, input <= 16383i16,
            -4096i16 <= input, input <= 4095i16,
            -16i16 <= input, input <= 15i16
        {
            let staged: bool = ((((input << 1u16) << 2i32) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -64i8 <= input, input <= 63i8,
            -16i8 <= input, input <= 15i8,
            0i8 <= input, input <= 31i8
        {
            let staged: bool = ((((input << 1i8) << 2u16) as u8) < 255u8) && enabled;
            staged
        }
        machine Root::exact_shift_left_chain_then_cast_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8, input <= 31u8, input <= 15u8
        {
            let staged: bool = ((((input << 1u16) << 2i32) as i8) < 127i8) && enabled;
            staged
        }
        machine Root::runtime_count_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            count: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8, count <= 7u8
        {
            let staged: bool = (((input << 1u8) << count) < 5u8) && enabled;
            staged
        }
        machine Root::computed_count_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((input << 0u8) << (input % 8u8)) < 5u8) && enabled;
            staged
        }
        machine Root::local_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let retained: u8 = input;
            let staged: bool = (((retained << 1u8) << 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::widened_exact_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = (((((input << 1u8) as u16) << 1u8) << 1u8) < 5u16) && enabled;
            staged
        }
        machine Root::exact_add_feeds_shift_left_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 0u8
        {
            let staged: bool = ((((input + 0u8) << 1u8) << 1u8) < 5u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_chain_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 250u8, input <= 251u8
        {
            let staged: bool = ((((input + 5u8) - 3u8) + 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::mixed_exact_add_subtract_chain_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -126i8 <= input, input <= 124i8
        {
            let staged: bool = ((((input - -3i8) + -5i8) - -1i8) < 127i8) && enabled;
            staged
        }
        machine Root::runtime_sibling_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            sibling: u8,
            enabled: bool
        ) -> bool
        requires input <= 254u8, sibling <= input + 1u8
        {
            let staged: bool = (((input + 1u8) - sibling) < 255u8) && enabled;
            staged
        }
        machine Root::right_associated_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires 1u8 <= input, input <= 254u8
        {
            let staged: bool = ((1u8 + (input - 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::local_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let retained: u8 = input;
            let staged: bool = (((retained + 2u8) - 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::widened_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((((input + 1u8) as u16) - 1u16) + 1u16) < 256u16) && enabled;
            staged
        }
        machine Root::multiply_feeds_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = ((((input * 2u8) + 1u8) - 1u8) < 255u8) && enabled;
            staged
        }
        machine Root::reversed_subtract_mixed_exact_add_subtract_chain_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 1u8
        {
            let staged: bool = ((2u8 - (input + 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::two_nested_exact_add_operands_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = (((input + 1u8) + (input + 1u8)) < 255u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_computed_sibling_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 253u8
        {
            let staged: bool = (((input + 1u8) + (input & 0u8)) < 4u8) && enabled;
            staged
        }
        machine Root::nested_exact_add_feeds_multiply_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 126u8
        {
            let staged: bool = (((input + 1u8) * 2u8) < 255u8) && enabled;
            staged
        }
        machine Root::mixed_exact_affine_u8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 124u8
        {
            let staged: bool = (((((input + 3u8) * 2u8) - 1u8) < 255u8) && enabled);
            staged
        }
        machine Root::mixed_exact_affine_i8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -61i8 <= input, input <= 66i8
        {
            let staged: bool = (((((input + -3i8) * 2i8) - -1i8) < 127i8) && enabled);
            staged
        }
        machine Root::zero_factor_mixed_exact_affine_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = (((((input + 3u8) * 0u8) + 255u8) < 255u8) && enabled);
            staged
        }
        machine Root::mixed_exact_affine_chain_cast_u8_to_i8_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8, input <= 124u8, input <= 125u8, input <= 61u8
        {
            let staged: bool = ((((((input + 3u8) * 2u8) - 1u8) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::mixed_exact_affine_chain_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -125i8 <= input, -61i8 <= input, input <= 66i8, 3i8 <= input
        {
            let staged: bool = ((((((input - 3i8) * 2i8) + 1i8) as u8) < 255u8) && enabled);
            staged
        }
        machine Root::zero_factor_mixed_exact_affine_chain_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 252u8
        {
            let staged: bool = ((((((input + 3u8) * 0u8) + 127u8) as i8) < 127i8) && enabled);
            staged
        }
        machine Root::nested_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool
        requires input <= 255u64
        {
            let staged: bool = (((input as u8) as u16) < 4u16) && enabled;
            staged
        }
        machine Root::roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((input as u16) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::nonroundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool
        requires input <= 127u8
        {
            let staged: bool = (((input as u16) as i8) < 4i8) && enabled;
            staged
        }
        machine Root::offset_chain_exact_cast_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 65530u16, input <= 65533u16, input <= 253u16
        {
            let staged: bool = (((((input + 5u16) - 3u16) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::offset_chain_exact_cast_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires input <= 32762i16, input <= 32765i16,
            -130i16 <= input, input <= 125i16
        {
            let staged: bool = (((((input + 5i16) - 3i16) as i8) < 4i8) && enabled);
            staged
        }
        machine Root::offset_chain_exact_cast_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires -127i8 <= input, 1i8 <= input
        {
            let staged: bool = ((((input - 1i8) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = ((((input as u8) + 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_subtract_u16_to_u8_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, 5u16 <= input, input <= 260u16
        {
            let staged: bool = ((((input as u8) - 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_i16_to_i8_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -123i16 <= input, input <= 132i16
        {
            let staged: bool = ((((input as i8) + -5i8) < 127i8) && enabled);
            staged
        }
        machine Root::exact_cast_then_add_i8_to_u8_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, -1i8 <= input
        {
            let staged: bool = ((((input as u8) + 1u8) < 255u8) && enabled);
            staged
        }
        machine Root::reversed_add_after_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = (((5u8 + (input as u8)) < 255u8) && enabled);
            staged
        }
        machine Root::local_exact_cast_then_add_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let retained: u16 = input;
            let staged: bool = ((((retained as u8) + 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::nested_exact_cast_then_add_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 254u16, input <= 253u16
        {
            let staged: bool = (((((input as u8) + 1u8) + 1u8) < 255u8) && enabled);
            staged
        }
        machine Root::mixed_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16,
            input <= 253u16, input <= 251u16
        {
            let staged: bool = ((((((input as u8) + 5u8) - 3u8) + 2u8) < 255u8) && enabled);
            staged
        }
        machine Root::cancelling_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 255u16, input <= 250u16
        {
            let staged: bool = (((((input as u8) + 5u8) - 5u8) < 255u8) && enabled);
            staged
        }
        machine Root::signed_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: i16,
            enabled: bool
        ) -> bool
        requires -128i16 <= input, input <= 127i16,
            -123i16 <= input, input <= 132i16,
            -120i16 <= input, input <= 135i16
        {
            let staged: bool = (((((input as i8) + -5i8) - 3i8) < 127i8) && enabled);
            staged
        }
        machine Root::cross_sign_exact_cast_then_offset_chain_integer_comparison_convergence(
            token: Token,
            input: i8,
            enabled: bool
        ) -> bool
        requires 0i8 <= input, -3i8 <= input, -1i8 <= input
        {
            let staged: bool = (((((input as u8) + 3u8) - 2u8) < 255u8) && enabled);
            staged
        }
        machine Root::right_associated_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires 1u16 <= input, input <= 255u16
        {
            let staged: bool = ((((1u16 + (input - 1u16)) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::local_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 254u16
        {
            let retained: u16 = input;
            let staged: bool = ((((retained + 1u16) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::reversed_subtract_offset_chain_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u16,
            enabled: bool
        ) -> bool
        requires input <= 3u16
        {
            let staged: bool = ((((3u16 - input) as u8) < 4u8) && enabled);
            staged
        }
        machine Root::local_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let retained: u8 = input;
            let staged: bool = (((retained as u16) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::multistep_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = ((((input as u16) as u32) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::deep_roundtrip_computed_exact_cast_integer_comparison_convergence(
            token: Token,
            input: u8,
            enabled: bool
        ) -> bool {
            let staged: bool = (((((input as u16) as u32) as u64) as u8) < 4u8) && enabled;
            staged
        }
        machine Root::member_integer_comparison_convergence(
            token: Token,
            input: u64,
            enabled: bool
        ) -> bool {
            let staged: bool = token.observed && ((input < 1u64) || enabled);
            staged
        }
        machine Root::short_circuit_return_expression(token: Token) -> bool {
            let staged: bool = true && false;
            !staged
        }
        machine Root::short_circuit_continuation_local(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            inverted
        }
        machine Root::reused_short_circuit_return(token: Token) -> bool {
            let staged: bool = true && false;
            staged == staged
        }
        machine Root::two_continuation_locals(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            let restored: bool = !inverted;
            restored
        }
        machine Root::three_continuation_locals(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            let restored: bool = !inverted;
            let inverted_again: bool = !restored;
            inverted_again
        }
        machine Root::repeated_short_circuit_locals(token: Token) -> bool {
            let first: bool = true && false;
            let second: bool = first || true;
            second
        }
        machine Root::nested_short_circuit(token: Token) -> bool {
            true && (false || true)
        }
        machine Root::repeated_short_circuit(token: Token) -> bool {
            (true && false) || true
        }
        machine Root::nested_short_circuit_locals(token: Token) -> bool {
            let staged: bool = true && (false || true);
            let repeated: bool = staged || (true && false);
            repeated
        }
        machine Root::mutable_local(token: Token) -> u64 {
            let mut staged: u64 = 1u64;
            staged
        }
        machine Root::call_local(token: Token) -> u64 {
            let staged: u64 = Helper::value();
            staged
        }
        machine Root::effect_before_return(token: Token) -> u64 {
            Helper::touch();
            1u64
        }
        "#,
    );
    let short_circuit = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit"))
        .expect("one final short-circuit local returned directly retains cleanup");
    assert_eq!(short_circuit.bindings.len(), 1);
    assert_eq!(short_circuit.return_statement_ordinal, 1);
    assert!(short_circuit.shared_boolean_convergence.is_none());
    let shared_convergence = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "shared_convergence"))
        .expect("one direct Boolean decision should publish shared convergence eligibility");
    assert_eq!(shared_convergence.bindings.len(), 1);
    assert_eq!(
        shared_convergence
            .shared_boolean_convergence
            .expect("shared convergence marker")
            .binding_ordinal,
        0
    );
    let member_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "member_integer_comparison_convergence",
        ));
    assert!(member_integer_comparison.is_none());
    let nested_shared_convergence = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "nested_shared_convergence"))
        .expect("one-input nested Boolean tree should retain a shared convergence plan");
    assert_eq!(
        nested_shared_convergence
            .shared_boolean_convergence
            .expect("nested shared convergence marker")
            .binding_ordinal,
        0
    );
    let computed_leaf = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "computed_leaf_convergence"))
        .expect("negated Boolean leaves retain the shared convergence plan");
    assert_eq!(
        computed_leaf
            .shared_boolean_convergence
            .expect("negated shared convergence marker")
            .binding_ordinal,
        0
    );
    for machine in [
        "comparison_leaf_convergence",
        "reversed_comparison_leaf_convergence",
    ] {
        let comparison_leaf = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one-input Boolean comparison leaf retains the scalar-return plan");
        assert_eq!(
            comparison_leaf
                .shared_boolean_convergence
                .expect("normalizable comparison leaf publishes shared convergence")
                .binding_ordinal,
            0
        );
    }
    let multiple_inputs = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "multiple_input_convergence"))
        .expect("multiple-input Boolean tree retains its scalar-return plan");
    assert_eq!(
        multiple_inputs
            .shared_boolean_convergence
            .expect("finite multiple-input tree publishes shared convergence")
            .binding_ordinal,
        0
    );
    let multiple_input_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "multiple_input_comparison_convergence",
        ))
        .expect("two-runtime-side equality retains the source-distributed fallback");
    assert!(
        multiple_input_comparison
            .shared_boolean_convergence
            .is_none()
    );
    let integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "integer_comparison_convergence"))
        .expect("integer comparison retains the scalar-return plan");
    assert_eq!(
        integer_comparison
            .shared_boolean_convergence
            .expect("integer comparison publishes shared convergence")
            .binding_ordinal,
        0
    );
    let computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "computed_integer_comparison_convergence",
        ))
        .expect("one computed integer shell retains the scalar-return plan");
    assert!(
        computed_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_computed_integer_comparison_convergence",
        ))
        .expect("two total integer shells retain the scalar-return plan");
    assert!(
        nested_computed_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let triple_computed_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "triple_computed_integer_comparison_convergence",
        ))
        .expect("three total integer shells retain the source-distributed fallback");
    assert!(
        triple_computed_integer_comparison
            .shared_boolean_convergence
            .is_none()
    );
    let bitwise_not_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "bitwise_not_integer_comparison_convergence",
        ))
        .expect("one bitwise-not shell retains the scalar-return plan");
    assert!(
        bitwise_not_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_bitwise_not_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_bitwise_not_integer_comparison_convergence",
        ))
        .expect("two bitwise-not shells retain the scalar-return plan");
    assert!(
        nested_bitwise_not_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let widened_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_integer_comparison_convergence",
        ))
        .expect("one integer-widening shell retains the scalar-return plan");
    assert!(
        widened_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_widened_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_widened_integer_comparison_convergence",
        ))
        .expect("two integer-widening shells retain the scalar-return plan");
    assert!(
        nested_widened_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_cast_integer_comparison_convergence",
        ))
        .expect("one guarded exact-cast shell retains the scalar-return plan");
    assert!(
        exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_exact_cast_integer_comparison_convergence",
        ))
        .expect("one signed exact-cast shell retains the scalar-return plan");
    assert!(
        signed_exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "unsigned_to_signed_exact_cast_integer_comparison_convergence",
        "signed_to_unsigned_exact_cast_integer_comparison_convergence",
    ] {
        let cross_sign_exact_cast_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one bounded cross-sign exact-cast shell retains the scalar-return plan");
        assert!(
            cross_sign_exact_cast_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "signed_positive_exact_add_integer_comparison_convergence",
        "signed_negative_exact_add_integer_comparison_convergence",
        "signed_positive_exact_subtract_integer_comparison_convergence",
        "signed_negative_exact_subtract_integer_comparison_convergence",
        "signed_positive_exact_multiply_integer_comparison_convergence",
        "signed_negative_exact_multiply_integer_comparison_convergence",
    ] {
        let signed_exact_add_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one bounded signed exact-arithmetic shell retains the scalar-return plan");
        assert!(
            signed_exact_add_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_add_integer_comparison_convergence",
        ))
        .expect("one proof-bearing exact-add shell retains the scalar-return plan");
    assert!(
        exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_add_integer_comparison_convergence",
        ))
        .expect("one computed-bound runtime exact-add shell retains the scalar-return plan");
    assert!(
        runtime_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_multiply_integer_comparison_convergence",
        ))
        .expect("one computed-bound runtime exact-multiply shell retains the scalar-return plan");
    assert!(
        runtime_exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "runtime_signed_positive_exact_multiply_integer_comparison_convergence",
        "runtime_signed_negative_exact_multiply_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_multiply_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed quotient-bound runtime exact-multiply shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_multiply_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_signed_positive_exact_add_integer_comparison_convergence",
        "runtime_signed_negative_exact_add_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_add_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed computed-bound runtime exact-add shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_add_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_signed_positive_exact_subtract_integer_comparison_convergence",
        "runtime_signed_negative_exact_subtract_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_subtract_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect(
                "one signed computed-bound runtime exact-subtract shell retains the scalar-return plan",
            );
        assert!(
            runtime_signed_exact_subtract_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_subtract_integer_comparison_convergence",
        ))
        .expect("one bounded exact-subtract shell retains the scalar-return plan");
    assert!(
        exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_subtract_integer_comparison_convergence",
        ))
        .expect("one relationally proven exact-subtract shell retains the scalar-return plan");
    assert!(
        runtime_exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_multiply_integer_comparison_convergence",
        ))
        .expect("one bounded exact-multiply shell retains the scalar-return plan");
    assert!(
        exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_divide_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_divide_integer_comparison_convergence",
        ))
        .expect("one constant-divisor exact-divide shell retains the scalar-return plan");
    assert!(
        exact_divide_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_remainder_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_remainder_integer_comparison_convergence",
        ))
        .expect("one constant-divisor exact-remainder shell retains the scalar-return plan");
    assert!(
        exact_remainder_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "signed_exact_divide_integer_comparison_convergence",
        "signed_exact_remainder_integer_comparison_convergence",
    ] {
        let signed_exact_division_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one landed safe signed-divisor shell retains the scalar-return plan");
        assert!(
            signed_exact_division_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let runtime_exact_divide_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_divide_integer_comparison_convergence",
        ))
        .expect("one proven runtime-divisor exact-divide shell retains the scalar-return plan");
    assert!(
        runtime_exact_divide_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "runtime_signed_exact_divide_integer_comparison_convergence",
        "runtime_signed_exact_remainder_integer_comparison_convergence",
        "runtime_negative_signed_exact_divide_integer_comparison_convergence",
        "runtime_negative_signed_exact_remainder_integer_comparison_convergence",
        "runtime_bounded_negative_signed_exact_divide_integer_comparison_convergence",
        "runtime_bounded_negative_signed_exact_remainder_integer_comparison_convergence",
    ] {
        let runtime_signed_exact_division_integer_comparison = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("one positive signed runtime-divisor shell retains the scalar-return plan");
        assert!(
            runtime_signed_exact_division_integer_comparison
                .shared_boolean_convergence
                .is_some()
        );
    }
    let exact_shift_right_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_shift_right_integer_comparison_convergence",
        ))
        .expect("one bounded exact-right-shift shell retains the scalar-return plan");
    assert!(
        exact_shift_right_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_count_exact_shift_right_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_count_exact_shift_right_integer_comparison_convergence",
        ))
        .expect("one signed-count exact-right-shift shell retains the scalar-return plan");
    assert!(
        signed_count_exact_shift_right_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one bounded exact-left-shift shell retains the scalar-return plan");
    assert!(
        exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let runtime_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one proven runtime exact-left-shift shell retains the scalar-return plan");
    assert!(
        runtime_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_count_runtime_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_count_runtime_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one signed-count runtime exact-left-shift shell retains the scalar-return plan");
    assert!(
        signed_count_runtime_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let signed_value_exact_shift_left_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "signed_value_exact_shift_left_integer_comparison_convergence",
        ))
        .expect("one signed-value exact-left-shift shell retains the scalar-return plan");
    assert!(
        signed_value_exact_shift_left_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let bitwise_not_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "bitwise_not_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add shell beneath bitwise-not retains the scalar-return plan");
    assert!(
        bitwise_not_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let widen_exact_subtract_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widen_exact_subtract_integer_comparison_convergence",
        ))
        .expect("one exact-subtract shell beneath widening retains the scalar-return plan");
    assert!(
        widen_exact_subtract_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let binary_right_exact_multiply_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "binary_right_exact_multiply_integer_comparison_convergence",
        ))
        .expect("one exact-multiply right subtree beneath bitwise-and retains the scalar plan");
    assert!(
        binary_right_exact_multiply_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let two_shell_nested_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "two_shell_nested_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add shell beneath widening and bitwise-not retains the scalar plan");
    assert!(
        two_shell_nested_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let sibling_exact_operations_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "sibling_exact_operations_integer_comparison_convergence",
        ))
        .expect("sibling exact-add and exact-multiply leaves retain the scalar plan");
    assert!(
        sibling_exact_operations_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let nested_exact_add_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_exact_add_integer_comparison_convergence",
        ))
        .expect("one exact-add result may feed one exact-add shell");
    assert!(
        nested_exact_add_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let same_root_affine_fork = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "two_nested_exact_add_operands_integer_comparison_convergence",
        ))
        .expect("two independently landed affine branches retain the scalar-return plan");
    assert!(same_root_affine_fork.shared_boolean_convergence.is_some());
    for machine in [
        "nested_exact_add_computed_sibling_integer_comparison_convergence",
        "local_exact_add_chain_integer_comparison_convergence",
    ] {
        let wider_nested_exact_add = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .expect("wider exact-add composition retains only the source-distributed fallback");
        assert!(wider_nested_exact_add.shared_boolean_convergence.is_none());
    }
    let affine_exact_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_exact_add_feeds_multiply_integer_comparison_convergence",
        ))
        .expect("a finite exact affine chain retains the scalar-return plan");
    assert!(affine_exact_chain.shared_boolean_convergence.is_some());
    let deep_nested_exact_add = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_nested_exact_add_integer_comparison_convergence",
        ))
        .expect("a finite exact-add chain retains the scalar-return plan");
    assert!(deep_nested_exact_add.shared_boolean_convergence.is_some());
    let deep_nested_exact_subtract = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_nested_exact_subtract_integer_comparison_convergence",
        ))
        .expect("a finite exact-subtract chain retains the scalar-return plan");
    assert!(
        deep_nested_exact_subtract
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "reversed_nested_exact_subtract_integer_comparison_convergence",
        "local_exact_subtract_chain_integer_comparison_convergence",
    ] {
        let wider_nested_exact_subtract = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "wider exact-subtract composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            wider_nested_exact_subtract
                .shared_boolean_convergence
                .is_none()
        );
    }
    let cancelling_mixed_exact_add_subtract = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "mixed_exact_add_subtract_integer_comparison_convergence",
        ))
        .expect("the cancelling mixed exact-add/subtract chain retains its scalar-return plan");
    assert!(
        cancelling_mixed_exact_add_subtract
            .shared_boolean_convergence
            .is_some()
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(
                &checked,
                "nested_exact_subtract_computed_sibling_integer_comparison_convergence",
            ))
            .is_none(),
        "a computed subtraction sibling remains outside the terminal scalar-return plan"
    );
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine =
            format!("mixed_exact_divide_remainder_chain_{carrier}_integer_comparison_convergence");
        let divide_remainder_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} finite mixed exact-divide/remainder chain retains the scalar-return plan"
                )
            });
        assert!(divide_remainder_chain.shared_boolean_convergence.is_some());
    }
    let exact_add_feeds_divide_remainder = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_add_feeds_divide_remainder_chain_integer_comparison_convergence",
        ))
        .expect("the direct affine-to-divide/remainder chain retains its scalar-return plan");
    assert!(
        exact_add_feeds_divide_remainder
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "local_exact_divide_remainder_chain_integer_comparison_convergence",
        "computed_right_exact_divide_integer_comparison_convergence",
        "signed_negative_one_exact_divide_chain_integer_comparison_convergence",
    ] {
        let fenced_divide_remainder_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-divide/remainder composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            fenced_divide_remainder_chain
                .shared_boolean_convergence
                .is_none()
        );
    }
    let runtime_divisor_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "runtime_divisor_exact_divide_chain_integer_comparison_convergence",
        ))
        .expect("the direct runtime-divisor chain retains its scalar-return plan");
    assert!(runtime_divisor_chain.shared_boolean_convergence.is_some());
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_multiply_chain_{carrier}_integer_comparison_convergence");
        let multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!("the {carrier} finite exact-multiply chain retains the scalar-return plan")
            });
        assert!(multiply_chain.shared_boolean_convergence.is_some());
    }
    let zero_factor_multiply_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "zero_factor_exact_multiply_chain_integer_comparison_convergence",
        ))
        .expect("a later zero factor retains every exact-multiply link");
    assert!(
        zero_factor_multiply_chain
            .shared_boolean_convergence
            .is_some()
    );
    let negative_factor_multiply_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "negative_factor_exact_multiply_chain_integer_comparison_convergence",
        ))
        .expect("the finite signed exact-multiply chain retains its scalar-return plan");
    assert!(
        negative_factor_multiply_chain
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "exact_cast_then_multiply_chain_u16_to_u8_integer_comparison_convergence",
        "zero_factor_exact_cast_then_multiply_chain_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_i8_to_u8_integer_comparison_convergence",
        "exact_cast_then_multiply_chain_u8_to_i8_integer_comparison_convergence",
    ] {
        let cast_then_multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("post-cast exact-multiply chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            cast_then_multiply_chain
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "exact_multiply_chain_then_cast_u16_to_u8_integer_comparison_convergence",
        "zero_factor_exact_multiply_chain_then_cast_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_i16_to_i8_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_i8_to_u8_integer_comparison_convergence",
        "exact_multiply_chain_then_cast_u8_to_i8_integer_comparison_convergence",
    ] {
        let multiply_chain_then_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("pre-cast exact-multiply chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            multiply_chain_then_cast
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_factor_exact_multiply_chain_integer_comparison_convergence",
        "reversed_exact_multiply_chain_integer_comparison_convergence",
        "local_exact_multiply_chain_integer_comparison_convergence",
        "two_computed_exact_multiply_operands_integer_comparison_convergence",
    ] {
        let fenced_multiply_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-multiply composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(fenced_multiply_chain.shared_boolean_convergence.is_none());
    }
    let widened_multiply_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_exact_multiply_chain_integer_comparison_convergence",
        ))
        .expect("the affine-widen-affine cohort retains its scalar-return plan");
    assert!(
        widened_multiply_chain.shared_boolean_convergence.is_some(),
        "strict widening now joins independently proved source and target affine chains",
    );
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_shift_right_chain_{carrier}_integer_comparison_convergence");
        let shift_right_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} finite exact-shift-right chain retains the scalar-return plan"
                )
            });
        assert!(shift_right_chain.shared_boolean_convergence.is_some());
    }
    for machine in [
        "exact_shift_right_chain_then_cast_u16_to_u8_integer_comparison_convergence",
        "exact_shift_right_chain_then_cast_i16_to_i8_integer_comparison_convergence",
        "width_exact_shift_right_chain_then_cast_i8_to_u8_integer_comparison_convergence",
        "width_exact_shift_right_chain_then_cast_u16_to_u8_integer_comparison_convergence",
    ] {
        let shift_right_chain_then_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| panic!("pre-cast exact-right-shift chain `{machine}` retained"));
        assert!(
            checked
                .facts
                .values
                .scalar_expressions
                .expression_at(
                    shift_right_chain_then_cast.state,
                    0,
                    CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
                )
                .is_some(),
            "pre-cast right-shift chain `{machine}` retains its checked local occurrence"
        );
        assert!(
            shift_right_chain_then_cast
                .shared_boolean_convergence
                .is_some(),
            "pre-cast right-shift chain `{machine}` retains convergence"
        );
    }
    let exact_divide_feeds_shift_right = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_divide_feeds_shift_right_chain_integer_comparison_convergence",
        ))
        .expect("the direct divide/remainder-to-shift chain retains its scalar-return plan");
    assert!(
        exact_divide_feeds_shift_right
            .shared_boolean_convergence
            .is_some()
    );
    for machine in [
        "runtime_count_exact_shift_right_chain_integer_comparison_convergence",
        "local_exact_shift_right_chain_integer_comparison_convergence",
        "right_associated_exact_shift_right_integer_comparison_convergence",
    ] {
        let fenced_shift_right_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-shift-right composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(
            fenced_shift_right_chain
                .shared_boolean_convergence
                .is_none()
        );
    }
    let widened_shift_right_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_exact_shift_right_chain_integer_comparison_convergence",
        ))
        .expect("the shift-widen-shift cohort retains its scalar-return plan");
    assert!(
        widened_shift_right_chain
            .shared_boolean_convergence
            .is_some(),
        "strict widening now joins independently proved source and target shift chains",
    );
    let mixed_shift_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_shift_left_feeds_shift_right_chain_integer_comparison_convergence",
        ))
        .expect("the left-then-right exact-shift chain retains its scalar-return plan");
    assert!(mixed_shift_chain.shared_boolean_convergence.is_some());
    for carrier in ["u8", "u16", "u32", "i8", "i16", "i32", "i64"] {
        let machine = format!("exact_shift_left_chain_{carrier}_integer_comparison_convergence");
        let shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!("the {carrier} finite exact-shift-left chain retains the scalar-return plan")
            });
        assert!(shift_left_chain.shared_boolean_convergence.is_some());
    }
    let width_shift_left_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "width_exact_shift_left_chain_integer_comparison_convergence",
        ))
        .expect("a cumulative carrier-width shift retains the zero-only root bound");
    assert!(width_shift_left_chain.shared_boolean_convergence.is_some());
    for machine in [
        "exact_cast_then_shift_left_chain_u16_to_u8_integer_comparison_convergence",
        "width_exact_cast_then_shift_left_chain_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_i8_to_u8_integer_comparison_convergence",
        "exact_cast_then_shift_left_chain_u8_to_i8_integer_comparison_convergence",
    ] {
        let cast_then_shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "post-cast exact-left-shift chain `{machine}` retains its scalar-return plan"
                )
            });
        assert!(
            cast_then_shift_left_chain
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "exact_shift_left_chain_then_cast_u16_to_u8_integer_comparison_convergence",
        "width_exact_shift_left_chain_then_cast_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_i16_to_i8_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_i8_to_u8_integer_comparison_convergence",
        "exact_shift_left_chain_then_cast_u8_to_i8_integer_comparison_convergence",
    ] {
        let shift_left_chain_then_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("pre-cast exact-left-shift chain `{machine}` retains its scalar-return plan")
            });
        assert!(
            shift_left_chain_then_cast
                .shared_boolean_convergence
                .is_some()
        );
    }
    for machine in [
        "runtime_count_exact_shift_left_chain_integer_comparison_convergence",
        "computed_count_exact_shift_left_chain_integer_comparison_convergence",
        "local_exact_shift_left_chain_integer_comparison_convergence",
    ] {
        let fenced_shift_left_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "fenced exact-shift-left composition `{machine}` retains only source-distributed fallback"
                )
            });
        assert!(fenced_shift_left_chain.shared_boolean_convergence.is_none());
    }
    let widened_shift_left_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_exact_shift_left_chain_integer_comparison_convergence",
        ))
        .expect("the shift-widen-shift cohort retains its scalar-return plan");
    assert!(
        widened_shift_left_chain
            .shared_boolean_convergence
            .is_some(),
        "strict widening now joins independently proved source and target shift chains",
    );
    let arithmetic_then_shift = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "exact_add_feeds_shift_left_chain_integer_comparison_convergence",
        ))
        .expect("the arithmetic-prefix exact-left-shift chain retains its scalar-return plan");
    assert!(arithmetic_then_shift.shared_boolean_convergence.is_some());
    for carrier in ["u8", "i8"] {
        let machine =
            format!("mixed_exact_add_subtract_chain_{carrier}_integer_comparison_convergence");
        let mixed_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, &machine))
            .unwrap_or_else(|| {
                panic!(
                    "the {carrier} mixed exact-add/subtract chain retains its scalar-return plan"
                )
            });
        assert!(mixed_chain.shared_boolean_convergence.is_some());
    }
    for machine in [
        "runtime_sibling_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "right_associated_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "local_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "reversed_subtract_mixed_exact_add_subtract_chain_integer_comparison_convergence",
    ] {
        let fenced_mixed_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(fenced_mixed_chain.is_none_or(|plan| plan.shared_boolean_convergence.is_none()));
    }
    let widened_mixed_affine_chain = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "widened_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        ))
        .expect("the affine-widen-affine cohort retains its scalar-return plan");
    assert!(
        widened_mixed_affine_chain
            .shared_boolean_convergence
            .is_some(),
        "strict widening now joins independently proved source and target affine chains",
    );
    for machine in [
        "nested_exact_add_feeds_multiply_integer_comparison_convergence",
        "nested_exact_subtract_feeds_multiply_integer_comparison_convergence",
        "exact_add_feeds_multiply_chain_integer_comparison_convergence",
        "multiply_feeds_mixed_exact_add_subtract_chain_integer_comparison_convergence",
        "mixed_exact_affine_u8_integer_comparison_convergence",
        "mixed_exact_affine_i8_integer_comparison_convergence",
        "zero_factor_mixed_exact_affine_integer_comparison_convergence",
        "mixed_exact_affine_chain_cast_u8_to_i8_integer_comparison_convergence",
        "mixed_exact_affine_chain_cast_i8_to_u8_integer_comparison_convergence",
        "zero_factor_mixed_exact_affine_chain_cast_integer_comparison_convergence",
    ] {
        let affine_chain = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| panic!("finite exact-affine chain `{machine}` retains its plan"));
        assert!(
            affine_chain.shared_boolean_convergence.is_some(),
            "finite exact-affine chain `{machine}` retains shared convergence"
        );
    }
    let nested_exact_cast_integer_comparison = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nested_exact_cast_integer_comparison_convergence",
        ))
        .expect("one exact-cast shell beneath widening retains the scalar-return plan");
    assert!(
        nested_exact_cast_integer_comparison
            .shared_boolean_convergence
            .is_some()
    );
    let roundtrip_computed_exact_cast = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("one direct widen-then-narrow round trip retains the scalar-return plan");
    assert!(
        roundtrip_computed_exact_cast
            .shared_boolean_convergence
            .is_some()
    );
    let nonroundtrip_computed_exact_cast = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "nonroundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("a wider computed exact cast retains only the source-distributed fallback");
    assert!(
        nonroundtrip_computed_exact_cast
            .shared_boolean_convergence
            .is_none()
    );
    for machine in [
        "offset_chain_exact_cast_u16_to_u8_integer_comparison_convergence",
        "offset_chain_exact_cast_i16_to_i8_integer_comparison_convergence",
        "offset_chain_exact_cast_i8_to_u8_integer_comparison_convergence",
    ] {
        let offset_chain_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!(
                    "computed offset-chain exact cast `{machine}` retains its scalar-return plan"
                )
            });
        assert!(offset_chain_cast.shared_boolean_convergence.is_some());
    }
    for machine in [
        "exact_cast_then_add_u16_to_u8_integer_comparison_convergence",
        "exact_cast_then_subtract_u16_to_u8_integer_comparison_convergence",
        "exact_cast_then_add_i16_to_i8_integer_comparison_convergence",
        "exact_cast_then_add_i8_to_u8_integer_comparison_convergence",
        "nested_exact_cast_then_add_integer_comparison_convergence",
        "mixed_exact_cast_then_offset_chain_integer_comparison_convergence",
        "cancelling_exact_cast_then_offset_chain_integer_comparison_convergence",
        "signed_exact_cast_then_offset_chain_integer_comparison_convergence",
        "cross_sign_exact_cast_then_offset_chain_integer_comparison_convergence",
    ] {
        let cast_then_offset = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("direct exact cast then landed offset `{machine}` retains its scalar-return plan")
            });
        assert!(cast_then_offset.shared_boolean_convergence.is_some());
    }
    for machine in [
        "reversed_add_after_exact_cast_integer_comparison_convergence",
        "local_exact_cast_then_add_integer_comparison_convergence",
    ] {
        let fenced_cast_then_offset = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(
            fenced_cast_then_offset.is_none_or(|plan| plan.shared_boolean_convergence.is_none()),
            "fenced exact-cast-then-offset composition `{machine}` must fail closed"
        );
    }
    for machine in [
        "right_associated_offset_chain_exact_cast_integer_comparison_convergence",
        "local_offset_chain_exact_cast_integer_comparison_convergence",
        "reversed_subtract_offset_chain_exact_cast_integer_comparison_convergence",
    ] {
        let fenced_offset_chain_cast = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine));
        assert!(
            fenced_offset_chain_cast.is_none_or(|plan| plan.shared_boolean_convergence.is_none()),
            "fenced computed offset-chain exact cast `{machine}` must fail closed"
        );
    }
    let local_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "local_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("a local round trip retains only the source-distributed fallback");
    assert!(local_roundtrip.shared_boolean_convergence.is_none());
    let multistep_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "multistep_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("two direct widening steps retain the scalar-return plan");
    assert!(multistep_roundtrip.shared_boolean_convergence.is_some());
    let deep_roundtrip = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(
            &checked,
            "deep_roundtrip_computed_exact_cast_integer_comparison_convergence",
        ))
        .expect("the complete finite widening chain retains the scalar-return plan");
    assert!(deep_roundtrip.shared_boolean_convergence.is_some());
    let member = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "member_convergence"))
        .expect("one direct Boolean member retains the scalar-return plan");
    assert!(member.shared_boolean_convergence.is_some());
    let repeated_member = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "repeated_member_convergence"))
        .expect("one direct Boolean member may be reused with a scalar input");
    assert!(repeated_member.shared_boolean_convergence.is_some());
    let member_only = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "member_only_convergence"))
        .expect("a field-only expression retains the source-distributed plan");
    assert!(member_only.shared_boolean_convergence.is_none());
    let multiple_members = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "multiple_member_convergence"))
        .expect("multiple direct Boolean members retain only the source-distributed plan");
    assert!(multiple_members.shared_boolean_convergence.is_none());
    let return_expression = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit_return_expression"))
        .expect("one branch-free return expression may consume the final short-circuit local");
    assert_eq!(return_expression.bindings.len(), 1);
    assert_eq!(return_expression.return_statement_ordinal, 1);
    let continuation_local = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "short_circuit_continuation_local"))
        .expect("one branch-free continuation local may consume the short-circuit local");
    assert_eq!(continuation_local.bindings.len(), 2);
    assert_eq!(continuation_local.return_statement_ordinal, 2);
    let reused_return = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "reused_short_circuit_return"))
        .expect("one branch-free return expression may reuse the short-circuit local");
    assert_eq!(reused_return.bindings.len(), 1);
    assert_eq!(reused_return.return_statement_ordinal, 1);
    let repeated_short_circuit_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "repeated_short_circuit_locals"))
        .expect("a later short-circuit stage may consume the preceding Boolean local");
    assert_eq!(repeated_short_circuit_locals.bindings.len(), 2);
    assert_eq!(repeated_short_circuit_locals.return_statement_ordinal, 2);
    let two_continuation_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "two_continuation_locals"))
        .expect("two branch-free continuation locals may consume the short-circuit local in order");
    assert_eq!(two_continuation_locals.bindings.len(), 3);
    assert_eq!(two_continuation_locals.return_statement_ordinal, 3);
    let three_continuation_locals = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "three_continuation_locals"))
        .expect("a finite branch-free continuation chain may consume the short-circuit local");
    assert_eq!(three_continuation_locals.bindings.len(), 4);
    assert_eq!(three_continuation_locals.return_statement_ordinal, 4);

    for (machine, binding_count) in [
        ("nested_short_circuit", 0),
        ("repeated_short_circuit", 0),
        ("nested_short_circuit_locals", 2),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| {
                panic!("`{machine}` should retain arbitrary nested short-circuit cleanup")
            });
        assert_eq!(plan.bindings.len(), binding_count);
        assert_eq!(
            usize::try_from(plan.return_statement_ordinal).unwrap(),
            binding_count
        );
    }

    for machine in ["mutable_local", "call_local", "effect_before_return"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside nominal scalar cleanup with finite locals",
        );
    }
}
