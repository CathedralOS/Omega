//! Exact cast, arithmetic, shift, and divide/remainder classifier matrices.

use super::*;

#[test]
fn exact_shift_left_chain_classifier_covers_u64_and_fences_other_exact_roots() {
    let count = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
                count(1i64, numerics::literals::LandedIntegerType::U8),
            ),
            count(2i64, numerics::literals::LandedIntegerType::U16),
        ),
        count(3i64, numerics::literals::LandedIntegerType::U32),
    );
    assert_eq!(
        exact_shift_left_chain_runtime_parameter_positions_for_test(&u64_chain, 1),
        Some(vec![0])
    );

    let shifted_root = CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactShiftRight,
        primitive_type: PrimitiveType::U64,
        left: Box::new(parameter()),
        right: Box::new(count(1i64, numerics::literals::LandedIntegerType::U8)),
    };
    let fenced = shift_left(
        shift_left(
            shifted_root,
            count(0i64, numerics::literals::LandedIntegerType::U8),
        ),
        count(0i64, numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_shift_left_chain_runtime_parameter_positions_for_test(&fenced, 1),
        None
    );
}

#[test]
fn mixed_exact_add_subtract_chain_classifier_is_left_associated_and_same_carrier() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
                literal(5i64, numerics::literals::LandedIntegerType::U64),
            ),
            literal(3i64, numerics::literals::LandedIntegerType::U64),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&mixed, 1),
        Some(vec![0])
    );

    let right_associated = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        literal(2i64, numerics::literals::LandedIntegerType::U64),
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            parameter(),
            literal(1i64, numerics::literals::LandedIntegerType::U64),
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
            literal(1i64, numerics::literals::LandedIntegerType::I64),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U64),
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
            literal(1i64, numerics::literals::LandedIntegerType::U64),
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
            literal(1i64, numerics::literals::LandedIntegerType::U64),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(&all_add, 1),
        None
    );
}

