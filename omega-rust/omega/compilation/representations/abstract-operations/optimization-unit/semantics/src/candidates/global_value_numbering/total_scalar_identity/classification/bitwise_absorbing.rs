//! Independent exact-width bitwise absorbing-literal classification.

use abstract_operations::AbstractOperation as O;
use optimization_unit::TotalScalarIdentityKind;

use super::{IndependentTotalScalarIdentity, all_ones, row, typed_integer};

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
                left: constant,
                ..
            },
            TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
        )
        | (
            O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                right: constant,
                ..
            },
            TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
        ) => Some(row(
            *psi_operation,
            *result,
            *constant,
            *constant,
            *scalar_type,
            *scalar_type,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left: constant,
                ..
            },
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
        )
        | (
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                right: constant,
                ..
            },
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight,
        ) => Some(row(
            *psi_operation,
            *result,
            *constant,
            *constant,
            *scalar_type,
            *scalar_type,
            all_ones(*scalar_type),
        )),
        _ => None,
    }
}
