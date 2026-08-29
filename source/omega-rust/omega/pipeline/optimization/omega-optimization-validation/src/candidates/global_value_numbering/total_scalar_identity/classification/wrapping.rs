//! Independent wrapping-arithmetic and zero-count-shift classification.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::TotalScalarIdentityKind;

use super::model::{
    IndependentTotalScalarIdentity, left_law_row, right_law_row, row, typed_integer,
};

pub(super) fn classify(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    match (operation, identity) {
        (
            O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
        ) => Some(left_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
        ) => Some(left_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 1),
        )),
        (
            O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 1),
        )),
        (
            O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
            },
            TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
        ) => Some(row(
            *psi_operation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            typed_integer(*count_type, 0),
        )),
        (
            O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                count_type,
                value,
                count,
            },
            TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
        ) => Some(row(
            *psi_operation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            typed_integer(*count_type, 0),
        )),
        (
            O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                ..
            },
            TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft,
        ) => Some(row(
            *psi_operation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                right,
                ..
            },
            TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight,
        ) => Some(row(
            *psi_operation,
            *result,
            *right,
            *right,
            *scalar_type,
            *scalar_type,
            typed_integer(*scalar_type, 0),
        )),
        _ => None,
    }
}
