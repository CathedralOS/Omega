//! Scalar operations on the wire: Boolean and integer comparisons and
//! logic, wrapping, saturating and exact integer arithmetic and shifts, casts
//! and widening, and IEEE float comparison and fused multiply-add.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use semantic_vocabulary::IeeeFloatComparisonOperation;
use semantic_vocabulary::{ObligationId, ValueId};
use terminal_psi::FloatResultPolicy;
use terminal_psi::{OperationKind, TrappingIntegerOperation};

// Fixed-tag scalar payloads declare their ordered, typed IDs beside the decoder.
macro_rules! encode_scalar_ids {
    ($name:ident => $tag:ident, $($field:ident : $ty:ty),+ $(,)?) => {
        pub(super) fn $name(writer: &mut Writer, $($field: $ty),+) -> Result<(), CodecError> {
            writer.u8(operation_tags::$tag);
            $(writer.id($field);)+
            Ok(())
        }
    };
}

// The matching decoders read the same ordered, named ID fields into one variant.
macro_rules! decode_scalar_ids {
    ($name:ident => $variant:ident, $($field:ident : $id:literal),+ $(,)?) => {
        pub(super) fn $name(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
            Ok(OperationKind::$variant {
                $($field: reader.id($id)?,)+
            })
        }
    };
}

macro_rules! encode_pair {
    ($name:ident => $tag:ident) => {
        encode_scalar_ids!($name => $tag, left: ValueId, right: ValueId);
    };
}
macro_rules! encode_compared {
    ($name:ident => $tag:ident) => {
        encode_scalar_ids!(
            $name => $tag,
            left: ValueId,
            right: ValueId,
            obligation: ObligationId,
        );
    };
}
macro_rules! encode_operand {
    ($name:ident => $tag:ident) => {
        encode_scalar_ids!($name => $tag, operand: ValueId);
    };
}
macro_rules! encode_shift {
    ($name:ident => $tag:ident) => {
        encode_scalar_ids!($name => $tag, value: ValueId, count: ValueId);
    };
}

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

/// A total bit re-view spells tag 101; a `{0, 1}` landing on `bool` spells
/// tag 102 and its obligation. Neither tag encodes the carriers: they are the
/// operand's and the result's declarations, which the verifier pairs.
pub(super) fn encode_scalar_representation_view(
    writer: &mut Writer,
    operand: ValueId,
    landing_obligation: Option<ObligationId>,
) -> Result<(), CodecError> {
    writer.u8(if landing_obligation.is_some() {
        operation_tags::LANDED_SCALAR_REPRESENTATION_VIEW
    } else {
        operation_tags::SCALAR_REPRESENTATION_VIEW
    });
    writer.id(operand);
    if let Some(obligation) = landing_obligation {
        writer.id(obligation);
    }
    Ok(())
}

pub(super) fn decode_scalar_representation_view(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ScalarRepresentationView {
        operand: reader.id("ValueId")?,
        landing_obligation: None,
    })
}

pub(super) fn decode_landed_scalar_representation_view(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ScalarRepresentationView {
        operand: reader.id("ValueId")?,
        landing_obligation: Some(reader.id("ObligationId")?),
    })
}

encode_scalar_ids!(
    encode_nearest_ieee_float_fused_multiply_add => NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD,
    left: ValueId, right: ValueId, addend: ValueId,
);

decode_scalar_ids!(decode_nearest_ieee_float_fused_multiply_add => NearestIeeeFloatFusedMultiplyAdd, left: "ValueId", right: "ValueId", addend: "ValueId");

encode_operand!(encode_boolean_not => BOOLEAN_NOT);

decode_scalar_ids!(decode_boolean_not => BooleanNot, operand: "ValueId");

encode_pair!(encode_boolean_equal => BOOLEAN_EQUAL);

decode_scalar_ids!(decode_boolean_equal => BooleanEqual, left: "ValueId", right: "ValueId");

