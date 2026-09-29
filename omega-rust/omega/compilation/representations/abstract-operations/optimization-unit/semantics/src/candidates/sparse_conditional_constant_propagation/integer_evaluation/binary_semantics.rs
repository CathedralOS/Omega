//! Exact language semantics and safety class for recognized binary integer operations.
//!
//! This table is the validator's own replay of each policy pair's meaning. It
//! does not call the representation's shared evaluator, so a producer that
//! folded under the wrong pair cannot confirm itself here.

use abstract_operations::{
    IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_core::OptimizationSafetyClass;
use semantic_vocabulary::{IntegerType, IntegerValue};

use super::BinaryIntegerOperation;

pub(super) fn evaluate(
    kind: BinaryIntegerOperation,
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> (Option<IntegerValue>, OptimizationSafetyClass) {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    use OptimizationSafetyClass::{ExactOperationSemantics, ProofCertified};
    match kind {
        BinaryIntegerOperation::Arithmetic(policy, operator) => match (policy, operator) {
            (Policy::Exact, Operator::Add) => (scalar_type.exact_add(left, right), ProofCertified),
            (Policy::Exact, Operator::Subtract) => {
                (scalar_type.exact_sub(left, right), ProofCertified)
            }
            (Policy::Exact, Operator::Multiply) => {
                (scalar_type.exact_mul(left, right), ProofCertified)
            }
            (Policy::Wrapping, Operator::Add) => (
                scalar_type.wrapping_add(left, right),
                ExactOperationSemantics,
            ),
            (Policy::Wrapping, Operator::Subtract) => (
                scalar_type.wrapping_sub(left, right),
                ExactOperationSemantics,
            ),
            (Policy::Wrapping, Operator::Multiply) => (
                scalar_type.wrapping_mul(left, right),
                ExactOperationSemantics,
            ),
            (Policy::Saturating, Operator::Add) => (
                scalar_type.saturating_add(left, right),
                ExactOperationSemantics,
            ),
            (Policy::Saturating, Operator::Subtract) => (
                scalar_type.saturating_sub(left, right),
                ExactOperationSemantics,
            ),
            (Policy::Saturating, Operator::Multiply) => (
                scalar_type.saturating_mul(left, right),
                ExactOperationSemantics,
            ),
            (Policy::Exact, Operator::Divide) => {
                (scalar_type.exact_div(left, right), ProofCertified)
            }
            (Policy::Exact, Operator::Remainder) => {
                (scalar_type.exact_rem(left, right), ProofCertified)
            }
            (Policy::Wrapping, Operator::Divide) => {
                (scalar_type.wrapping_div(left, right), ProofCertified)
            }
            (Policy::Wrapping, Operator::Remainder) => {
                (scalar_type.wrapping_rem(left, right), ProofCertified)
            }
            (Policy::Saturating, Operator::Divide) => {
                (scalar_type.saturating_div(left, right), ProofCertified)
            }
            (Policy::Saturating, Operator::Remainder) => {
                (scalar_type.saturating_rem(left, right), ProofCertified)
            }
        },
        BinaryIntegerOperation::Shift(policy, direction, count_type) => match (policy, direction) {
            (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Left) => (
                scalar_type.exact_shift_left(left, count_type, right),
                ProofCertified,
            ),
            (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Right) => (
                scalar_type.exact_shift_right(left, count_type, right),
                ProofCertified,
            ),
            (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Left) => (
                scalar_type.wrapping_shift_left(left, count_type, right),
                ExactOperationSemantics,
            ),
            (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Right) => (
                scalar_type.wrapping_shift_right(left, count_type, right),
                ExactOperationSemantics,
            ),
        },
        BinaryIntegerOperation::BitwiseAnd => (
            scalar_type.bitwise_and(left, right),
            ExactOperationSemantics,
        ),
        BinaryIntegerOperation::BitwiseOr => {
            (scalar_type.bitwise_or(left, right), ExactOperationSemantics)
        }
        BinaryIntegerOperation::BitwiseXor => (
            scalar_type.bitwise_xor(left, right),
            ExactOperationSemantics,
        ),
    }
}
