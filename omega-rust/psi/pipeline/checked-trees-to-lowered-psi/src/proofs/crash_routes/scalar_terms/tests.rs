//! IEEE comparison guard scalar-term controls.
//!
//! A float operand is an ordinary scalar term, but an IEEE comparison is an
//! atomic proposition (`Proposition::ScalarIeeeFloatComparison`, handled by
//! `crash_predicates`): IEEE equality is non-reflexive at NaN and distinguishes
//! signed zero, so it can never be a generic/reflexive `ScalarTerm`. The term
//! lane must refuse it rather than silently weakening it into Boolean equality.

use super::{
    CheckedBooleanExpression, CheckedScalarExpression, LoweredDirectExpression, LoweringError,
    ScalarTerm, ScalarType, ValueDeclaration, checked_boolean_scalar_term, checked_scalar_term,
    lowered_direct_scalar_term,
};
use crate::proofs::{CheckedIntegerComparisonKind, PrimitiveType, integer_scalar_type};
use crate::terminal_identities::value_id;
use checked_trees::CheckedIeeeFloatComparisonKind;
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};

fn declared_value(identity: usize, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id: value_id(identity),
        scalar_type,
        qualifications: Default::default(),
    }
}

fn float_parameter(position: usize) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::F32,
    }
}

fn integer_parameter(position: usize) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::I32,
    }
}

fn float_pair() -> CheckedBooleanExpression {
    CheckedBooleanExpression::ScalarIeeeFloatComparison {
        kind: CheckedIeeeFloatComparisonKind::Equal,
        left: Box::new(float_parameter(0)),
        right: Box::new(float_parameter(1)),
    }
}

fn float_values() -> Vec<ValueDeclaration> {
    [
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
    ]
    .into_iter()
    .enumerate()
    .map(|(identity, scalar_type)| declared_value(identity + 1, scalar_type))
    .collect()
}

fn i32_scalar_type() -> ScalarType {
    integer_scalar_type(PrimitiveType::I32).unwrap()
}

fn i32_integer_type() -> semantic_vocabulary::IntegerType {
    let ScalarType::Integer(kind) = i32_scalar_type() else {
        unreachable!("i32 is an integer scalar type")
    };
    kind
}

#[test]
fn float_parameters_lower_to_float_scalar_terms() {
    let values = float_values();
    let term = checked_scalar_term(&float_parameter(0), &values, &[]).unwrap();
    assert_eq!(
        term,
        ScalarTerm::value(
            value_id(1),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        )
    );
}

#[test]
fn scalar_ieee_comparison_is_an_atomic_proposition_not_a_term() {
    let values = float_values();
    for expression in [
        float_pair(),
        CheckedBooleanExpression::Not(Box::new(float_pair())),
        CheckedBooleanExpression::Equal {
            left: Box::new(float_pair()),
            right: Box::new(CheckedBooleanExpression::Constant(true)),
        },
    ] {
        let Err(error) = checked_boolean_scalar_term(&expression, &values, &[]) else {
            panic!("IEEE comparison in a scalar-term position must reject: {expression:?}");
        };
        assert!(
            matches!(&error, LoweringError::Unsupported(message) if message
                .contains("scalar IEEE equality is an atomic proposition, not a scalar term")),
            "{error:?}"
        );
    }
}

#[test]
fn integer_comparisons_lower_to_typed_terms() {
    let values = [i32_scalar_type(), i32_scalar_type()]
        .into_iter()
        .enumerate()
        .map(|(identity, scalar_type)| declared_value(identity + 1, scalar_type))
        .collect::<Vec<_>>();
    for (kind, expect) in [
        (
            CheckedIntegerComparisonKind::Equal,
            ScalarTerm::integer_equal,
        ),
        (
            CheckedIntegerComparisonKind::LessThan,
            ScalarTerm::integer_less_than,
        ),
        (
            CheckedIntegerComparisonKind::LessOrEqual,
            ScalarTerm::integer_less_or_equal,
        ),
    ] {
        let expression = CheckedBooleanExpression::IntegerComparison {
            kind,
            left: Box::new(integer_parameter(0)),
            right: Box::new(integer_parameter(1)),
        };
        let term = checked_boolean_scalar_term(&expression, &values, &[]).unwrap();
        let expected = expect(
            i32_integer_type(),
            ScalarTerm::value(value_id(1), i32_scalar_type()),
            ScalarTerm::value(value_id(2), i32_scalar_type()),
        )
        .unwrap();
        assert_eq!(term, expected);
    }
}

#[test]
fn integer_comparison_rejects_float_operands() {
    let values = float_values();
    let expression = CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::Equal,
        left: Box::new(float_parameter(0)),
        right: Box::new(float_parameter(1)),
    };
    let Err(error) = checked_boolean_scalar_term(&expression, &values, &[]) else {
        panic!("a float operand cannot form an integer comparison term");
    };
    assert!(
        matches!(&error, LoweringError::Unsupported(message) if message
            .contains("crash comparison operand is not an integer")),
        "{error:?}"
    );
}

#[test]
fn ieee_float_literal_has_no_generic_scalar_term() {
    let Err(error) = lowered_direct_scalar_term(
        &LoweredDirectExpression::IeeeFloatLiteral {
            value: IeeeFloatValue::Binary32(0x3f80_0000),
        },
        &[],
        &[],
    ) else {
        panic!("an IEEE float literal must not become a generic scalar term");
    };
    assert!(
        matches!(&error, LoweringError::Unsupported(message) if message
            .contains("generic scalar crash terms do not carry IEEE float literals")),
        "{error:?}"
    );
}
