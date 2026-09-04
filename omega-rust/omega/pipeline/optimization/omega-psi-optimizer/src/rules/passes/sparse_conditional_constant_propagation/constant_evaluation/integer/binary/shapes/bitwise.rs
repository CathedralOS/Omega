//! AND, OR, and XOR operation shapes.

use omega_abstract_operations::AbstractOperation as O;

use super::super::model::{IntegerBinaryKind, IntegerBinaryShape};

pub(super) fn classify(operation: &O) -> Option<IntegerBinaryShape> {
    let (source, result, scalar_type, left, right, kind) = match operation {
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
            IntegerBinaryKind::BitwiseAnd,
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
            IntegerBinaryKind::BitwiseOr,
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
            IntegerBinaryKind::BitwiseXor,
        ),
        _ => return None,
    };
    Some(IntegerBinaryShape::scalar(
        source,
        result,
        scalar_type,
        left,
        right,
        kind,
    ))
}
