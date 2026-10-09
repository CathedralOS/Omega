//! Independent saturating-arithmetic identity classification.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
};
use optimization_unit::TotalScalarIdentityKind;

use super::{IndependentTotalScalarIdentity, left_law_row, right_law_row, row, typed_integer};

pub(super) fn classify(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    use IntegerArithmeticOperator as Operator;
    use TotalScalarIdentityKind as Kind;
    let Some(operands) = operation.integer_arithmetic() else {
        return None;
    };
    if operands.arithmetic.policy() != IntegerOverflowPolicy::Saturating {
        return None;
    }
    let (psi_operation, result, scalar_type, left, right) = (
        operands.psi_operation,
        operands.result,
        operands.scalar_type,
        operands.left,
        operands.right,
    );
    match (operands.arithmetic.operator(), identity) {
        (Operator::Add, Kind::SaturatingIntegerAddZeroLeft) => Some(left_law_row(
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            typed_integer(scalar_type, 0),
        )),
        (Operator::Add, Kind::SaturatingIntegerAddZeroRight)
        | (Operator::Subtract, Kind::SaturatingIntegerSubtractZeroRight) => Some(right_law_row(
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            typed_integer(scalar_type, 0),
        )),
        (Operator::Multiply, Kind::SaturatingIntegerMultiplyOneLeft) => Some(left_law_row(
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            typed_integer(scalar_type, 1),
        )),
        (Operator::Multiply, Kind::SaturatingIntegerMultiplyOneRight) => Some(right_law_row(
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            typed_integer(scalar_type, 1),
        )),
        (Operator::Multiply, Kind::SaturatingIntegerMultiplyZeroLeft) => Some(row(
            psi_operation,
            result,
            left,
            left,
            scalar_type,
            scalar_type,
            typed_integer(scalar_type, 0),
        )),
        (Operator::Multiply, Kind::SaturatingIntegerMultiplyZeroRight) => Some(row(
            psi_operation,
            result,
            right,
            right,
            scalar_type,
            scalar_type,
            typed_integer(scalar_type, 0),
        )),
        _ => None,
    }
}
