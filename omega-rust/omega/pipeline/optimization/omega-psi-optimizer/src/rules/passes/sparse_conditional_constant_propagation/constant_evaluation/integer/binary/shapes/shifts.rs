//! Shift operation shapes, including the distinct count type.

use omega_abstract_operations::AbstractOperation as O;

use super::super::model::{IntegerBinaryKind, IntegerBinaryShape};

pub(super) fn classify(operation: &O) -> Option<IntegerBinaryShape> {
    let (source, result, value_type, count_type, value, count, kind) = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::ExactShiftLeft,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::ExactShiftRight,
        ),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::WrappingShiftLeft,
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::WrappingShiftRight,
        ),
        _ => return None,
    };
    Some(IntegerBinaryShape::shift(
        source, result, value_type, count_type, value, count, kind,
    ))
}
