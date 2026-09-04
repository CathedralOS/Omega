//! Independently reconstructed identity shape.

use psi_core::{IntegerSign, IntegerType, IntegerValue, OperationId, ValueId};

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

pub(super) fn row(
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

pub(super) fn left_law_row(
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

pub(super) fn right_law_row(
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

pub(super) fn typed_integer(scalar_type: IntegerType, value: u128) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}

pub(super) fn all_ones(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => scalar_type.maximum_value(),
    }
}