#[test]
fn offset_chain_exact_cast_classifier_requires_one_direct_same_carrier_left_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(5i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_offset_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::U8, &mixed, 1,),
        Some(vec![0])
    );
    let one_add = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        parameter(0, PrimitiveType::I8),
        literal(-1i64, numerics::literals::LandedIntegerType::I8),
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
        literal(5i64, numerics::literals::LandedIntegerType::U16),
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(3i64, numerics::literals::LandedIntegerType::U16),
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
        literal(3i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::U16),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(2i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::U16),
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
        literal(0i64, numerics::literals::LandedIntegerType::U16),
    );
    assert_eq!(
        exact_multiply_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::I8, &zero, 1,),
        Some(vec![0])
    );
    let signed = operation(
        PrimitiveType::I16,
        parameter(0, PrimitiveType::I16),
        literal(2i64, numerics::literals::LandedIntegerType::I16),
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
        literal(2i64, numerics::literals::LandedIntegerType::U16),
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
        literal(-1i64, numerics::literals::LandedIntegerType::I16),
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
            literal(2i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::U16),
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
            literal(2i64, numerics::literals::LandedIntegerType::U16),
            literal(3i64, numerics::literals::LandedIntegerType::U16),
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
        literal(2i64, numerics::literals::LandedIntegerType::U16),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: checked_trees::CheckedIntegerRange::default(),
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
            literal(5i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&cross_sign, 1),
        Some(vec![0])
    );

    let reversed_add = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        literal(5i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_offset_runtime_parameter_positions_for_test(&nested, 1),
        Some(vec![0])
    );
    let deep_mixed = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::U8,
        nested,
        literal(2i64, numerics::literals::LandedIntegerType::U8),
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
            literal(2i64, numerics::literals::LandedIntegerType::U8),
            literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: checked_trees::CheckedIntegerRange::default(),
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
        literal(2i64, numerics::literals::LandedIntegerType::U8),
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
            literal(3i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(0i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&finite_with_zero, 1),
        Some(vec![0])
    );
    let signed = multiply(
        PrimitiveType::I8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(2i64, numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&signed, 1),
        Some(vec![0])
    );

    let reversed = multiply(
        PrimitiveType::U8,
        literal(2i64, numerics::literals::LandedIntegerType::U8),
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
        literal(-1i64, numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_cast_then_multiply_runtime_parameter_positions_for_test(&negative, 1),
        None
    );
    let mismatched = multiply(
        PrimitiveType::U8,
        cast(PrimitiveType::I16, PrimitiveType::I8, 0),
        literal(2i64, numerics::literals::LandedIntegerType::U8),
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
            literal(2i64, numerics::literals::LandedIntegerType::U8),
            literal(3i64, numerics::literals::LandedIntegerType::U8),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
    };

    let direct = multiply(
        PrimitiveType::I8,
        multiply(
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(-2i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(-3i64, numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_signed_multiply_chain_runtime_parameter_positions_for_test(&direct, 1),
        Some(vec![0]),
    );
    let pre_cast = multiply(
        PrimitiveType::I16,
        parameter(PrimitiveType::I16),
        literal(-2i64, numerics::literals::LandedIntegerType::I16),
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
            literal(-2i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(0i64, numerics::literals::LandedIntegerType::I8),
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
            literal(2i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::I8),
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
                literal(i64::MIN, numerics::literals::LandedIntegerType::I64),
            ),
            literal(i64::MIN, numerics::literals::LandedIntegerType::I64),
        ),
        literal(4i64, numerics::literals::LandedIntegerType::I64),
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
                    literal(0i64, numerics::literals::LandedIntegerType::I64),
                ),
                literal(i64::MIN, numerics::literals::LandedIntegerType::I64),
            ),
            literal(i64::MIN, numerics::literals::LandedIntegerType::I64),
        ),
        literal(4i64, numerics::literals::LandedIntegerType::I64),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let i8_literal = |value| literal(value, numerics::literals::LandedIntegerType::I8);
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

    let i64_literal = |value| literal(value, numerics::literals::LandedIntegerType::I64);
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
        range: checked_trees::CheckedIntegerRange::default(),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let wrap = |source| cast(PrimitiveType::I32, cast(PrimitiveType::U64, source));

    let affine = binary(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I64,
        binary(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I64,
            parameter(PrimitiveType::I64),
            literal(2i64, numerics::literals::LandedIntegerType::I64),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::I64),
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
        literal(-2i64, numerics::literals::LandedIntegerType::I64),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U16),
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
            literal(2i64, numerics::literals::LandedIntegerType::U64),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::U64),
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
        literal(0i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::I64),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
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
            literal(1i64, numerics::literals::LandedIntegerType::I32),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(&affine, 1),
        Some(vec![0]),
    );

    let signed_product = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I32,
        signed_chain(),
        literal(-2i64, numerics::literals::LandedIntegerType::I32),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U16),
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
            literal(2i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::I32),
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
        literal(1i64, numerics::literals::LandedIntegerType::I32),
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
        literal(1i64, numerics::literals::LandedIntegerType::I32),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::I32),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
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
                literal(1i64, numerics::literals::LandedIntegerType::I64),
            )),
        ),
        (
            PrimitiveType::I32,
            signed_cast_chain(binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I64,
                parameter(PrimitiveType::I64),
                literal(-2i64, numerics::literals::LandedIntegerType::I64),
            )),
        ),
        (
            PrimitiveType::I32,
            signed_cast_chain(binary(
                CheckedIntegerBinaryKind::ExactShiftRight,
                PrimitiveType::I64,
                parameter(PrimitiveType::I64),
                literal(1i64, numerics::literals::LandedIntegerType::U8),
            )),
        ),
        (
            PrimitiveType::I8,
            small_signed_cast_chain(binary(
                CheckedIntegerBinaryKind::ExactRemainder,
                PrimitiveType::U32,
                parameter(PrimitiveType::U32),
                literal(3i64, numerics::literals::LandedIntegerType::U32),
            )),
        ),
    ];
    for (target_type, source) in sources {
        let landed_type = match target_type {
            PrimitiveType::I32 => numerics::literals::LandedIntegerType::I32,
            PrimitiveType::I8 => numerics::literals::LandedIntegerType::I8,
            _ => unreachable!("fixture uses signed target carriers"),
        };
        let count_type = numerics::literals::LandedIntegerType::U8;
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
                literal(1i64, numerics::literals::LandedIntegerType::I64),
            ),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::I32),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        )),
        signed_widen_chain(binary(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(-2i64, numerics::literals::LandedIntegerType::I8),
        )),
        signed_widen_chain(binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::I8,
            parameter(PrimitiveType::I8),
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        )),
        unsigned_widen_chain(binary(
            CheckedIntegerBinaryKind::ExactRemainder,
            PrimitiveType::U8,
            parameter(PrimitiveType::U8),
            literal(3i64, numerics::literals::LandedIntegerType::U8),
        )),
    ];
    for source in sources {
        let targets = [
            binary(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I32,
                source.clone(),
                literal(1i64, numerics::literals::LandedIntegerType::I32),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I32,
                source.clone(),
                literal(-2i64, numerics::literals::LandedIntegerType::I32),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactShiftLeft,
                PrimitiveType::I32,
                source.clone(),
                literal(1i64, numerics::literals::LandedIntegerType::U8),
            ),
            binary(
                CheckedIntegerBinaryKind::ExactDivide,
                PrimitiveType::I32,
                source,
                literal(2i64, numerics::literals::LandedIntegerType::I32),
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::I8),
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
                literal(1i64, numerics::literals::LandedIntegerType::I16),
            ),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U32),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let widen_then_cast = |source| cast(PrimitiveType::I16, widen(PrimitiveType::I32, source));
    let cast_then_widen = |source| widen(PrimitiveType::I32, cast(PrimitiveType::I16, source));
    let sources = vec![
        (
            PrimitiveType::I16,
            numerics::literals::LandedIntegerType::I16,
            widen_then_cast(binary(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                literal(1i64, numerics::literals::LandedIntegerType::I16),
            )),
        ),
        (
            PrimitiveType::I16,
            numerics::literals::LandedIntegerType::I16,
            widen_then_cast(binary(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                literal(-2i64, numerics::literals::LandedIntegerType::I16),
            )),
        ),
        (
            PrimitiveType::I16,
            numerics::literals::LandedIntegerType::I16,
            widen_then_cast(binary(
                CheckedIntegerBinaryKind::ExactShiftRight,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                literal(1i64, numerics::literals::LandedIntegerType::U8),
            )),
        ),
        (
            PrimitiveType::I32,
            numerics::literals::LandedIntegerType::I32,
            cast_then_widen(binary(
                CheckedIntegerBinaryKind::ExactRemainder,
                PrimitiveType::U16,
                parameter(PrimitiveType::U16),
                literal(3i64, numerics::literals::LandedIntegerType::U16),
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
                literal(1i64, numerics::literals::LandedIntegerType::U8),
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
                            literal(1i64, numerics::literals::LandedIntegerType::I8),
                        ),
                    ),
                ),
            ),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
                    literal(1i64, numerics::literals::LandedIntegerType::I8),
                ),
            ),
            literal(1i64, numerics::literals::LandedIntegerType::I16),
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
                    literal(1i64, numerics::literals::LandedIntegerType::I16),
                ),
            ),
            literal(1i64, numerics::literals::LandedIntegerType::I8),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = |source_type, target_type, operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: target_type,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: operand,
            primitive_type: source_type,
        }),
        range: checked_trees::CheckedIntegerRange::default(),
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
        literal(1i64, numerics::literals::LandedIntegerType::I8),
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
            literal(2i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(0i64, numerics::literals::LandedIntegerType::I32),
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
            literal(count, numerics::literals::LandedIntegerType::I16),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
            literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = || CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U16,
        }),
        range: checked_trees::CheckedIntegerRange::default(),
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
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
            literal(-1i64, numerics::literals::LandedIntegerType::I8),
        ),
        shift(
            cast(),
            literal(8i64, numerics::literals::LandedIntegerType::U16),
        ),
        shift(
            cast(),
            shift(
                literal(1i64, numerics::literals::LandedIntegerType::U8),
                literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::I8,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = || CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::I8,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I16,
        }),
        range: checked_trees::CheckedIntegerRange::default(),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U8,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
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
        range: checked_trees::CheckedIntegerRange::default(),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
    );
    let mixed = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftLeft,
            PrimitiveType::U8,
            right.clone(),
            literal(1i64, numerics::literals::LandedIntegerType::I32),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U64),
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
                literal(1i64, numerics::literals::LandedIntegerType::I8),
            ),
            literal(2i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::I32),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&alternating, 1),
        Some(vec![0]),
    );
    let partial_cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(parameter(0, PrimitiveType::I16)),
        range: checked_trees::CheckedIntegerRange::default(),
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
                literal(1i64, numerics::literals::LandedIntegerType::I8),
            ),
            literal(2i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::I32),
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
                range: checked_trees::CheckedIntegerRange::default(),
            },
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_mixed_shift_chain_runtime_parameter_positions_for_test(&address, 1),
        None,
    );
}

