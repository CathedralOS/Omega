//! Scalar operations on the wire: Boolean and integer comparisons and
//! logic, wrapping, saturating and exact integer arithmetic and shifts, casts
//! and widening, and IEEE float comparison and fused multiply-add.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use semantic_vocabulary::IeeeFloatComparisonOperation;
use semantic_vocabulary::{ObligationId, ValueId};
use terminal_psi::{OperationKind, TrappingIntegerOperation};

pub(super) fn encode_ieee_float_compare(
    writer: &mut Writer,
    comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::IEEE_FLOAT_COMPARE);
    writer.u8(match comparison {
        IeeeFloatComparisonOperation::Equal => 0,
        IeeeFloatComparisonOperation::NotEqual => 1,
        IeeeFloatComparisonOperation::Less => 2,
        IeeeFloatComparisonOperation::LessOrEqual => 3,
        IeeeFloatComparisonOperation::Greater => 4,
        IeeeFloatComparisonOperation::GreaterOrEqual => 5,
    });
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_ieee_float_compare(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IeeeFloatCompare {
        comparison: match reader.u8()? {
            0 => IeeeFloatComparisonOperation::Equal,
            1 => IeeeFloatComparisonOperation::NotEqual,
            2 => IeeeFloatComparisonOperation::Less,
            3 => IeeeFloatComparisonOperation::LessOrEqual,
            4 => IeeeFloatComparisonOperation::Greater,
            5 => IeeeFloatComparisonOperation::GreaterOrEqual,
            tag => return Err(CodecError::InvalidTag("IeeeFloatComparisonOperation", tag)),
        },
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_nearest_ieee_float_fused_multiply_add(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    addend: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD);
    writer.id(left);
    writer.id(right);
    writer.id(addend);
    Ok(())
}

pub(super) fn decode_nearest_ieee_float_fused_multiply_add(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::NearestIeeeFloatFusedMultiplyAdd {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        addend: reader.id("ValueId")?,
    })
}

pub(super) fn encode_boolean_not(writer: &mut Writer, operand: ValueId) -> Result<(), CodecError> {
    writer.u8(operation_tags::BOOLEAN_NOT);
    writer.id(operand);
    Ok(())
}

pub(super) fn decode_boolean_not(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::BooleanNot {
        operand: reader.id("ValueId")?,
    })
}

