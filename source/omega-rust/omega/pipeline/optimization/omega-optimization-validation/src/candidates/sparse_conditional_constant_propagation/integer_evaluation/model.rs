//! Closed vocabulary carried from binary shape recognition into exact evaluation.

use omega_optimization_core::OptimizationSafetyClass;
use psi_core::{IntegerType, IntegerValue, OperationId, ValueId};

pub(super) type IntegerEvaluation = (
    OperationId,
    ValueId,
    IntegerType,
    IntegerValue,
    OptimizationSafetyClass,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BinaryIntegerOperation {
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
    ExactShiftLeft(IntegerType),
    ExactShiftRight(IntegerType),
    WrappingShiftLeft(IntegerType),
    WrappingShiftRight(IntegerType),
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BinaryOperationShape {
    pub(super) kind: BinaryIntegerOperation,
    pub(super) source: OperationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left: ValueId,
    pub(super) right: ValueId,
}
