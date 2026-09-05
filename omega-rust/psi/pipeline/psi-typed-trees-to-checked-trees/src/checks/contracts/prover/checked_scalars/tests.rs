use super::*;
use psi_numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLanding, IntegerLiteral},
};

fn literal(value: i64, landed_type: LandedIntegerType) -> CheckedScalarExpression {
    CheckedScalarExpression::IntegerLiteral {
        literal: IntegerLiteral::from_value(value).with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    }
}

fn operation(
    kind: CheckedIntegerBinaryKind,
    primitive_type: PrimitiveType,
    left: CheckedScalarExpression,
    right: CheckedScalarExpression,
) -> CheckedScalarExpression {
    CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn expected(value: i64) -> Option<ScalarValue> {
    Some(ScalarValue::Integer(BigInt::from_i64(value)))
}

#[test]
fn checked_integer_operations_use_selected_width_and_policy() {
    use CheckedIntegerBinaryKind::*;
    for (kind, left, right, value) in [
        (ExactAdd, 2, 3, Some(5)),
        (ExactSubtract, 3, 2, Some(1)),
        (ExactMultiply, 3, 2, Some(6)),
        (ExactDivide, 7, 2, Some(3)),
        (ExactRemainder, 7, 2, Some(1)),
        (WrappingAdd, 255, 1, Some(0)),
        (SaturatingAdd, 255, 1, Some(255)),
        (WrappingSubtract, 0, 1, Some(255)),
        (SaturatingSubtract, 0, 1, Some(0)),
        (WrappingMultiply, 128, 2, Some(0)),
        (SaturatingMultiply, 128, 2, Some(255)),
        (WrappingDivide, 7, 2, Some(3)),
        (WrappingRemainder, 7, 2, Some(1)),
        (SaturatingDivide, 7, 2, Some(3)),
        (SaturatingRemainder, 7, 2, Some(1)),
        (BitwiseAnd, 6, 3, Some(2)),
        (BitwiseOr, 6, 3, Some(7)),
        (BitwiseXor, 6, 3, Some(5)),
        (WrappingShiftLeft, 1, 8, Some(1)),
        (WrappingShiftRight, 128, 8, Some(128)),
        (ExactShiftLeft, 1, 7, Some(128)),
        (ExactShiftRight, 128, 7, Some(1)),
        (ExactAdd, 255, 1, None),
        (ExactSubtract, 0, 1, None),
        (ExactMultiply, 128, 2, None),
        (ExactShiftLeft, 128, 1, None),
        (ExactShiftRight, 1, 8, None),
    ] {
        let expression = operation(
            kind,
            PrimitiveType::U8,
            literal(left, LandedIntegerType::U8),
            literal(right, LandedIntegerType::U8),
        );
        assert_eq!(
            evaluate(&expression, &mut |_| panic!("literal plan has no binding")),
            value.and_then(expected),
            "{kind:?}({left}, {right})"
        );
    }
}

#[test]
fn signed_quotient_overflow_and_zero_divisors_follow_fixed_width_laws() {
    use CheckedIntegerBinaryKind::*;
    for (kind, value) in [
        (ExactDivide, None),
        (ExactRemainder, None),
        (WrappingDivide, Some(-128)),
        (WrappingRemainder, Some(0)),
        (SaturatingDivide, Some(127)),
        (SaturatingRemainder, Some(0)),
    ] {
        let expression = operation(
            kind,
            PrimitiveType::I8,
            literal(-128, LandedIntegerType::I8),
            literal(-1, LandedIntegerType::I8),
        );
        assert_eq!(
            evaluate(&expression, &mut |_| None),
            value.and_then(expected),
            "{kind:?}"
        );
        let zero = operation(
            kind,
            PrimitiveType::I8,
            literal(1, LandedIntegerType::I8),
            literal(0, LandedIntegerType::I8),
        );
        assert_eq!(evaluate(&zero, &mut |_| None), None, "{kind:?} by zero");
    }
    for (kind, value) in [(ExactDivide, -3), (ExactRemainder, -1)] {
        assert_eq!(
            evaluate(
                &operation(
                    kind,
                    PrimitiveType::I8,
                    literal(-7, LandedIntegerType::I8),
                    literal(2, LandedIntegerType::I8)
                ),
                &mut |_| None
            ),
            expected(value)
        );
    }
}

#[test]
fn binding_values_and_operand_types_must_match_the_checked_carrier() {
    let binding = CheckedScalarExpression::Parameter {
        position: 3,
        primitive_type: PrimitiveType::U8,
    };
    for value in [-1, 256] {
        assert_eq!(
            evaluate(&binding, &mut |position| {
                assert_eq!(position, 3);
                expected(value)
            }),
            None
        );
    }
    assert_eq!(
        evaluate(&binding, &mut |_| Some(ScalarValue::Boolean(true))),
        None
    );
    assert_eq!(evaluate(&binding, &mut |_| None), None);
    assert_eq!(evaluate(&binding, &mut |_| expected(255)), expected(255));
    let mismatched = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        literal(1, LandedIntegerType::U8),
        literal(1, LandedIntegerType::I8),
    );
    assert_eq!(evaluate(&mismatched, &mut |_| None), None);
    let anonymous = CheckedScalarExpression::IntegerLiteral {
        literal: IntegerLiteral::from_value(7),
    };
    assert_eq!(evaluate(&anonymous, &mut |_| None), None);
}

