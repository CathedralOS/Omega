//! Scalar operations whose payload is a closed sub-tagged family: trapping
//! integer primitives and float result policies. Fixed-tag scalar operations
//! are rows of `operation_wire!`.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use semantic_vocabulary::ValueId;
use terminal_psi::FloatResultPolicy;
use terminal_psi::{OperationKind, TrappingIntegerOperation};

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
