//! Closed independent classification of total scalar identities.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct IndependentTotalScalarIdentity {
    pub source_operation: OperationId,
    pub result: ValueId,
    pub replacement: ValueId,
    pub law_operand: ValueId,
    pub scalar_type: IntegerType,
    pub law_operand_type: IntegerType,
    pub law_constant: IntegerValue,
}

pub(super) fn independently_classify_total_scalar_identity(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    let (
        source_operation,
        result,
        replacement,
        law_operand,
        scalar_type,
        law_operand_type,
        law_constant,
    ) = match (operation, identity) {
        (
            O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
        ) => (
            *psi_operation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 0),
        ),
        (
            O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
        ) => (
            *psi_operation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 0),
        ),
        (
            O::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight,
        ) => (
            *psi_operation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 0),
        ),
        (
            O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
        ) => (
            *psi_operation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 1),
        ),
        (
            O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight,
        ) => (
            *psi_operation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 1),
        ),
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
        ) => (
            *psi_operation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            independently_typed_integer(*count_type, 0),
        ),
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
        ) => (
            *psi_operation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            independently_typed_integer(*count_type, 0),
        ),
        (
            O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                ..
            },
            TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft,
        ) => (
            *psi_operation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 0),
        ),
        (
            O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                right,
                ..
            },
            TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight,
        ) => (
            *psi_operation,
            *result,
            *right,
            *right,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 0),
        ),
        (
            O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft,
        ) => (
            *psi_operation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 0),
        ),
        (
            O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerAddZeroRight,
        ) => (
            *psi_operation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 0),
        ),
        (
            O::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight,
        ) => (
            *psi_operation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 0),
        ),
        (
            O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
        ) => (
            *psi_operation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 1),
        ),
        (
            O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight,
        ) => (
            *psi_operation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independently_typed_integer(*scalar_type, 1),
        ),
        _ => return None,
    };
    Some(IndependentTotalScalarIdentity {
        source_operation,
        result,
        replacement,
        law_operand,
        scalar_type,
        law_operand_type,
        law_constant,
    })
}

fn independently_typed_integer(scalar_type: IntegerType, value: u128) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}
