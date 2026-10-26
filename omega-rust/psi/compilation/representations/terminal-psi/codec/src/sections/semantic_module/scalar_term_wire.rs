//! Canonical recursive scalar-term wire format.
//!
//! This module owns exact scalar-term variant tags, recursive operand order,
//! structural field paths, and decode-time constructor validation. Scalar
//! primitive encodings remain in the sibling scalar wire module.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerType, PropositionError, ScalarReadProjection, ScalarTerm,
};

use super::scalar_wire::{
    decode_integer_type, decode_integer_value, decode_scalar_type, encode_integer_type,
    encode_integer_value, encode_scalar_type,
};
use super::structural_field_wire::{
    decode_canonical_structural_field, decode_ieee_float_format, encode_canonical_structural_field,
    encode_ieee_float_format,
};
use super::wire::{Reader, Writer, decode_counted};
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
        ScalarTerm::StructuralRead {
            root,
            path,
            scalar_type,
        } => {
            term.validate().map_err(CodecError::MalformedProposition)?;
            writer.u8(40);
            writer.id(*root);
            encode_scalar_type(writer, *scalar_type);
            writer.len("scalar read path", path.len())?;
            for projection in path {
                match projection {
                    ScalarReadProjection::Referent => writer.u8(5),
                    ScalarReadProjection::Extent => writer.u8(6),
                    ScalarReadProjection::Static(CanonicalStructuralPathSegment::Field(field)) => {
                        writer.u8(1);
                        writer.id(*field);
                    }
                    ScalarReadProjection::Static(CanonicalStructuralPathSegment::FixedIndex(
                        index,
                    )) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                    ScalarReadProjection::Static(CanonicalStructuralPathSegment::Case(case)) => {
                        writer.u8(3);
                        writer.id(*case);
                    }
                    ScalarReadProjection::Static(CanonicalStructuralPathSegment::Referent) => {
                        return Err(CodecError::MalformedProposition(
                            semantic_vocabulary::PropositionError::NonCanonicalScalarReadProjection,
                        ));
                    }
                    ScalarReadProjection::Index(index) => {
                        writer.u8(4);
                        encode_scalar_term(writer, index, depth + 1)?;
                    }
                }
            }
        }
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
            encode_canonical_structural_field(writer, *root, path, "Boolean field path")?;
        }
        ScalarTerm::IntegerField {
            root,
            path,
            scalar_type,
        } => {
            writer.u8(35);
            encode_canonical_structural_field(writer, *root, path, "Integer field path")?;
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
            encode_canonical_structural_field(writer, *root, path, "View extent path")?;
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
        } => encode_typed_binary_term(writer, 12, *scalar_type, left, right, depth)?,
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 13, *scalar_type, left, right, depth)?,
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 14, *scalar_type, left, right, depth)?,
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => encode_typed_unary_term(writer, 20, *scalar_type, operand, depth)?,
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => encode_cast_term(writer, 21, *source_type, *target_type, operand, depth)?,
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => encode_cast_term(writer, 22, *source_type, *target_type, operand, depth)?,
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 15, *scalar_type, left, right, depth)?,
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 16, *scalar_type, left, right, depth)?,
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 17, *scalar_type, left, right, depth)?,
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => encode_shift_term(writer, 18, *value_type, *count_type, value, count, depth)?,
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => encode_shift_term(writer, 19, *value_type, *count_type, value, count, depth)?,
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => encode_shift_term(writer, 23, *value_type, *count_type, value, count, depth)?,
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => encode_shift_term(writer, 24, *value_type, *count_type, value, count, depth)?,
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 25, *scalar_type, left, right, depth)?,
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 26, *scalar_type, left, right, depth)?,
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 27, *scalar_type, left, right, depth)?,
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 28, *scalar_type, left, right, depth)?,
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 29, *scalar_type, left, right, depth)?,
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 30, *scalar_type, left, right, depth)?,
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 31, *scalar_type, left, right, depth)?,
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 32, *scalar_type, left, right, depth)?,
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 33, *scalar_type, left, right, depth)?,
        ScalarTerm::Integer { scalar_type, value } => {
            writer.u8(3);
            encode_integer_type(writer, *scalar_type);
            encode_integer_value(writer, *value);
        }
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 4, *scalar_type, left, right, depth)?,
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 5, *scalar_type, left, right, depth)?,
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 6, *scalar_type, left, right, depth)?,
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 7, *scalar_type, left, right, depth)?,
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 8, *scalar_type, left, right, depth)?,
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_typed_binary_term(writer, 9, *scalar_type, left, right, depth)?,
    }
    Ok(())
}

