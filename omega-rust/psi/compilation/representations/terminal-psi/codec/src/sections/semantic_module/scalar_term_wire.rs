//! Canonical recursive scalar-term wire format.
//!
//! This module owns exact scalar-term variant tags, recursive operand order,
//! structural field paths, and decode-time constructor validation. Scalar
//! primitive encodings remain in the sibling scalar wire module.

use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarTerm};

#[cfg(test)]
mod ieee_field_tests {
    use super::*;
    use semantic_vocabulary::{IeeeFloatFormat, PlaceId, StructuralCaseId, StructuralFieldId};

    fn encode(term: &ScalarTerm) -> Vec<u8> {
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, term, 0).unwrap();
        writer.finish()
    }

    #[test]
    fn ieee_float_field_wire_preserves_exact_format_root_and_path() {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let original = ScalarTerm::ieee_float_field_path(
                PlaceId::new(11).unwrap(),
                vec![
                    CanonicalStructuralPathSegment::Field(StructuralFieldId::new(12).unwrap()),
                    CanonicalStructuralPathSegment::FixedIndex(13),
                    CanonicalStructuralPathSegment::Case(StructuralCaseId::new(14).unwrap()),
                ],
                format,
            );
            let bytes = encode(&original);
            assert_eq!(bytes[0], 38);
            assert_eq!(
                decode_scalar_term(&mut Reader::new(&bytes), 0).unwrap(),
                original
            );
            for length in 0..bytes.len() {
                assert!(decode_scalar_term(&mut Reader::new(&bytes[..length]), 0).is_err());
            }
            let mut wrong_format = bytes.clone();
            *wrong_format.last_mut().unwrap() = 255;
            assert!(decode_scalar_term(&mut Reader::new(&wrong_format), 0).is_err());
            let mut changed = original.clone();
            let ScalarTerm::IeeeFloatField { root, path, .. } = &mut changed else {
                unreachable!()
            };
            *root = PlaceId::new(21).unwrap();
            path[0] = CanonicalStructuralPathSegment::Field(StructuralFieldId::new(22).unwrap());
            let changed_bytes = encode(&changed);
            assert_ne!(bytes, changed_bytes);
            assert_eq!(
                decode_scalar_term(&mut Reader::new(&changed_bytes), 0).unwrap(),
                changed
            );
        }
    }

    #[test]
    fn ieee_float_field_wire_rejects_empty_paths() {
        let term = ScalarTerm::ieee_float_field_path(
            PlaceId::new(1).unwrap(),
            vec![],
            IeeeFloatFormat::Binary32,
        );
        assert!(encode_scalar_term(&mut Writer::default(), &term, 0).is_err());
        let mut writer = Writer::default();
        writer.u8(38);
        writer.id(PlaceId::new(1).unwrap());
        writer.u32(0);
        writer.u8(1);
        assert!(decode_scalar_term(&mut Reader::new(&writer.finish()), 0).is_err());
    }
}

use super::scalar_wire::{
    decode_integer_type, decode_integer_value, decode_scalar_type, encode_integer_type,
    encode_integer_value, encode_scalar_type,
};
use super::structural_field_wire::{
    decode_canonical_structural_field, decode_ieee_float_format, encode_canonical_structural_field,
    encode_ieee_float_format,
};
use super::wire::{Reader, Writer};
use super::{CodecError, MAX_SCALAR_TERM_DEPTH};

pub(crate) fn encode_scalar_terms(
    writer: &mut Writer,
    terms: &[ScalarTerm],
) -> Result<(), CodecError> {
    writer.len("scalar terms", terms.len())?;
    for term in terms {
        encode_scalar_term(writer, term, 0)?;
    }
    Ok(())
}

