//! Scalar terms, scalar types, integer values, primitives and IEEE float
//! fields on the wire.

use crate::sections::proof_bundle::ProofCodecError;

#[cfg(test)]
mod ieee_field_tests {
    use super::*;
    use semantic_vocabulary::{IeeeFloatFormat, PlaceId, StructuralCaseId, StructuralFieldId};

    fn encode(term: &ScalarTerm) -> Vec<u8> {
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, term, 0, super::super::FORMAT_MARKER).unwrap();
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
                decode_scalar_term(&mut Reader::new(&bytes), 0, super::super::FORMAT_MARKER)
                    .unwrap(),
                original
            );
            for length in 0..bytes.len() {
                assert!(
                    decode_scalar_term(
                        &mut Reader::new(&bytes[..length]),
                        0,
                        super::super::FORMAT_MARKER
                    )
                    .is_err()
                );
            }
            let mut wrong_format = bytes.clone();
            *wrong_format.last_mut().unwrap() = 255;
            assert!(
                decode_scalar_term(
                    &mut Reader::new(&wrong_format),
                    0,
                    super::super::FORMAT_MARKER
                )
                .is_err()
            );
            let mut changed = original.clone();
            let ScalarTerm::IeeeFloatField { root, path, .. } = &mut changed else {
                unreachable!()
            };
            *root = PlaceId::new(21).unwrap();
            path[0] = CanonicalStructuralPathSegment::Field(StructuralFieldId::new(22).unwrap());
            let changed_bytes = encode(&changed);
            assert_ne!(bytes, changed_bytes);
            assert_eq!(
                decode_scalar_term(
                    &mut Reader::new(&changed_bytes),
                    0,
                    super::super::FORMAT_MARKER
                )
                .unwrap(),
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
        assert!(
            encode_scalar_term(
                &mut Writer::default(),
                &term,
                0,
                super::super::FORMAT_MARKER
            )
            .is_err()
        );
        let mut writer = Writer::default();
        writer.u8(38);
        writer.id(PlaceId::new(1).unwrap());
        writer.u32(0);
        writer.u8(1);
        assert!(
            decode_scalar_term(
                &mut Reader::new(&writer.finish()),
                0,
                super::super::FORMAT_MARKER
            )
            .is_err()
        );
    }
}
use crate::sections::proof_bundle::proposition_codec::{
    decode_canonical_structural_field, encode_canonical_structural_field,
};
use crate::sections::proof_bundle::wire::{Reader, Writer};
use proof_admission::PrimitiveJudgment;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IeeeFloatComparisonKind, IeeeFloatFormat,
    IeeeFloatStructuralField, IntegerCarrier, IntegerMathLiteral, IntegerMathTerm, IntegerSign,
    IntegerType, IntegerValue, ScalarTerm, ScalarType,
};

pub(crate) const MAX_SCALAR_TERM_DEPTH: usize = 256;

pub(crate) fn encode_ieee_float_format(writer: &mut Writer, format: IeeeFloatFormat) {
    writer.u8(match format {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}

pub(crate) fn encode_ieee_float_comparison_kind(
    writer: &mut Writer,
    kind: IeeeFloatComparisonKind,
) {
    writer.u8(match kind {
        IeeeFloatComparisonKind::Equal => 1,
        IeeeFloatComparisonKind::NotEqual => 2,
        IeeeFloatComparisonKind::LessThan => 3,
        IeeeFloatComparisonKind::LessOrEqual => 4,
        IeeeFloatComparisonKind::NotLessThan => 5,
        IeeeFloatComparisonKind::NotLessOrEqual => 6,
    });
}

pub(crate) fn decode_ieee_float_comparison_kind(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatComparisonKind, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatComparisonKind::Equal),
        2 => Ok(IeeeFloatComparisonKind::NotEqual),
        3 => Ok(IeeeFloatComparisonKind::LessThan),
        4 => Ok(IeeeFloatComparisonKind::LessOrEqual),
        5 => Ok(IeeeFloatComparisonKind::NotLessThan),
        6 => Ok(IeeeFloatComparisonKind::NotLessOrEqual),
        tag => Err(ProofCodecError::InvalidTag("IeeeFloatComparisonKind", tag)),
    }
}

