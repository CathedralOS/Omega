//! Independent wrapping-arithmetic and zero-count-shift classification.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_unit::TotalScalarIdentityKind;

use super::{IndependentTotalScalarIdentity, left_law_row, right_law_row, row, typed_integer};

pub(super) fn classify(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    use IntegerArithmeticOperator as Operator;
    use TotalScalarIdentityKind as Kind;
    match operation {
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } if arithmetic.policy() == IntegerOverflowPolicy::Wrapping => {
            let (psi_operation, result, scalar_type, left, right) =
                (*psi_operation, *result, *scalar_type, *left, *right);
            match (arithmetic.operator(), identity) {
                (Operator::Add, Kind::WrappingIntegerAddZeroLeft) => Some(left_law_row(
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                    typed_integer(scalar_type, 0),
                )),
                (Operator::Add, Kind::WrappingIntegerAddZeroRight)
                | (Operator::Subtract, Kind::WrappingIntegerSubtractZeroRight) => {
                    Some(right_law_row(
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                        typed_integer(scalar_type, 0),
                    ))
                }
                (Operator::Multiply, Kind::WrappingIntegerMultiplyOneLeft) => Some(left_law_row(
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                    typed_integer(scalar_type, 1),
                )),
                (Operator::Multiply, Kind::WrappingIntegerMultiplyOneRight) => Some(right_law_row(
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                    typed_integer(scalar_type, 1),
                )),
                (Operator::Multiply, Kind::WrappingIntegerMultiplyZeroLeft) => Some(row(
                    psi_operation,
                    result,
                    left,
                    left,
                    scalar_type,
                    scalar_type,
                    typed_integer(scalar_type, 0),
                )),
                (Operator::Multiply, Kind::WrappingIntegerMultiplyZeroRight) => Some(row(
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
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Wrapping,
            direction,
            value,
            count,
        } => match (direction, identity) {
            (IntegerShiftDirection::Left, Kind::WrappingIntegerShiftLeftZeroCount)
            | (IntegerShiftDirection::Right, Kind::WrappingIntegerShiftRightZeroCount) => {
                Some(row(
                    *psi_operation,
                    *result,
                    *value,
                    *count,
                    *value_type,
                    *count_type,
                    typed_integer(*count_type, 0),
                ))
            }
            _ => None,
        },
        _ => None,
    }
}