#[test]
fn shifts_use_the_count_carrier_and_preserve_signed_bits() {
    let expression = operation(
        CheckedIntegerBinaryKind::WrappingShiftLeft,
        PrimitiveType::U8,
        literal(1, LandedIntegerType::U8),
        literal(-1, LandedIntegerType::I64),
    );
    assert_eq!(evaluate(&expression, &mut |_| None), expected(128));
    let expression = operation(
        CheckedIntegerBinaryKind::ExactShiftRight,
        PrimitiveType::I8,
        literal(-4, LandedIntegerType::I8),
        literal(1, LandedIntegerType::U64),
    );
    assert_eq!(evaluate(&expression, &mut |_| None), expected(-2));
    let expression = CheckedScalarExpression::IntegerBitwiseNot {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(literal(1, LandedIntegerType::U8)),
    };
    assert_eq!(evaluate(&expression, &mut |_| None), expected(254));
}

#[test]
fn integer_casts_require_admitted_values_and_the_recorded_range() {
    let widen = CheckedScalarExpression::IntegerWiden {
        primitive_type: PrimitiveType::I16,
        operand: Box::new(literal(255, LandedIntegerType::U8)),
    };
    assert_eq!(evaluate(&widen, &mut |_| None), expected(255));
    for (value, minimum, maximum, accepted) in [
        (7, 0, 10, true),
        (11, 0, 10, false),
        (256, 0, 300, false),
        (-1, -10, 10, false),
        (7, 10, 0, false),
    ] {
        let cast = CheckedScalarExpression::IntegerExactCast {
            primitive_type: PrimitiveType::U8,
            operand: Box::new(literal(value, LandedIntegerType::I16)),
            range: psi_checked_trees::CheckedIntegerRange {
                minimum: BigInt::from_i64(minimum),
                maximum: BigInt::from_i64(maximum),
            },
        };
        assert_eq!(
            evaluate(&cast, &mut |_| None),
            accepted.then(|| ScalarValue::Integer(BigInt::from_i64(value)))
        );
    }
}

#[test]
fn booleans_preserve_short_circuit_order_and_unknown_values() {
    use CheckedBooleanExpression as Boolean;
    for (left, conjunction, expected) in [
        (false, true, Some(false)),
        (true, false, Some(true)),
        (true, true, None),
        (false, false, None),
    ] {
        let left = Box::new(Boolean::Constant(left));
        let right = Box::new(Boolean::Local { position: 2 });
        let expression = if conjunction {
            Boolean::And { left, right }
        } else {
            Boolean::Or { left, right }
        };
        let mut reads = Vec::new();
        assert_eq!(
            evaluate(
                &CheckedScalarExpression::Boolean(Box::new(expression)),
                &mut |position| {
                    reads.push(position);
                    None
                }
            ),
            expected.map(ScalarValue::Boolean)
        );
        assert_eq!(reads.is_empty(), expected.is_some());
    }
    let expression = Boolean::And {
        left: Box::new(Boolean::Parameter { position: 0 }),
        right: Box::new(Boolean::Constant(false)),
    };
    assert_eq!(
        evaluate(
            &CheckedScalarExpression::Boolean(Box::new(expression)),
            &mut |_| None
        ),
        None,
        "an unknown evaluated left operand is not made known by an unevaluated right operand"
    );
}

#[test]
fn comparisons_retain_integer_width_signedness_and_full_unsigned_precision() {
    for (kind, value) in [
        (CheckedIntegerComparisonKind::Equal, false),
        (CheckedIntegerComparisonKind::LessThan, true),
        (CheckedIntegerComparisonKind::LessOrEqual, true),
    ] {
        let expression = CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::IntegerComparison {
                kind,
                left: Box::new(literal(-1, LandedIntegerType::I8)),
                right: Box::new(literal(0, LandedIntegerType::I8)),
            },
        ));
        assert_eq!(
            evaluate(&expression, &mut |_| None),
            Some(ScalarValue::Boolean(value))
        );
    }
    let expression = CheckedScalarExpression::Local {
        position: 0,
        primitive_type: PrimitiveType::U64,
    };
    let maximum = ScalarValue::Integer(BigInt::from_u64(u64::MAX));
    assert_eq!(
        evaluate(&expression, &mut |_| Some(maximum.clone())),
        Some(maximum)
    );
    let expression = CheckedScalarExpression::IeeeFloatLiteral {
        value: psi_core::IeeeFloatValue::Binary32(0),
    };
    assert_eq!(evaluate(&expression, &mut |_| None), None);
}

#[test]
fn generated_return_plan_preserves_explicit_integer_landing() {
    let tokens = psi_source_files_to_tokens::Lexer::new("machine value() -> u8 { 3u8 + 4u8 }")
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let program =
        crate::lower_typed_trees(program).expect("checked lowering retains landed operands");
    let plans = &program.facts.values.scalar_expressions;
    let state = &program.machine_states(&program.machines()[0])[0];
    let expression = plans
        .expression_at(
            state.symbol,
            0,
            psi_checked_trees::CheckedScalarExpressionRole::Return,
        )
        .expect("selected return plan");
    assert_eq!(evaluate(expression, &mut |_| None), expected(7));
}

#[test]
fn address_values_require_target_width_authority_not_a_fixed_width_guess() {
    assert_eq!(
        evaluate(&literal(7, LandedIntegerType::Addr), &mut |_| None),
        None
    );
    let parameter = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::Addr,
    };
    assert_eq!(evaluate(&parameter, &mut |_| expected(7)), None);
    let operation = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::Addr,
        literal(1, LandedIntegerType::Addr),
        literal(2, LandedIntegerType::Addr),
    );
    assert_eq!(evaluate(&operation, &mut |_| None), None);
}