pub(crate) fn decode_ieee_float_format(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatFormat, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatFormat::Binary32),
        2 => Ok(IeeeFloatFormat::Binary64),
        tag => Err(ProofCodecError::InvalidTag("IeeeFloatFormat", tag)),
    }
}

pub(crate) fn encode_ieee_float_field(
    writer: &mut Writer,
    field: &IeeeFloatStructuralField,
) -> Result<(), ProofCodecError> {
    encode_canonical_structural_field(writer, field.root(), field.path(), "IEEE float field path")
}

pub(crate) fn decode_ieee_float_field(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatStructuralField, ProofCodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    IeeeFloatStructuralField::new(root, path).map_err(ProofCodecError::MalformedProposition)
}

#[allow(
    clippy::only_used_in_recursion,
    reason = "the format marker is deliberately threaded through recursive proof encoding"
)]
pub(crate) fn encode_scalar_term(
    writer: &mut Writer,
    term: &ScalarTerm,
    depth: usize,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ScalarTerm::IeeeFloatField { root, path, format } => {
            term.validate()
                .map_err(ProofCodecError::MalformedProposition)?;
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
            encode_scalar_term(writer, operand, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(25);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(26);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(27);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(28);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(29);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(30);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(31);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(32);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(33);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
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
            encode_scalar_term(writer, value, depth + 1, format_marker)?;
            encode_scalar_term(writer, count, depth + 1, format_marker)?;
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
            encode_scalar_term(writer, value, depth + 1, format_marker)?;
            encode_scalar_term(writer, count, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(22);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1, format_marker)?;
        }
        ScalarTerm::BooleanEqual { left, right } => {
            writer.u8(11);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(12);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(13);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(14);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(match term {
                ScalarTerm::IntegerBitwiseAnd { .. } => 15,
                ScalarTerm::IntegerBitwiseOr { .. } => 16,
                ScalarTerm::IntegerBitwiseXor { .. } => 17,
                _ => unreachable!(),
            });
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        }
        | ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(match term {
                ScalarTerm::WrappingIntegerShiftLeft { .. } => 18,
                ScalarTerm::WrappingIntegerShiftRight { .. } => 19,
                _ => unreachable!(),
            });
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1, format_marker)?;
            encode_scalar_term(writer, count, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            writer.u8(20);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, operand, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(21);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1, format_marker)?;
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
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(5);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(6);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(7);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(8);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(9);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
    }
    Ok(())
}

fn encode_scalar_type(writer: &mut Writer, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => writer.u8(1),
        ScalarType::Integer(integer_type) => {
            writer.u8(2);
            encode_integer_type(writer, integer_type);
        }
        ScalarType::IeeeFloat(format) => {
            writer.u8(3);
            encode_ieee_float_format(writer, format);
        }
    }
}

