//! Obligation-free total scalar-expression reconstruction.
use crate::candidates::global_value_numbering::expression_keys::{
    IndependentTotalScalarExpressionKey, independent_pair,
};
use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::{OperationId, ScalarType, ValueId};
use std::collections::BTreeMap;

pub(crate) fn independent_total_scalar_expression(
    operation: &O,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Option<(
    IndependentTotalScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
)> {
    let operand_integer = |value: ValueId| match value_types.get(&value) {
        Some(ScalarType::Integer(row)) => Some(*row),
        _ => None,
    };
    Some(match operation {
        O::BooleanConstant {
            psi_operation,
            result,
            value,
        } => (
            IndependentTotalScalarExpressionKey::BooleanConstant(*value),
            *psi_operation,
            *result,
            ScalarType::Boolean,
        ),
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => (
            IndependentTotalScalarExpressionKey::IntegerConstant(*scalar_type, *value),
            *psi_operation,
            *result,
            *scalar_type,
        ),
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::BooleanNot(*operand),
            *psi_operation,
            *result,
            ScalarType::Boolean,
        ),
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::BooleanEqual(left, right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerEqual(scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            (
                IndependentTotalScalarExpressionKey::IntegerLessThan(scalar_type, *left, *right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            (
                IndependentTotalScalarExpressionKey::IntegerLessOrEqual(scalar_type, *left, *right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::IntegerBitwiseNot(*scalar_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::IntegerWiden(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseAnd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseOr(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseXor(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
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
        } => (
            match direction {
                IntegerShiftDirection::Left => {
                    IndependentTotalScalarExpressionKey::WrappingShiftLeft(
                        *value_type,
                        *count_type,
                        *value,
                        *count,
                    )
                }
                IntegerShiftDirection::Right => {
                    IndependentTotalScalarExpressionKey::WrappingShiftRight(
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
        ),
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => {
            use IndependentTotalScalarExpressionKey as Key;
            use IntegerArithmeticOperator as Operator;
            use IntegerOverflowPolicy as Policy;
            // Add and multiply commute; subtract keeps its operand order.
            let (low, high) = independent_pair(*left, *right);
            let key = match (arithmetic.policy(), arithmetic.operator()) {
                (Policy::Wrapping, Operator::Add) => Key::WrappingAdd(*scalar_type, low, high),
                (Policy::Wrapping, Operator::Subtract) => {
                    Key::WrappingSubtract(*scalar_type, *left, *right)
                }
                (Policy::Wrapping, Operator::Multiply) => {
                    Key::WrappingMultiply(*scalar_type, low, high)
                }
                (Policy::Saturating, Operator::Add) => Key::SaturatingAdd(*scalar_type, low, high),
                (Policy::Saturating, Operator::Subtract) => {
                    Key::SaturatingSubtract(*scalar_type, *left, *right)
                }
                (Policy::Saturating, Operator::Multiply) => {
                    Key::SaturatingMultiply(*scalar_type, low, high)
                }
                _ => return None,
            };
            (
                key,
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    })
}
