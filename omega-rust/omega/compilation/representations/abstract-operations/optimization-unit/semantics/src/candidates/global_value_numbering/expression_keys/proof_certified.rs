//! Proof-certified scalar-expression reconstruction.
use crate::candidates::global_value_numbering::expression_keys::{
    IndependentProofScalarExpressionKey, independent_pair,
};
use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::{OperationId, ScalarType, ValueId};

pub(crate) fn independent_proof_scalar_expression(
    operation: &O,
) -> Option<(
    IndependentProofScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    semantic_vocabulary::ObligationId,
)> {
    Some(match operation {
        O::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type,
            target_type,
            operand,
        } => (
            IndependentProofScalarExpressionKey::ExactCast(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            *obligation,
        ),
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { obligation },
            direction,
            value,
            count,
        } => (
            match direction {
                IntegerShiftDirection::Left => IndependentProofScalarExpressionKey::ExactShiftLeft(
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                ),
                IntegerShiftDirection::Right => {
                    IndependentProofScalarExpressionKey::ExactShiftRight(
                        *value_type,
                        *count_type,
                        *value,
                        *count,
                    )
                }
            },
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => {
            use IndependentProofScalarExpressionKey as Key;
            use IntegerArithmeticOperator as Operator;
            use IntegerOverflowPolicy as Policy;
            let obligation = arithmetic.obligation()?;
            // Add and multiply commute; every other operator keeps its
            // operand order.
            let (low, high) = independent_pair(*left, *right);
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
                obligation,
            )
        }
        _ => return None,
    })
}