encode_pair!(encode_integer_equal => INTEGER_EQUAL);

decode_scalar_ids!(decode_integer_equal => IntegerEqual, left: "ValueId", right: "ValueId");

encode_pair!(encode_integer_less_than => INTEGER_LESS_THAN);

decode_scalar_ids!(decode_integer_less_than => IntegerLessThan, left: "ValueId", right: "ValueId");

encode_pair!(encode_integer_less_or_equal => INTEGER_LESS_OR_EQUAL);

decode_scalar_ids!(decode_integer_less_or_equal => IntegerLessOrEqual, left: "ValueId", right: "ValueId");

encode_operand!(encode_integer_bitwise_not => INTEGER_BITWISE_NOT);

decode_scalar_ids!(decode_integer_bitwise_not => IntegerBitwiseNot, operand: "ValueId");

encode_operand!(encode_integer_widen => INTEGER_WIDEN);

decode_scalar_ids!(decode_integer_widen => IntegerWiden, operand: "ValueId");

encode_scalar_ids!(
    encode_integer_exact_cast => INTEGER_EXACT_CAST,
    operand: ValueId, obligation: ObligationId,
);

decode_scalar_ids!(decode_integer_exact_cast => IntegerExactCast, operand: "ValueId", obligation: "ObligationId");

encode_pair!(encode_integer_bitwise_and => INTEGER_BITWISE_AND);

decode_scalar_ids!(decode_integer_bitwise_and => IntegerBitwiseAnd, left: "ValueId", right: "ValueId");

encode_pair!(encode_integer_bitwise_or => INTEGER_BITWISE_OR);

decode_scalar_ids!(decode_integer_bitwise_or => IntegerBitwiseOr, left: "ValueId", right: "ValueId");

encode_pair!(encode_integer_bitwise_xor => INTEGER_BITWISE_XOR);

decode_scalar_ids!(decode_integer_bitwise_xor => IntegerBitwiseXor, left: "ValueId", right: "ValueId");

encode_shift!(encode_wrapping_integer_shift_left => WRAPPING_INTEGER_SHIFT_LEFT);

decode_scalar_ids!(decode_wrapping_integer_shift_left => WrappingIntegerShiftLeft, value: "ValueId", count: "ValueId");

encode_shift!(encode_wrapping_integer_shift_right => WRAPPING_INTEGER_SHIFT_RIGHT);

decode_scalar_ids!(decode_wrapping_integer_shift_right => WrappingIntegerShiftRight, value: "ValueId", count: "ValueId");

encode_scalar_ids!(
    encode_exact_integer_shift_left => EXACT_INTEGER_SHIFT_LEFT,
    value: ValueId, count: ValueId, obligation: ObligationId,
);

decode_scalar_ids!(decode_exact_integer_shift_left => ExactIntegerShiftLeft, value: "ValueId", count: "ValueId", obligation: "ObligationId");

encode_scalar_ids!(
    encode_exact_integer_shift_right => EXACT_INTEGER_SHIFT_RIGHT,
    value: ValueId, count: ValueId, obligation: ObligationId,
);

decode_scalar_ids!(decode_exact_integer_shift_right => ExactIntegerShiftRight, value: "ValueId", count: "ValueId", obligation: "ObligationId");

encode_compared!(encode_exact_integer_add => EXACT_INTEGER_ADD);

