//! Independent exact-width bitwise neutral-literal classification.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::TotalScalarIdentityKind;

use super::model::{
    IndependentTotalScalarIdentity, all_ones, left_law_row, right_law_row, typed_integer,
};

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
                right,
            },
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft,
        ) => Some(left_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            all_ones(*scalar_type),
        )),
        (
            O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            all_ones(*scalar_type),
        )),
        (
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft,
        ) => Some(left_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::IntegerBitwiseOrZeroRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft,
        ) => Some(left_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        (
            O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::IntegerBitwiseXorZeroRight,
        ) => Some(right_law_row(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            typed_integer(*scalar_type, 0),
        )),
        _ => None,
    }
}
