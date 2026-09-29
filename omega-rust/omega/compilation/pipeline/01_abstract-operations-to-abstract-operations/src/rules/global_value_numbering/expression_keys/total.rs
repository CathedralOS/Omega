//! Obligation-free scalar-operation classification.

use std::collections::BTreeMap;

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::{ScalarType, ValueId};

use super::{ScalarExpressionRow, TotalScalarExpressionKey, canonical_pair};

pub(in crate::rules::global_value_numbering) fn total_scalar_expression(
    operation: &O,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Option<ScalarExpressionRow<TotalScalarExpressionKey>> {
    let boolean = ScalarType::Boolean;
    let integer_operand_type = |value: ValueId| match value_types.get(&value) {
        Some(ScalarType::Integer(scalar_type)) => Some(*scalar_type),
        _ => None,
    };
    let row = match operation {
        O::BooleanConstant {
            psi_operation,
            result,
            value,
        } => (
            TotalScalarExpressionKey::BooleanConstant(*value),
            *psi_operation,
            *result,
            boolean,
        ),
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => (
            TotalScalarExpressionKey::IntegerConstant(*scalar_type, *value),
            *psi_operation,
            *result,
            *scalar_type,
        ),
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => (
            TotalScalarExpressionKey::BooleanNot(*operand),
            *psi_operation,
            *result,
            boolean,
        ),
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::BooleanEqual(left, right),
                *psi_operation,
                *result,
                boolean,
            )
        }
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = integer_operand_type(*left)?;
            if integer_operand_type(*right)? != scalar_type {
                return None;
            }
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::IntegerEqual(scalar_type, left, right),
                *psi_operation,
                *result,
                boolean,
            )
        }
        O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = integer_operand_type(*left)?;
            if integer_operand_type(*right)? != scalar_type {
                return None;
            }
            (
                TotalScalarExpressionKey::IntegerLessThan(scalar_type, *left, *right),
                *psi_operation,
                *result,
                boolean,
            )
        }
        O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = integer_operand_type(*left)?;
            if integer_operand_type(*right)? != scalar_type {
                return None;
            }
            (
                TotalScalarExpressionKey::IntegerLessOrEqual(scalar_type, *left, *right),
                *psi_operation,
                *result,
                boolean,
            )
        }
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => (
            TotalScalarExpressionKey::IntegerBitwiseNot(*scalar_type, *operand),
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
            TotalScalarExpressionKey::IntegerWiden(*source_type, *target_type, *operand),
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
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::IntegerBitwiseAnd(*scalar_type, left, right),
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
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::IntegerBitwiseOr(*scalar_type, left, right),
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
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::IntegerBitwiseXor(*scalar_type, left, right),
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
                IntegerShiftDirection::Left => TotalScalarExpressionKey::WrappingShiftLeft(
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                ),
                IntegerShiftDirection::Right => TotalScalarExpressionKey::WrappingShiftRight(
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
            use TotalScalarExpressionKey as Key;
            // Add and multiply commute; subtract keeps its operand order.
            let (low, high) = canonical_pair(*left, *right);
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
    };
    Some(row)
}
