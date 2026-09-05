//! Typed evaluation of a classified binary integer operation.

use semantic_vocabulary::IntegerValue;

use super::model::{IntegerBinaryKind, IntegerBinaryShape};

pub(super) fn evaluate(
    shape: &IntegerBinaryShape,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<IntegerValue> {
    match shape.kind {
        IntegerBinaryKind::ExactAdd => shape.scalar_type.exact_add(left, right),
        IntegerBinaryKind::ExactSubtract => shape.scalar_type.exact_sub(left, right),
        IntegerBinaryKind::ExactMultiply => shape.scalar_type.exact_mul(left, right),
        IntegerBinaryKind::WrappingAdd => shape.scalar_type.wrapping_add(left, right),
        IntegerBinaryKind::WrappingSubtract => shape.scalar_type.wrapping_sub(left, right),
        IntegerBinaryKind::WrappingMultiply => shape.scalar_type.wrapping_mul(left, right),
        IntegerBinaryKind::SaturatingAdd => shape.scalar_type.saturating_add(left, right),
        IntegerBinaryKind::SaturatingSubtract => shape.scalar_type.saturating_sub(left, right),
        IntegerBinaryKind::SaturatingMultiply => shape.scalar_type.saturating_mul(left, right),
        IntegerBinaryKind::ExactDivide => shape.scalar_type.exact_div(left, right),
        IntegerBinaryKind::ExactRemainder => shape.scalar_type.exact_rem(left, right),
        IntegerBinaryKind::WrappingDivide => shape.scalar_type.wrapping_div(left, right),
        IntegerBinaryKind::WrappingRemainder => shape.scalar_type.wrapping_rem(left, right),
        IntegerBinaryKind::SaturatingDivide => shape.scalar_type.saturating_div(left, right),
        IntegerBinaryKind::SaturatingRemainder => shape.scalar_type.saturating_rem(left, right),
        IntegerBinaryKind::ExactShiftLeft => shape.scalar_type.exact_shift_left(
            left,
            shape.count_type.expect("shift count type"),
            right,
        ),
        IntegerBinaryKind::ExactShiftRight => shape.scalar_type.exact_shift_right(
            left,
            shape.count_type.expect("shift count type"),
            right,
        ),
        IntegerBinaryKind::WrappingShiftLeft => shape.scalar_type.wrapping_shift_left(
            left,
            shape.count_type.expect("shift count type"),
            right,
        ),
        IntegerBinaryKind::WrappingShiftRight => shape.scalar_type.wrapping_shift_right(
            left,
            shape.count_type.expect("shift count type"),
            right,
        ),
        IntegerBinaryKind::BitwiseAnd => shape.scalar_type.bitwise_and(left, right),
        IntegerBinaryKind::BitwiseOr => shape.scalar_type.bitwise_or(left, right),
        IntegerBinaryKind::BitwiseXor => shape.scalar_type.bitwise_xor(left, right),
    }
}
