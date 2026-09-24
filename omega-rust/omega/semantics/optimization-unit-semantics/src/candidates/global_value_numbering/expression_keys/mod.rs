//! Optimizer module role: stage group. Independent scalar-expression reconstruction group.
//!
//! The model defines three closed equivalence vocabularies. Obligation-free,
//! proof-certified, and compatible-policy reconstruction descend into separate
//! leaves; compatible-policy keeps its directional leader/redundant join.
//! Canonical commutative operand order is shared here and nowhere else.
use semantic_vocabulary::ValueId;

mod compatible_policy;
mod proof_certified;
mod total;

pub(crate) use compatible_policy::{
    independent_compatible_policy_scalar_leader, independent_compatible_policy_scalar_redundant,
};
pub(crate) use proof_certified::independent_proof_scalar_expression;
use semantic_vocabulary::{IntegerType, ScalarType};
pub(crate) use total::independent_total_scalar_expression;

pub(crate) fn independent_pair(left: ValueId, right: ValueId) -> (ValueId, ValueId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentTotalScalarExpressionKey {
    BooleanConstant(bool),
    IntegerConstant(ScalarType, semantic_vocabulary::IntegerValue),
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