#[test]
fn exact_shift_cast_shift_classifier_unifies_both_nonempty_sides() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
    );
    let cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(source),
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let accepted = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        shift(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            cast,
            literal(1i64, numerics::literals::LandedIntegerType::I32),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U64),
    );
    assert_eq!(
        exact_shift_cast_shift_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );

    let homogeneous_source = shift(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U16,
        parameter(),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
    );
    let right_only_target = shift(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        CheckedScalarExpression::IntegerExactCast {
            primitive_type: PrimitiveType::U8,
            operand: Box::new(homogeneous_source),
            range: checked_trees::CheckedIntegerRange::default(),
        },
        literal(1i64, numerics::literals::LandedIntegerType::I16),
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
            range: checked_trees::CheckedIntegerRange::default(),
        },
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
    };

    let source_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(),
            literal(1i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
    );
    let affine_cast_shift = binary(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            cast(source_affine),
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U32),
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
            literal(1i64, numerics::literals::LandedIntegerType::I16),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U64),
    );
    let shift_cast_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U8,
            cast(source_shift),
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let source_divide_remainder = || {
        binary(
            CheckedIntegerBinaryKind::ExactRemainder,
            PrimitiveType::U16,
            binary(
                CheckedIntegerBinaryKind::ExactDivide,
                PrimitiveType::U16,
                parameter(),
                literal(2i64, numerics::literals::LandedIntegerType::U16),
            ),
            literal(64i64, numerics::literals::LandedIntegerType::U16),
        )
    };
    let divide_cast_affine = binary(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U8,
            cast(source_divide_remainder()),
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U32),
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
            literal(1i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
    );
    let affine_cast_divide = binary(
        CheckedIntegerBinaryKind::ExactRemainder,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactDivide,
            PrimitiveType::U8,
            cast(source_affine),
            literal(2i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::I16),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U64),
    );
    let shift_cast_remainder = binary(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U8,
        cast(source_shift),
        literal(2i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(&runtime_divisor, 2,),
        None,
    );
    let empty_source = binary(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U8,
        cast(parameter()),
        literal(2i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(&empty_source, 1),
        None,
    );
}

#[test]
fn exact_divide_remainder_cast_sandwich_classifier_requires_both_safe_chains() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
                literal(2i64, numerics::literals::LandedIntegerType::U16),
            ),
            literal(64i64, numerics::literals::LandedIntegerType::U16),
        )
    };
    let cast = |operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(operand),
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let sandwich = binary(
        CheckedIntegerBinaryKind::ExactRemainder,
        PrimitiveType::U8,
        binary(
            CheckedIntegerBinaryKind::ExactDivide,
            PrimitiveType::U8,
            cast(source()),
            literal(2i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cast_sandwich_runtime_parameter_positions_for_test(&sandwich, 1),
        Some(vec![0]),
    );

    let unsafe_target = binary(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U8,
        cast(source()),
        literal(0i64, numerics::literals::LandedIntegerType::U8),
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
        literal(2i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_divide_remainder_cast_sandwich_runtime_parameter_positions_for_test(&empty_source, 1,),
        None,
    );
}

#[test]
fn exact_divide_remainder_cross_chain_classifier_accepts_all_four_compositions() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
    let u8_literal = |value| literal(value, numerics::literals::LandedIntegerType::U8);
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
            literal(1i64, numerics::literals::LandedIntegerType::I16),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U64),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(3i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U8),
    );
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactShiftLeft,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactShiftRight,
            PrimitiveType::U8,
            affine,
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
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
            literal(0i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_arithmetic_then_shift_runtime_parameter_positions_for_test(&homogeneous_offset, 1),
        Some(vec![0]),
    );
    let outer_right = operation(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::U8,
        homogeneous_offset,
        literal(1i64, numerics::literals::LandedIntegerType::I16),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::I8),
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
            literal(2i64, numerics::literals::LandedIntegerType::U8),
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
            literal(-1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_arithmetic_then_shift_runtime_parameter_positions_for_test(&negative_factor, 1),
        None,
    );
}