pub(super) fn encode_boolean_equal(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BOOLEAN_EQUAL);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_boolean_equal(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::BooleanEqual {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_integer_equal(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_EQUAL);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_integer_equal(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerEqual {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_integer_less_than(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_LESS_THAN);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_integer_less_than(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerLessThan {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_integer_less_or_equal(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_LESS_OR_EQUAL);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_integer_less_or_equal(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerLessOrEqual {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_integer_bitwise_not(
    writer: &mut Writer,
    operand: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_BITWISE_NOT);
    writer.id(operand);
    Ok(())
}

pub(super) fn decode_integer_bitwise_not(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerBitwiseNot {
        operand: reader.id("ValueId")?,
    })
}

pub(super) fn encode_integer_widen(
    writer: &mut Writer,
    operand: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_WIDEN);
    writer.id(operand);
    Ok(())
}

pub(super) fn decode_integer_widen(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerWiden {
        operand: reader.id("ValueId")?,
    })
}

pub(super) fn encode_integer_exact_cast(
    writer: &mut Writer,
    operand: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_EXACT_CAST);
    writer.id(operand);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_integer_exact_cast(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerExactCast {
        operand: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_integer_bitwise_and(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_BITWISE_AND);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_integer_bitwise_and(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerBitwiseAnd {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_integer_bitwise_or(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_BITWISE_OR);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_integer_bitwise_or(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerBitwiseOr {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_integer_bitwise_xor(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_BITWISE_XOR);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_integer_bitwise_xor(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerBitwiseXor {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_wrapping_integer_shift_left(
    writer: &mut Writer,
    value: ValueId,
    count: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::WRAPPING_INTEGER_SHIFT_LEFT);
    writer.id(value);
    writer.id(count);
    Ok(())
}

pub(super) fn decode_wrapping_integer_shift_left(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WrappingIntegerShiftLeft {
        value: reader.id("ValueId")?,
        count: reader.id("ValueId")?,
    })
}

pub(super) fn encode_wrapping_integer_shift_right(
    writer: &mut Writer,
    value: ValueId,
    count: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::WRAPPING_INTEGER_SHIFT_RIGHT);
    writer.id(value);
    writer.id(count);
    Ok(())
}

pub(super) fn decode_wrapping_integer_shift_right(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WrappingIntegerShiftRight {
        value: reader.id("ValueId")?,
        count: reader.id("ValueId")?,
    })
}

pub(super) fn encode_exact_integer_shift_left(
    writer: &mut Writer,
    value: ValueId,
    count: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::EXACT_INTEGER_SHIFT_LEFT);
    writer.id(value);
    writer.id(count);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_exact_integer_shift_left(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ExactIntegerShiftLeft {
        value: reader.id("ValueId")?,
        count: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_exact_integer_shift_right(
    writer: &mut Writer,
    value: ValueId,
    count: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::EXACT_INTEGER_SHIFT_RIGHT);
    writer.id(value);
    writer.id(count);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_exact_integer_shift_right(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ExactIntegerShiftRight {
        value: reader.id("ValueId")?,
        count: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_exact_integer_add(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::EXACT_INTEGER_ADD);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_exact_integer_add(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ExactIntegerAdd {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_exact_integer_subtract(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::EXACT_INTEGER_SUBTRACT);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_exact_integer_subtract(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ExactIntegerSubtract {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_exact_integer_multiply(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::EXACT_INTEGER_MULTIPLY);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_exact_integer_multiply(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ExactIntegerMultiply {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_exact_integer_divide(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::EXACT_INTEGER_DIVIDE);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_exact_integer_divide(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ExactIntegerDivide {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_exact_integer_remainder(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::EXACT_INTEGER_REMAINDER);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_exact_integer_remainder(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ExactIntegerRemainder {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_wrapping_integer_divide(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::WRAPPING_INTEGER_DIVIDE);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_wrapping_integer_divide(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WrappingIntegerDivide {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_wrapping_integer_remainder(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::WRAPPING_INTEGER_REMAINDER);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_wrapping_integer_remainder(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WrappingIntegerRemainder {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_saturating_integer_divide(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::SATURATING_INTEGER_DIVIDE);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_saturating_integer_divide(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::SaturatingIntegerDivide {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_saturating_integer_remainder(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::SATURATING_INTEGER_REMAINDER);
    writer.id(left);
    writer.id(right);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_saturating_integer_remainder(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::SaturatingIntegerRemainder {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_wrapping_integer_add(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::WRAPPING_INTEGER_ADD);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_wrapping_integer_add(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WrappingIntegerAdd {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_saturating_integer_add(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::SATURATING_INTEGER_ADD);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_saturating_integer_add(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::SaturatingIntegerAdd {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_wrapping_integer_subtract(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::WRAPPING_INTEGER_SUBTRACT);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_wrapping_integer_subtract(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WrappingIntegerSubtract {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_saturating_integer_subtract(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::SATURATING_INTEGER_SUBTRACT);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_saturating_integer_subtract(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::SaturatingIntegerSubtract {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_wrapping_integer_multiply(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::WRAPPING_INTEGER_MULTIPLY);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_wrapping_integer_multiply(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WrappingIntegerMultiply {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

pub(super) fn encode_saturating_integer_multiply(
    writer: &mut Writer,
    left: ValueId,
    right: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::SATURATING_INTEGER_MULTIPLY);
    writer.id(left);
    writer.id(right);
    Ok(())
}

pub(super) fn decode_saturating_integer_multiply(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::SaturatingIntegerMultiply {
        left: reader.id("ValueId")?,
        right: reader.id("ValueId")?,
    })
}

/// One Trapping primitive: the family tag, a closed primitive sub-tag, then
/// the primitive's operands in evaluation order. The crash site is the
/// operation itself, so no cause, guard, or frontier rides on the wire.
pub(super) fn encode_trapping_integer(
    writer: &mut Writer,
    operation: TrappingIntegerOperation,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::TRAPPING_INTEGER);
    let (primitive, operands): (u8, &[ValueId]) = match &operation {
        TrappingIntegerOperation::Add { left, right } => (1, &[*left, *right]),
        TrappingIntegerOperation::Subtract { left, right } => (2, &[*left, *right]),
        TrappingIntegerOperation::Multiply { left, right } => (3, &[*left, *right]),
        TrappingIntegerOperation::Divide { left, right } => (4, &[*left, *right]),
        TrappingIntegerOperation::Remainder { left, right } => (5, &[*left, *right]),
        TrappingIntegerOperation::ShiftLeft { value, count } => (6, &[*value, *count]),
        TrappingIntegerOperation::ShiftRight { value, count } => (7, &[*value, *count]),
        TrappingIntegerOperation::Convert { operand } => (8, &[*operand]),
    };
    writer.u8(primitive);
    for operand in operands {
        writer.id(*operand);
    }
    Ok(())
}

pub(super) fn decode_trapping_integer(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let primitive = reader.u8()?;
    let binary = |reader: &mut Reader<'_>| -> Result<(ValueId, ValueId), CodecError> {
        Ok((reader.id("ValueId")?, reader.id("ValueId")?))
    };
    let operation = match primitive {
        1 => {
            let (left, right) = binary(reader)?;
            TrappingIntegerOperation::Add { left, right }
        }
        2 => {
            let (left, right) = binary(reader)?;
            TrappingIntegerOperation::Subtract { left, right }
        }
        3 => {
            let (left, right) = binary(reader)?;
            TrappingIntegerOperation::Multiply { left, right }
        }
        4 => {
            let (left, right) = binary(reader)?;
            TrappingIntegerOperation::Divide { left, right }
        }
        5 => {
            let (left, right) = binary(reader)?;
            TrappingIntegerOperation::Remainder { left, right }
        }
        6 => {
            let (value, count) = binary(reader)?;
            TrappingIntegerOperation::ShiftLeft { value, count }
        }
        7 => {
            let (value, count) = binary(reader)?;
            TrappingIntegerOperation::ShiftRight { value, count }
        }
        8 => TrappingIntegerOperation::Convert {
            operand: reader.id("ValueId")?,
        },
        tag => return Err(CodecError::InvalidTag("TrappingIntegerOperation", tag)),
    };
    Ok(OperationKind::TrappingInteger { operation })
}
