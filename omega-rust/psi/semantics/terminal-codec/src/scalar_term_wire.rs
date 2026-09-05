//! Canonical recursive scalar-term wire format.
//!
//! This module owns exact scalar-term variant tags, recursive operand order,
//! structural field paths, and decode-time constructor validation. Scalar
//! primitive encodings remain in the sibling scalar wire module.

use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarTerm};

use super::scalar_wire::{
    decode_integer_type, decode_integer_value, decode_scalar_type, encode_integer_type,
    encode_integer_value, encode_scalar_type,
};
use super::wire::{Reader, Writer};
use super::{CodecError, MAX_SCALAR_TERM_DEPTH};

pub(super) fn encode_scalar_term(
    writer: &mut Writer,
    term: &ScalarTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ScalarTerm::Value { id, scalar_type } => {
            writer.u8(1);
            writer.id(*id);
            encode_scalar_type(writer, *scalar_type);
        }
        ScalarTerm::BooleanField { root, path } => {
            writer.u8(34);
            writer.id(*root);
            writer.len("Boolean field path", path.len())?;
            for segment in path {
                match segment {
                    CanonicalStructuralPathSegment::Field(field) => {
                        writer.u8(1);
                        writer.id(*field);
                    }
                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                    CanonicalStructuralPathSegment::Case(case) => {
                        writer.u8(3);
                        writer.id(*case);
                    }
                }
            }
        }
        ScalarTerm::IntegerField {
            root,
            path,
            scalar_type,
        } => {
            writer.u8(35);
            writer.id(*root);
            writer.len("Integer field path", path.len())?;
            for segment in path {
                match segment {
                    CanonicalStructuralPathSegment::Field(field) => {
                        writer.u8(1);
                        writer.id(*field);
                    }
                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                    CanonicalStructuralPathSegment::Case(case) => {
                        writer.u8(3);
                        writer.id(*case);
                    }
                }
            }
            encode_integer_type(writer, *scalar_type);
        }
        ScalarTerm::Boolean(value) => {
            writer.u8(2);
            writer.u8(u8::from(*value));
        }
        ScalarTerm::BooleanNot { operand } => {
            writer.u8(10);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::BooleanEqual { left, right } => {
            writer.u8(11);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(12);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(13);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(14);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            writer.u8(20);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(21);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(22);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(15);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(16);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(17);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(18);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(19);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(23);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(24);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(25);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(26);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(27);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(28);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(29);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(30);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(31);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(32);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(33);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::Integer { scalar_type, value } => {
            writer.u8(3);
            encode_integer_type(writer, *scalar_type);
            encode_integer_value(writer, *value);
        }
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(4);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(5);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(6);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(7);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(8);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(9);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
    }
    Ok(())
}

pub(super) fn decode_scalar_term(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ScalarTerm, CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => ScalarTerm::value(reader.id("ValueId")?, decode_scalar_type(reader)?),
        2 => ScalarTerm::boolean(reader.boolean()?),
        3 => {
            let scalar_type = decode_integer_type(reader)?;
            let value = decode_integer_value(reader)?;
            ScalarTerm::integer(scalar_type, value).map_err(CodecError::MalformedProposition)?
        }
        4 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        5 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        6 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        7 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        8 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        9 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        10 => ScalarTerm::boolean_not(decode_scalar_term(reader, depth + 1)?)
            .map_err(CodecError::MalformedProposition)?,
        11 => {
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::boolean_equal(left, right).map_err(CodecError::MalformedProposition)?
        }
        12 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_equal(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        13 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_less_than(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        14 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_less_or_equal(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        15 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_and(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        16 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_or(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        17 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_xor(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        18 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_shift_left(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        19 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_shift_right(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        20 => {
            let scalar_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_not(scalar_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        21 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_widen(source_type, target_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        22 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        23 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        24 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        25 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        26 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        27 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        28 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        29 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        30 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        31 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        32 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        33 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        34 => {
            let root = reader.id("PlaceId")?;
            let count = reader.count()?;
            let mut path = Vec::new();
            for _ in 0..count {
                path.push(match reader.u8()? {
                    1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
                    2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
                    3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "CanonicalStructuralPathSegment",
                            tag,
                        ));
                    }
                });
            }
            ScalarTerm::boolean_field_path(root, path)
        }
        35 => {
            let root = reader.id("PlaceId")?;
            let count = reader.count()?;
            let mut path = Vec::new();
            for _ in 0..count {
                path.push(match reader.u8()? {
                    1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
                    2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
                    3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "CanonicalStructuralPathSegment",
                            tag,
                        ));
                    }
                });
            }
            ScalarTerm::integer_field_path(root, path, decode_integer_type(reader)?)
        }
        tag => return Err(CodecError::InvalidTag("ScalarTerm", tag)),
    })
}
