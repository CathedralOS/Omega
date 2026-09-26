//! Optimizer module role: executable entrance. Exhaustive independent dispatch for total scalar identity families.

use abstract_operations::AbstractOperation as O;
use optimization_unit::TotalScalarIdentityKind;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, OperationId, ValueId};

mod bitwise_absorbing;
mod bitwise_neutral;
mod saturating;
mod wrapping;

pub(super) fn independently_classify_total_scalar_identity(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    match identity {
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerAddZeroRight
        | TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight
        | TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount
        | TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount
        | TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => {
            wrapping::classify(operation, identity)
        }
        TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerAddZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => {
            saturating::classify(operation, identity)
        }
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => {
            bitwise_neutral::classify(operation, identity)
        }
        TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight => {
            bitwise_absorbing::classify(operation, identity)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IndependentTotalScalarIdentity {
    pub source_operation: OperationId,
    pub result: ValueId,
    pub replacement: ValueId,
    pub law_operand: ValueId,
    pub scalar_type: IntegerType,
    pub law_operand_type: IntegerType,
    pub law_constant: IntegerValue,
}

fn row(
    source_operation: OperationId,
    result: ValueId,
    replacement: ValueId,
    law_operand: ValueId,
    scalar_type: IntegerType,
    law_operand_type: IntegerType,
    law_constant: IntegerValue,
) -> IndependentTotalScalarIdentity {
    IndependentTotalScalarIdentity {
        source_operation,
        result,
        replacement,
        law_operand,
        scalar_type,
        law_operand_type,
        law_constant,
    }
}

fn left_law_row(
    source_operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
    law_constant: IntegerValue,
) -> IndependentTotalScalarIdentity {
    row(
        source_operation,
        result,
        right,
        left,
        scalar_type,
        scalar_type,
        law_constant,
    )
}

fn right_law_row(
    source_operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
    law_constant: IntegerValue,
) -> IndependentTotalScalarIdentity {
    row(
        source_operation,
        result,
        left,
        right,
        scalar_type,
        scalar_type,
        law_constant,
    )
}

fn typed_integer(scalar_type: IntegerType, value: u128) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}

fn all_ones(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => scalar_type.maximum_value(),
    }
}
