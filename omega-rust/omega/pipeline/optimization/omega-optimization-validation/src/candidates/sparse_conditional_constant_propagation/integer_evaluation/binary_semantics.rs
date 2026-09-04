//! Exact language semantics and safety class for recognized binary integer operations.

use omega_optimization_core::OptimizationSafetyClass;
use psi_core::{IntegerType, IntegerValue};

use super::model::BinaryIntegerOperation;

pub(super) fn evaluate(
    kind: BinaryIntegerOperation,
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> (Option<IntegerValue>, OptimizationSafetyClass) {
    match kind {
        BinaryIntegerOperation::ExactAdd => (
            scalar_type.exact_add(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::ExactSubtract => (
            scalar_type.exact_sub(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::ExactMultiply => (
            scalar_type.exact_mul(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::WrappingAdd => (
            scalar_type.wrapping_add(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::WrappingSubtract => (
            scalar_type.wrapping_sub(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::WrappingMultiply => (
            scalar_type.wrapping_mul(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::SaturatingAdd => (
            scalar_type.saturating_add(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::SaturatingSubtract => (
            scalar_type.saturating_sub(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::SaturatingMultiply => (
            scalar_type.saturating_mul(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::ExactDivide => (
            scalar_type.exact_div(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::ExactRemainder => (
            scalar_type.exact_rem(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::WrappingDivide => (
            scalar_type.wrapping_div(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::WrappingRemainder => (
            scalar_type.wrapping_rem(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::SaturatingDivide => (
            scalar_type.saturating_div(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::SaturatingRemainder => (
            scalar_type.saturating_rem(left, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::ExactShiftLeft(count_type) => (
            scalar_type.exact_shift_left(left, count_type, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::ExactShiftRight(count_type) => (
            scalar_type.exact_shift_right(left, count_type, right),
            OptimizationSafetyClass::ProofCertified,
        ),
        BinaryIntegerOperation::WrappingShiftLeft(count_type) => (
            scalar_type.wrapping_shift_left(left, count_type, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::WrappingShiftRight(count_type) => (
            scalar_type.wrapping_shift_right(left, count_type, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::BitwiseAnd => (
            scalar_type.bitwise_and(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::BitwiseOr => (
            scalar_type.bitwise_or(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        BinaryIntegerOperation::BitwiseXor => (
            scalar_type.bitwise_xor(left, right),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
    }
}