pub(crate) fn decode_scalar_terms(reader: &mut Reader<'_>) -> Result<Vec<ScalarTerm>, CodecError> {
    decode_counted(reader, |reader| decode_scalar_term(reader, 0))
}

fn encode_typed_binary_term(
    writer: &mut Writer,
    tag: u8,
    scalar_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    depth: usize,
) -> Result<(), CodecError> {
    writer.u8(tag);
    encode_integer_type(writer, scalar_type);
    encode_scalar_term(writer, left, depth + 1)?;
    encode_scalar_term(writer, right, depth + 1)
}

fn encode_typed_unary_term(
    writer: &mut Writer,
    tag: u8,
    scalar_type: IntegerType,
    operand: &ScalarTerm,
    depth: usize,
) -> Result<(), CodecError> {
    writer.u8(tag);
    encode_integer_type(writer, scalar_type);
    encode_scalar_term(writer, operand, depth + 1)
}

fn encode_shift_term(
    writer: &mut Writer,
    tag: u8,
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
    depth: usize,
) -> Result<(), CodecError> {
    writer.u8(tag);
    encode_integer_type(writer, value_type);
    encode_integer_type(writer, count_type);
    encode_scalar_term(writer, value, depth + 1)?;
    encode_scalar_term(writer, count, depth + 1)
}

fn encode_cast_term(
    writer: &mut Writer,
    tag: u8,
    source_type: IntegerType,
    target_type: IntegerType,
    operand: &ScalarTerm,
    depth: usize,
) -> Result<(), CodecError> {
    writer.u8(tag);
    encode_integer_type(writer, source_type);
    encode_integer_type(writer, target_type);
    encode_scalar_term(writer, operand, depth + 1)
}

fn decode_typed_binary_term(
    reader: &mut Reader<'_>,
    depth: usize,
    constructor: fn(IntegerType, ScalarTerm, ScalarTerm) -> Result<ScalarTerm, PropositionError>,
) -> Result<ScalarTerm, CodecError> {
    let scalar_type = decode_integer_type(reader)?;
    let left = decode_scalar_term(reader, depth + 1)?;
    let right = decode_scalar_term(reader, depth + 1)?;
    constructor(scalar_type, left, right).map_err(CodecError::MalformedProposition)
}

fn decode_shift_term(
    reader: &mut Reader<'_>,
    depth: usize,
    constructor: fn(
        IntegerType,
        IntegerType,
        ScalarTerm,
        ScalarTerm,
    ) -> Result<ScalarTerm, PropositionError>,
) -> Result<ScalarTerm, CodecError> {
    let value_type = decode_integer_type(reader)?;
    let count_type = decode_integer_type(reader)?;
    let value = decode_scalar_term(reader, depth + 1)?;
    let count = decode_scalar_term(reader, depth + 1)?;
    constructor(value_type, count_type, value, count).map_err(CodecError::MalformedProposition)
}