fn encode_integer_type(writer: &mut Writer, integer_type: IntegerType) {
    writer.u8(match (integer_type.carrier(), integer_type.sign()) {
        (IntegerCarrier::Fixed, IntegerSign::Signed) => 1,
        (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 2,
        (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
        (IntegerCarrier::Address, IntegerSign::Signed) => {
            unreachable!("address carriers are unsigned")
        }
    });
    writer.u16(integer_type.bits());
}

fn encode_integer_value(writer: &mut Writer, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            writer.u8(1);
            writer.bytes(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            writer.u8(2);
            writer.bytes(&value.to_le_bytes());
        }
    }
}

pub(crate) fn encode_primitive(writer: &mut Writer, judgment: PrimitiveJudgment) {
    writer.u8(match judgment {
        PrimitiveJudgment::Truth => 1,
        PrimitiveJudgment::ReflexiveEquality => 2,
        PrimitiveJudgment::ClosedIntegerRelation => 3,
        PrimitiveJudgment::IntegerCarrierBound => 4,
    });
}

pub(crate) fn encode_integer_math_term(
    writer: &mut Writer,
    term: &IntegerMathTerm,
    depth: usize,
) -> Result<(), ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    match term {
        IntegerMathTerm::IntegerLiteral(literal) => {
            writer.u8(1);
            writer.u8(u8::from(literal.negative()));
            writer.bytes(&literal.magnitude().to_le_bytes());
        }
        IntegerMathTerm::MathValue { source_type, value } => {
            writer.u8(2);
            encode_integer_type(writer, *source_type);
            writer.id(*value);
        }
        IntegerMathTerm::Add(left, right)
        | IntegerMathTerm::Subtract(left, right)
        | IntegerMathTerm::Multiply(left, right) => {
            writer.u8(match term {
                IntegerMathTerm::Add(_, _) => 3,
                IntegerMathTerm::Subtract(_, _) => 4,
                IntegerMathTerm::Multiply(_, _) => 5,
                _ => unreachable!(),
            });
            encode_integer_math_term(writer, left, depth + 1)?;
            encode_integer_math_term(writer, right, depth + 1)?;
        }
        IntegerMathTerm::ShiftLeft { value, count } => {
            writer.u8(6);
            encode_integer_math_term(writer, value, depth + 1)?;
            encode_integer_math_term(writer, count, depth + 1)?;
        }
    }
    Ok(())
}

pub(crate) fn decode_integer_math_term(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<IntegerMathTerm, ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => IntegerMathTerm::IntegerLiteral(
            IntegerMathLiteral::new(
                match reader.u8()? {
                    0 => false,
                    1 => true,
                    tag => return Err(ProofCodecError::InvalidTag("Boolean", tag)),
                },
                u128::from_le_bytes(reader.array()?),
            )
            .map_err(ProofCodecError::MalformedProposition)?,
        ),
        2 => IntegerMathTerm::MathValue {
            source_type: decode_integer_type(reader)?,
            value: reader.id("ValueId")?,
        },
        3 => IntegerMathTerm::Add(
            Box::new(decode_integer_math_term(reader, depth + 1)?),
            Box::new(decode_integer_math_term(reader, depth + 1)?),
        ),
        4 => IntegerMathTerm::Subtract(
            Box::new(decode_integer_math_term(reader, depth + 1)?),
            Box::new(decode_integer_math_term(reader, depth + 1)?),
        ),
        5 => IntegerMathTerm::Multiply(
            Box::new(decode_integer_math_term(reader, depth + 1)?),
            Box::new(decode_integer_math_term(reader, depth + 1)?),
        ),
        6 => IntegerMathTerm::ShiftLeft {
            value: Box::new(decode_integer_math_term(reader, depth + 1)?),
            count: Box::new(decode_integer_math_term(reader, depth + 1)?),
        },
        tag => return Err(ProofCodecError::InvalidTag("IntegerMathTerm", tag)),
    })
}

