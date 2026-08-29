//! Independent proof-certified scalar identity classification.

use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct IndependentProofCertifiedScalarIdentity {
    pub(super) source_operation: OperationId,
    pub(super) obligation: psi_core::ObligationId,
    pub(super) result: ValueId,
    pub(super) replacement: ValueId,
    pub(super) identity_operand: ValueId,
    pub(super) result_type: IntegerType,
    pub(super) identity_type: IntegerType,
    pub(super) identity_constant: IntegerValue,
}

pub(crate) fn independent_proof_certified_scalar_identity(
    operation: &O,
    identity: ProofCertifiedScalarIdentityKind,
) -> Option<IndependentProofCertifiedScalarIdentity> {
    let row = match (operation, identity) {
        (
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type,
                value,
                count,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            independent_integer_zero(*count_type),
        ),
        (
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type,
                value,
                count,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            independent_integer_zero(*count_type),
        ),
        (
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                right,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                value,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *value,
            *value_type,
            *value_type,
            independent_integer_zero(*value_type),
        ),
        (
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                value,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *value,
            *value_type,
            *value_type,
            independent_integer_zero(*value_type),
        ),
        (
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                value,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue,
        ) if value_type.carrier() == IntegerCarrier::Fixed
            && value_type.sign() == IntegerSign::Signed =>
        {
            (
                *psi_operation,
                *obligation,
                *result,
                *value,
                *value,
                *value_type,
                *value_type,
                IntegerValue::Signed(-1),
            )
        }
        _ => return None,
    };
    Some(IndependentProofCertifiedScalarIdentity {
        source_operation: row.0,
        obligation: row.1,
        result: row.2,
        replacement: row.3,
        identity_operand: row.4,
        result_type: row.5,
        identity_type: row.6,
        identity_constant: row.7,
    })
}

pub(crate) fn independent_integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}

pub(crate) fn independent_integer_one(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    }
}