fn decode_cast_term(
    reader: &mut Reader<'_>,
    depth: usize,
    constructor: fn(IntegerType, IntegerType, ScalarTerm) -> Result<ScalarTerm, PropositionError>,
) -> Result<ScalarTerm, CodecError> {
    let source_type = decode_integer_type(reader)?;
    let target_type = decode_integer_type(reader)?;
    let operand = decode_scalar_term(reader, depth + 1)?;
    constructor(source_type, target_type, operand).map_err(CodecError::MalformedProposition)
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
        4 => decode_typed_binary_term(reader, depth, ScalarTerm::wrapping_integer_add)?,
        5 => decode_typed_binary_term(reader, depth, ScalarTerm::saturating_integer_add)?,
        6 => decode_typed_binary_term(reader, depth, ScalarTerm::wrapping_integer_subtract)?,
        7 => decode_typed_binary_term(reader, depth, ScalarTerm::saturating_integer_subtract)?,
        8 => decode_typed_binary_term(reader, depth, ScalarTerm::wrapping_integer_multiply)?,
        9 => decode_typed_binary_term(reader, depth, ScalarTerm::saturating_integer_multiply)?,
        10 => ScalarTerm::boolean_not(decode_scalar_term(reader, depth + 1)?)
            .map_err(CodecError::MalformedProposition)?,
        11 => {
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::boolean_equal(left, right).map_err(CodecError::MalformedProposition)?
        }
        12 => decode_typed_binary_term(reader, depth, ScalarTerm::integer_equal)?,
        13 => decode_typed_binary_term(reader, depth, ScalarTerm::integer_less_than)?,
        14 => decode_typed_binary_term(reader, depth, ScalarTerm::integer_less_or_equal)?,
        15 => decode_typed_binary_term(reader, depth, ScalarTerm::integer_bitwise_and)?,
        16 => decode_typed_binary_term(reader, depth, ScalarTerm::integer_bitwise_or)?,
        17 => decode_typed_binary_term(reader, depth, ScalarTerm::integer_bitwise_xor)?,
        18 => decode_shift_term(reader, depth, ScalarTerm::wrapping_integer_shift_left)?,
        19 => decode_shift_term(reader, depth, ScalarTerm::wrapping_integer_shift_right)?,
        20 => {
            let scalar_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_not(scalar_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        21 => decode_cast_term(reader, depth, ScalarTerm::integer_widen)?,
        22 => decode_cast_term(reader, depth, ScalarTerm::integer_exact_cast)?,
        23 => decode_shift_term(reader, depth, ScalarTerm::exact_integer_shift_right)?,
        24 => decode_shift_term(reader, depth, ScalarTerm::exact_integer_shift_left)?,
        25 => decode_typed_binary_term(reader, depth, ScalarTerm::exact_integer_add)?,
        26 => decode_typed_binary_term(reader, depth, ScalarTerm::exact_integer_subtract)?,
        27 => decode_typed_binary_term(reader, depth, ScalarTerm::exact_integer_multiply)?,
        28 => decode_typed_binary_term(reader, depth, ScalarTerm::exact_integer_divide)?,
        29 => decode_typed_binary_term(reader, depth, ScalarTerm::exact_integer_remainder)?,
        30 => decode_typed_binary_term(reader, depth, ScalarTerm::wrapping_integer_divide)?,
        31 => decode_typed_binary_term(reader, depth, ScalarTerm::wrapping_integer_remainder)?,
        32 => decode_typed_binary_term(reader, depth, ScalarTerm::saturating_integer_divide)?,
        33 => decode_typed_binary_term(reader, depth, ScalarTerm::saturating_integer_remainder)?,
        34 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            ScalarTerm::boolean_field_path(root, path)
        }
        35 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            ScalarTerm::integer_field_path(root, path, decode_integer_type(reader)?)
        }
        36 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
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
        40 => {
            let root = reader.id("PlaceId")?;
            let scalar_type = decode_scalar_type(reader)?;
            let path = decode_counted(reader, |reader| {
                Ok(match reader.u8()? {
                    1 => ScalarReadProjection::Static(CanonicalStructuralPathSegment::Field(
                        reader.id("StructuralFieldId")?,
                    )),
                    2 => ScalarReadProjection::Static(CanonicalStructuralPathSegment::FixedIndex(
                        reader.u64()?,
                    )),
                    3 => ScalarReadProjection::Static(CanonicalStructuralPathSegment::Case(
                        reader.id("StructuralCaseId")?,
                    )),
                    4 => ScalarReadProjection::Index(Box::new(decode_scalar_term(
                        reader,
                        depth + 1,
                    )?)),
                    5 => ScalarReadProjection::Referent,
                    6 => ScalarReadProjection::Extent,
                    tag => return Err(CodecError::InvalidTag("ScalarReadProjection", tag)),
                })
            })?;
            ScalarTerm::structural_read(root, path, scalar_type)
                .map_err(CodecError::MalformedProposition)?
        }
        tag => return Err(CodecError::InvalidTag("ScalarTerm", tag)),
    })
}

#[cfg(test)]
mod scalar_read_projection_tests {
    use super::*;
    use semantic_vocabulary::{PlaceId, ScalarType};

    #[test]
    fn explicit_reference_read_has_an_exact_canonical_wire_roundtrip() {
        let term = ScalarTerm::structural_read(
            PlaceId::new(1).unwrap(),
            vec![ScalarReadProjection::Referent],
            ScalarType::Boolean,
        )
        .unwrap();
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, &term, 0).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let decoded = decode_scalar_term(&mut reader, 0).unwrap();
        assert_eq!(decoded, term);
        assert_eq!(reader.remaining(), 0);
        let mut repeated = Writer::default();
        encode_scalar_term(&mut repeated, &decoded, 0).unwrap();
        assert_eq!(repeated.finish(), bytes);
    }

    #[test]
    fn static_reference_alias_cannot_be_published_on_the_scalar_read_wire() {
        let term = ScalarTerm::StructuralRead {
            root: PlaceId::new(1).unwrap(),
            path: vec![ScalarReadProjection::Static(
                CanonicalStructuralPathSegment::Referent,
            )],
            scalar_type: ScalarType::Boolean,
        };
        assert!(matches!(
            encode_scalar_term(&mut Writer::default(), &term, 0),
            Err(CodecError::MalformedProposition(
                PropositionError::NonCanonicalScalarReadProjection
            ))
        ));
    }
}