#[test]
fn exact_shift_then_arithmetic_classifier_unifies_affine_suffix_shapes() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
    );
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U8,
            shift_prefix,
            literal(3i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(-1i64, numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_shift_then_arithmetic_runtime_parameter_positions_for_test(&negative_factor, 1),
        None,
    );
}

#[test]
fn shift_left_chain_exact_cast_classifier_accepts_one_finite_heterogeneous_literal_chain() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
        literal(1i64, numerics::literals::LandedIntegerType::I8),
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
            literal(2i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(0i64, numerics::literals::LandedIntegerType::I32),
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
            literal(count, numerics::literals::LandedIntegerType::I16),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(1i64, numerics::literals::LandedIntegerType::I16),
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
            literal(1i64, numerics::literals::LandedIntegerType::U8),
            literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal(1i64, numerics::literals::LandedIntegerType::U8),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(1i64, numerics::literals::LandedIntegerType::I8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
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
            literal(-1i64, numerics::literals::LandedIntegerType::I8),
        ),
        shift(
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(16i64, numerics::literals::LandedIntegerType::U16),
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
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
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
            literal(2i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(3i64, numerics::literals::LandedIntegerType::U16),
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
        literal(2i64, numerics::literals::LandedIntegerType::U16),
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
                numerics::literals::LandedIntegerType::I16,
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
        literal(3i64, numerics::literals::LandedIntegerType::U8),
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
