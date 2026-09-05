//! Closed binary-operation kind and classified operation shape.

use semantic_vocabulary::{IntegerType, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerBinaryKind {
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    WrappingAdd,
    WrappingSubtract,
    WrappingMultiply,
    SaturatingAdd,
    SaturatingSubtract,
    SaturatingMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    ExactShiftLeft,
    ExactShiftRight,
    WrappingShiftLeft,
    WrappingShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

pub(super) struct IntegerBinaryShape {
    pub(super) source: OperationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left: ValueId,
    pub(super) right: ValueId,
    pub(super) count_type: Option<IntegerType>,
    pub(super) kind: IntegerBinaryKind,
}

impl IntegerBinaryShape {
    pub(super) fn scalar(
        source: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
        kind: IntegerBinaryKind,
    ) -> Self {
        Self {
            source,
            result,
            scalar_type,
            left,
            right,
            count_type: None,
            kind,
        }
    }

    pub(super) fn shift(
        source: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
        kind: IntegerBinaryKind,
    ) -> Self {
        Self {
            source,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
            count_type: Some(count_type),
            kind,
        }
    }
}
