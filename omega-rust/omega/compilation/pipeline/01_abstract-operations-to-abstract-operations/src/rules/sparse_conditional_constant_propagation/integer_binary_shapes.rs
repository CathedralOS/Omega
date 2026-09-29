//! Exhaustive binary operation-shape classification for the integer
//! constant folds: which operation a node computes, and its operands.

use abstract_operations::AbstractOperation as O;

use crate::rules::sparse_conditional_constant_propagation::integer_binary_constants::{
    IntegerBinaryOperation, IntegerBinaryShape,
};

pub(super) fn classify(operation: &O) -> Option<IntegerBinaryShape> {
    let (source, result, scalar_type, left, right, operation) = match operation {
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
            IntegerBinaryOperation::Shift {
                policy: *policy,
                direction: *direction,
                count_type: *count_type,
            },
        ),
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryOperation::Arithmetic(*arithmetic),
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryOperation::BitwiseAnd,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryOperation::BitwiseOr,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryOperation::BitwiseXor,
        ),
        _ => return None,
    };
    Some(IntegerBinaryShape {
        source,
        result,
        scalar_type,
        left,
        right,
        operation,
    })
}