decode_scalar_ids!(decode_exact_integer_add => ExactIntegerAdd, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_compared!(encode_exact_integer_subtract => EXACT_INTEGER_SUBTRACT);

decode_scalar_ids!(decode_exact_integer_subtract => ExactIntegerSubtract, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_compared!(encode_exact_integer_multiply => EXACT_INTEGER_MULTIPLY);

decode_scalar_ids!(decode_exact_integer_multiply => ExactIntegerMultiply, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_compared!(encode_exact_integer_divide => EXACT_INTEGER_DIVIDE);

decode_scalar_ids!(decode_exact_integer_divide => ExactIntegerDivide, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_compared!(encode_exact_integer_remainder => EXACT_INTEGER_REMAINDER);

decode_scalar_ids!(decode_exact_integer_remainder => ExactIntegerRemainder, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_compared!(encode_wrapping_integer_divide => WRAPPING_INTEGER_DIVIDE);

decode_scalar_ids!(decode_wrapping_integer_divide => WrappingIntegerDivide, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_compared!(encode_wrapping_integer_remainder => WRAPPING_INTEGER_REMAINDER);

decode_scalar_ids!(decode_wrapping_integer_remainder => WrappingIntegerRemainder, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_compared!(encode_saturating_integer_divide => SATURATING_INTEGER_DIVIDE);

decode_scalar_ids!(decode_saturating_integer_divide => SaturatingIntegerDivide, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_compared!(encode_saturating_integer_remainder => SATURATING_INTEGER_REMAINDER);

decode_scalar_ids!(decode_saturating_integer_remainder => SaturatingIntegerRemainder, left: "ValueId", right: "ValueId", obligation: "ObligationId");

encode_pair!(encode_wrapping_integer_add => WRAPPING_INTEGER_ADD);

decode_scalar_ids!(decode_wrapping_integer_add => WrappingIntegerAdd, left: "ValueId", right: "ValueId");

encode_pair!(encode_saturating_integer_add => SATURATING_INTEGER_ADD);

decode_scalar_ids!(decode_saturating_integer_add => SaturatingIntegerAdd, left: "ValueId", right: "ValueId");

encode_pair!(encode_wrapping_integer_subtract => WRAPPING_INTEGER_SUBTRACT);

decode_scalar_ids!(decode_wrapping_integer_subtract => WrappingIntegerSubtract, left: "ValueId", right: "ValueId");

encode_pair!(encode_saturating_integer_subtract => SATURATING_INTEGER_SUBTRACT);

decode_scalar_ids!(decode_saturating_integer_subtract => SaturatingIntegerSubtract, left: "ValueId", right: "ValueId");

encode_pair!(encode_wrapping_integer_multiply => WRAPPING_INTEGER_MULTIPLY);

decode_scalar_ids!(decode_wrapping_integer_multiply => WrappingIntegerMultiply, left: "ValueId", right: "ValueId");

encode_pair!(encode_saturating_integer_multiply => SATURATING_INTEGER_MULTIPLY);

decode_scalar_ids!(decode_saturating_integer_multiply => SaturatingIntegerMultiply, left: "ValueId", right: "ValueId");

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

/// Result format belongs to its declaration. Eligibility is already Bool.
/// Saturating tags 2 and 3 described the retired binary operand/rule payloads;
/// tag 4 is a new wire shape, so old artifacts fail with InvalidTag.
pub(super) fn encode_float_result_policy(
    writer: &mut Writer,
    policy: &FloatResultPolicy,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::FLOAT_RESULT_POLICY);
    match policy {
        FloatResultPolicy::Trapping { raw_result } => {
            writer.u8(1);
            writer.id(*raw_result);
        }
        FloatResultPolicy::Saturating {
            raw_result,
            eligible,
        } => {
            writer.u8(4);
            writer.id(*raw_result);
            writer.id(*eligible);
        }
    }
    Ok(())
}
pub(super) fn decode_float_result_policy(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let policy = match reader.u8()? {
        1 => FloatResultPolicy::Trapping {
            raw_result: reader.id("ValueId")?,
        },
        4 => FloatResultPolicy::Saturating {
            raw_result: reader.id("ValueId")?,
            eligible: reader.id("ValueId")?,
        },
        tag => return Err(CodecError::InvalidTag("FloatResultPolicy", tag)),
    };
    Ok(OperationKind::FloatResultPolicy { policy })
}