#[allow(
    clippy::only_used_in_recursion,
    reason = "the format marker is deliberately threaded through recursive proof decoding"
)]
pub(crate) fn decode_scalar_term(
    reader: &mut Reader<'_>,
    depth: usize,
    format_marker: u16,
) -> Result<ScalarTerm, ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => ScalarTerm::value(reader.id("ValueId")?, decode_scalar_type(reader)?),
        2 => ScalarTerm::boolean(reader.boolean()?),
        3 => {
            let scalar_type = decode_integer_type(reader)?;
            let value = decode_integer_value(reader)?;
            ScalarTerm::integer(scalar_type, value)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        4 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        5 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        6 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        7 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        8 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        9 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        10 => ScalarTerm::boolean_not(decode_scalar_term(reader, depth + 1, format_marker)?)
            .map_err(ProofCodecError::MalformedProposition)?,
        11 => {
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::boolean_equal(left, right).map_err(ProofCodecError::MalformedProposition)?
        }
        12 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_equal(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        13 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_less_than(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        14 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_less_or_equal(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        15 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_bitwise_and(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        16 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_bitwise_or(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        17 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_bitwise_xor(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        18 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_marker)?;
            let count = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_shift_left(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        19 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_marker)?;
            let count = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_shift_right(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        20 => {
            let scalar_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_bitwise_not(scalar_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        21 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_widen(source_type, target_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        22 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        23 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_marker)?;
            let count = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        24 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_marker)?;
            let count = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        25 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        26 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        27 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        28 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_divide(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        29 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_remainder(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        30 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_divide(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        31 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_remainder(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        32 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_divide(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        33 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_remainder(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
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
                        return Err(ProofCodecError::InvalidTag(
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
                        return Err(ProofCodecError::InvalidTag(
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
                        return Err(ProofCodecError::InvalidTag(
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
            tag => return Err(ProofCodecError::InvalidTag("IeeeFloatValue", tag)),
        }),
        38 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            let term =
                ScalarTerm::ieee_float_field_path(root, path, decode_ieee_float_format(reader)?);
            term.validate()
                .map_err(ProofCodecError::MalformedProposition)?;
            term
        }
        39 => {
            let predicate = match reader.u8()? {
                1 => semantic_vocabulary::ByteSequencePredicate::ValidUtf8,
                2 => semantic_vocabulary::ByteSequencePredicate::NoNul,
                4 => semantic_vocabulary::ByteSequencePredicate::AsciiOnly,
                8 => semantic_vocabulary::ByteSequencePredicate::NonEmpty,
                tag => return Err(ProofCodecError::InvalidTag("ByteSequencePredicate", tag)),
            };
            let (root, path) = decode_canonical_structural_field(reader)?;
            ScalarTerm::byte_sequence_predicate(predicate, root, path)
        }
        tag => return Err(ProofCodecError::InvalidTag("ScalarTerm", tag)),
    })
}

fn decode_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, ProofCodecError> {
    Ok(match reader.u8()? {
        1 => ScalarType::Boolean,
        2 => ScalarType::Integer(decode_integer_type(reader)?),
        3 => ScalarType::IeeeFloat(decode_ieee_float_format(reader)?),
        tag => return Err(ProofCodecError::InvalidTag("ScalarType", tag)),
    })
}

fn decode_integer_type(reader: &mut Reader<'_>) -> Result<IntegerType, ProofCodecError> {
    let tag = reader.u8()?;
    let bits = reader.u16()?;
    match tag {
        1 => IntegerType::new(IntegerSign::Signed, bits),
        2 => IntegerType::new(IntegerSign::Unsigned, bits),
        3 => IntegerType::address(bits),
        tag => return Err(ProofCodecError::InvalidTag("IntegerSign", tag)),
    }
    .map_err(ProofCodecError::MalformedProposition)
}

fn decode_integer_value(reader: &mut Reader<'_>) -> Result<IntegerValue, ProofCodecError> {
    Ok(match reader.u8()? {
        1 => IntegerValue::Signed(i128::from_le_bytes(reader.array()?)),
        2 => IntegerValue::Unsigned(u128::from_le_bytes(reader.array()?)),
        tag => return Err(ProofCodecError::InvalidTag("IntegerValue", tag)),
    })
}

pub(crate) fn decode_primitive(
    reader: &mut Reader<'_>,
) -> Result<PrimitiveJudgment, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(PrimitiveJudgment::Truth),
        2 => Ok(PrimitiveJudgment::ReflexiveEquality),
        3 => Ok(PrimitiveJudgment::ClosedIntegerRelation),
        4 => Ok(PrimitiveJudgment::IntegerCarrierBound),
        tag => Err(ProofCodecError::InvalidTag("PrimitiveJudgment", tag)),
    }
}

#[cfg(test)]
mod byte_predicate_tests {
    use super::*;
    use semantic_vocabulary::{
        ByteSequencePredicate as Predicate, PlaceId, StructuralCaseId, StructuralFieldId,
    };

    fn encode(term: &ScalarTerm) -> Vec<u8> {
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, term, 0, super::super::FORMAT_MARKER).unwrap();
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<ScalarTerm, ProofCodecError> {
        decode_scalar_term(&mut Reader::new(bytes), 0, super::super::FORMAT_MARKER)
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