pub(crate) fn encode_scalar_term(
    writer: &mut Writer,
    term: &ScalarTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ScalarTerm::IeeeFloatField { root, path, format } => {
            term.validate().map_err(CodecError::MalformedProposition)?;
            writer.u8(38);
            encode_canonical_structural_field(writer, *root, path, "IEEE float field path")?;
            encode_ieee_float_format(writer, *format);
        }
        ScalarTerm::IeeeFloatLiteral(value) => {
            writer.u8(37);
            match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                    writer.u8(1);
                    writer.u32(*bits);
                }
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                    writer.u8(2);
                    writer.u64(*bits);
                }
            }
        }
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
        ScalarTerm::ByteSequencePredicate {
            predicate,
            root,
            path,
        } => {
            writer.u8(39);
            writer.u8(predicate.mask_bit());
            encode_canonical_structural_field(writer, *root, path, "Byte predicate path")?;
        }
        ScalarTerm::ViewExtent {
            root,
            path,
            scalar_type,
        } => {
            writer.u8(36);
            writer.id(*root);
            writer.len("View extent path", path.len())?;
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

pub(crate) fn decode_scalar_terms(reader: &mut Reader<'_>) -> Result<Vec<ScalarTerm>, CodecError> {
    let count = reader.count()?;
    let mut terms = Vec::with_capacity(usize::try_from(count).expect("u32 count fits usize"));
    for _ in 0..count {
        terms.push(decode_scalar_term(reader, 0)?);
    }
    Ok(terms)
}

pub(crate) fn decode_scalar_term(
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
        36 => {
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
            ScalarTerm::view_extent(root, path, decode_integer_type(reader)?)
        }
        37 => ScalarTerm::IeeeFloatLiteral(match reader.u8()? {
            1 => semantic_vocabulary::IeeeFloatValue::Binary32(reader.u32()?),
            2 => semantic_vocabulary::IeeeFloatValue::Binary64(reader.u64()?),
            tag => return Err(CodecError::InvalidTag("IeeeFloatValue", tag)),
        }),
        38 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            let term =
                ScalarTerm::ieee_float_field_path(root, path, decode_ieee_float_format(reader)?);
            term.validate().map_err(CodecError::MalformedProposition)?;
            term
        }
        39 => {
            let predicate = match reader.u8()? {
                1 => semantic_vocabulary::ByteSequencePredicate::ValidUtf8,
                2 => semantic_vocabulary::ByteSequencePredicate::NoNul,
                4 => semantic_vocabulary::ByteSequencePredicate::AsciiOnly,
                8 => semantic_vocabulary::ByteSequencePredicate::NonEmpty,
                tag => return Err(CodecError::InvalidTag("ByteSequencePredicate", tag)),
            };
            let (root, path) = decode_canonical_structural_field(reader)?;
            ScalarTerm::byte_sequence_predicate(predicate, root, path)
        }
        tag => return Err(CodecError::InvalidTag("ScalarTerm", tag)),
    })
}

#[cfg(test)]
mod byte_predicate_tests {
    use super::*;
    use semantic_vocabulary::{
        ByteSequencePredicate as Predicate, PlaceId, StructuralCaseId, StructuralFieldId,
    };

    fn encode(term: &ScalarTerm) -> Vec<u8> {
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, term, 0).unwrap();
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<ScalarTerm, CodecError> {
        decode_scalar_term(&mut Reader::new(bytes), 0)
    }

    #[test]
    fn byte_predicate_wire_preserves_whole_and_projected_observation_identity() {
        let root = PlaceId::new(11).unwrap();
        let projection = vec![
            CanonicalStructuralPathSegment::Field(StructuralFieldId::new(12).unwrap()),
            CanonicalStructuralPathSegment::FixedIndex(13),
            CanonicalStructuralPathSegment::Case(StructuralCaseId::new(14).unwrap()),
        ];
        for predicate in Predicate::ALL {
            for path in [vec![], projection.clone()] {
                let term = ScalarTerm::byte_sequence_predicate(predicate, root, path.clone());
                let bytes = encode(&term);
                assert_eq!(bytes[0], 39);
                assert_eq!(decode(&bytes).unwrap(), term);
                for end in 0..bytes.len() {
                    assert!(decode(&bytes[..end]).is_err());
                }
                let mut changed_path = path.clone();
                changed_path.push(CanonicalStructuralPathSegment::FixedIndex(99));
                let changed_kind = if predicate == Predicate::ValidUtf8 {
                    Predicate::NoNul
                } else {
                    Predicate::ValidUtf8
                };
                for changed in [
                    ScalarTerm::byte_sequence_predicate(changed_kind, root, path.clone()),
                    ScalarTerm::byte_sequence_predicate(predicate, PlaceId::new(21).unwrap(), path),
                    ScalarTerm::byte_sequence_predicate(predicate, root, changed_path),
                ] {
                    assert_ne!(bytes, encode(&changed));
                    assert_eq!(decode(&encode(&changed)).unwrap(), changed);
                }
            }
        }
    }

    #[test]
    fn byte_predicate_wire_rejects_unknown_kind_zero_root_and_bad_path() {
        let term = ScalarTerm::byte_sequence_predicate(
            Predicate::ValidUtf8,
            PlaceId::new(1).unwrap(),
            vec![],
        );
        let bytes = encode(&term);
        for kind in [0, 3, 16, 255] {
            let mut malformed = bytes.clone();
            malformed[1] = kind;
            assert!(decode(&malformed).is_err());
        }
        let mut writer = Writer::default();
        writer.u8(39);
        writer.u8(1);
        writer.u64(0);
        writer.u32(0);
        assert!(decode(&writer.finish()).is_err());
        let mut writer = Writer::default();
        writer.u8(39);
        writer.u8(1);
        writer.id(PlaceId::new(1).unwrap());
        writer.u32(1);
        writer.u8(255);
        assert!(decode(&writer.finish()).is_err());
    }
}
