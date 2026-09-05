//! Independent saturating-arithmetic identity classification.

use abstract_operations::AbstractOperation as O;
use optimization_unit::TotalScalarIdentityKind;

use super::model::{
    IndependentTotalScalarIdentity, left_law_row, right_law_row, row, typed_integer,
};

pub(super) fn classify(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    match (operation, identity) {
        (
            O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft,
        ) => Some(left_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerAddZeroRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
        ) => Some(left_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 1),
        )),
        (
            O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 1),
        )),
        (
            O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                ..
            },
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
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
            O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                right,
                ..
            },
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
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
