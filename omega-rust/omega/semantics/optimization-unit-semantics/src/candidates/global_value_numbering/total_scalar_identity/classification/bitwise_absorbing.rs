//! Independent exact-width bitwise absorbing-literal classification.

use abstract_operations::AbstractOperation as O;
use optimization_unit::TotalScalarIdentityKind;

use super::model::{IndependentTotalScalarIdentity, all_ones, row, typed_integer};

pub(super) fn classify(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    match (operation, identity) {
        (
            O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                ..
            },
            TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
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
            O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                right,
                ..
            },
            TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
        ) => Some(row(
            *psi_operation,
            *result,
            *right,
            *right,
            *scalar_type,
            *scalar_type,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                ..
            },
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
        ) => Some(row(
            *psi_operation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            all_ones(*scalar_type),
        )),
        (
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                right,
                ..
            },
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight,
        ) => Some(row(
            *psi_operation,
            *result,
            *right,
            *right,
            *scalar_type,
            *scalar_type,
            all_ones(*scalar_type),
        )),
        _ => None,
    }
}
