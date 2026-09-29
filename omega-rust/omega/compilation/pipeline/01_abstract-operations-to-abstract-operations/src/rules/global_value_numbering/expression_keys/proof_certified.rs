//! Proof-certified scalar-operation classification.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::ScalarType;

use super::{ProofCertifiedScalarExpressionKey, ScalarExpressionRow, canonical_pair};

pub(in crate::rules) fn proof_certified_scalar_expression(
    operation: &O,
) -> Option<ScalarExpressionRow<ProofCertifiedScalarExpressionKey>> {
    let row = match operation {
        O::IntegerExactCast {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::ExactCast(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
        ),
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { .. },
            direction,
            value,
            count,
        } => (
            match direction {
                IntegerShiftDirection::Left => ProofCertifiedScalarExpressionKey::ExactShiftLeft(
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                ),
                IntegerShiftDirection::Right => ProofCertifiedScalarExpressionKey::ExactShiftRight(
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                ),
            },
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => {
            use IntegerArithmeticOperator as Operator;
            use IntegerOverflowPolicy as Policy;
            use ProofCertifiedScalarExpressionKey as Key;
            // Add and multiply commute; every other operator keeps its
            // operand order.
            let (low, high) = canonical_pair(*left, *right);
            let key = match (arithmetic.policy(), arithmetic.operator()) {
                (Policy::Exact, Operator::Add) => Key::ExactAdd(*scalar_type, low, high),
                (Policy::Exact, Operator::Subtract) => {
                    Key::ExactSubtract(*scalar_type, *left, *right)
                }
                (Policy::Exact, Operator::Multiply) => Key::ExactMultiply(*scalar_type, low, high),
                (Policy::Exact, Operator::Divide) => Key::ExactDivide(*scalar_type, *left, *right),
                (Policy::Exact, Operator::Remainder) => {
                    Key::ExactRemainder(*scalar_type, *left, *right)
                }
                (Policy::Wrapping, Operator::Divide) => {
                    Key::WrappingDivide(*scalar_type, *left, *right)
                }
                (Policy::Wrapping, Operator::Remainder) => {
                    Key::WrappingRemainder(*scalar_type, *left, *right)
                }
                (Policy::Saturating, Operator::Divide) => {
                    Key::SaturatingDivide(*scalar_type, *left, *right)
                }
                (Policy::Saturating, Operator::Remainder) => {
                    Key::SaturatingRemainder(*scalar_type, *left, *right)
                }
                (
                    Policy::Wrapping | Policy::Saturating,
                    Operator::Add | Operator::Subtract | Operator::Multiply,
                ) => return None,
            };
            (
                key,
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    };
    Some(row)
}
