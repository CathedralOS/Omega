//! Closed independent scalar-expression key vocabularies.

use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentTotalScalarExpressionKey {
    BooleanConstant(bool),
    IntegerConstant(ScalarType, psi_core::IntegerValue),
    BooleanNot(ValueId),
    BooleanEqual(ValueId, ValueId),
    IntegerEqual(IntegerType, ValueId, ValueId),
    IntegerLessThan(IntegerType, ValueId, ValueId),
    IntegerLessOrEqual(IntegerType, ValueId, ValueId),
    IntegerBitwiseNot(IntegerType, ValueId),
    IntegerWiden(IntegerType, IntegerType, ValueId),
    IntegerBitwiseAnd(IntegerType, ValueId, ValueId),
    IntegerBitwiseOr(IntegerType, ValueId, ValueId),
    IntegerBitwiseXor(IntegerType, ValueId, ValueId),
    WrappingShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    WrappingShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    WrappingAdd(IntegerType, ValueId, ValueId),
    WrappingSubtract(IntegerType, ValueId, ValueId),
    WrappingMultiply(IntegerType, ValueId, ValueId),
    SaturatingAdd(IntegerType, ValueId, ValueId),
    SaturatingSubtract(IntegerType, ValueId, ValueId),
    SaturatingMultiply(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentProofScalarExpressionKey {
    ExactCast(IntegerType, IntegerType, ValueId),
    ExactShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ExactShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    ExactAdd(IntegerType, ValueId, ValueId),
    ExactSubtract(IntegerType, ValueId, ValueId),
    ExactMultiply(IntegerType, ValueId, ValueId),
    ExactDivide(IntegerType, ValueId, ValueId),
    ExactRemainder(IntegerType, ValueId, ValueId),
    WrappingDivide(IntegerType, ValueId, ValueId),
    WrappingRemainder(IntegerType, ValueId, ValueId),
    SaturatingDivide(IntegerType, ValueId, ValueId),
    SaturatingRemainder(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentCompatiblePolicyScalarExpressionKey {
    ShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    Add(IntegerType, ValueId, ValueId),
    Subtract(IntegerType, ValueId, ValueId),
    Multiply(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentScalarExpressionKey {
    ObligationFree(IndependentTotalScalarExpressionKey),
    ProofCertified(IndependentProofScalarExpressionKey),
    CompatiblePolicy(IndependentCompatiblePolicyScalarExpressionKey),
}
