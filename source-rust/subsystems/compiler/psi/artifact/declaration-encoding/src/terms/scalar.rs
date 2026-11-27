use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerType, PropositionError, ScalarReadProjection, ScalarTerm,
};

use crate::CodecError;
use crate::structural_fields::{
    decode_canonical_structural_field, decode_ieee_float_format, encode_canonical_structural_field,
    encode_ieee_float_format,
};
use crate::terms::scalar_types::{
    decode_integer_type, decode_integer_value, decode_scalar_type, encode_integer_type,
    encode_integer_value, encode_scalar_type,
};
use crate::terms::table::{TableChildren, TermEncoding, decode_term_table, encode_term_table};
use crate::wire::{Reader, Writer, decode_counted};

pub fn encode_scalar_terms(writer: &mut Writer, terms: &[ScalarTerm]) -> Result<(), CodecError> {
    writer.len("scalar terms", terms.len())?;
    for term in terms {
        encode_scalar_term(writer, term)?;
    }
    Ok(())
}

/// Encodes one scalar term as a postorder table (`crate::terms::table`).
pub fn encode_scalar_term(writer: &mut Writer, term: &ScalarTerm) -> Result<(), CodecError> {
    encode_term_table::<ScalarTermTable>(writer, term)
}

pub fn decode_scalar_terms(reader: &mut Reader<'_>) -> Result<Vec<ScalarTerm>, CodecError> {
    decode_counted(reader, decode_scalar_term)
}

/// Canonical ordering retains symbolic widths during checking.
pub fn canonical_scalar_term_order_key(term: &ScalarTerm) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::checking_identity();
    encode_scalar_term(&mut writer, term)?;
    Ok(writer.finish())
}

pub fn decode_scalar_term(reader: &mut Reader<'_>) -> Result<ScalarTerm, CodecError> {
    decode_term_table::<ScalarTermTable>(reader)
}

struct ScalarTermTable;

impl TermEncoding for ScalarTermTable {
    type Term = ScalarTerm;
    const LABEL: &'static str = "scalar term";

    fn children<'term>(term: &'term Self::Term, children: &mut Vec<&'term Self::Term>) {
        children.extend(term.operands());
    }

    fn write_row(term: &Self::Term, writer: &mut Writer) -> Result<(), CodecError> {
        write_scalar_row(writer, term)
    }

    fn read_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, Self::Term>,
    ) -> Result<Self::Term, CodecError> {
        read_scalar_row(reader, children)
    }
}

/// One row: the form tag and fields; operand handles follow in the table.
/// Every node passes its local check, as the decoder's constructors require.
fn write_scalar_row(writer: &mut Writer, term: &ScalarTerm) -> Result<(), CodecError> {
    term.validate_after_operands()
        .map_err(CodecError::MalformedProposition)?;
    match term {
        ScalarTerm::DeclarationApplication { declaration, arguments, scalar_type } => {
            if !writer.is_checking_identity() {
                return Err(CodecError::MalformedStructuralFoundation(
                    "an erased declaration application needs its checked signature for publication",
                ));
            }
            writer.u8(42);
            encode_scalar_type(writer, *scalar_type)?;
            writer.boolean(declaration.supplier.is_some());
            if let Some((slot, generation)) = declaration.supplier {
                writer.u32(slot);
                writer.u32(generation);
            }
            writer.u32(declaration.member.0);
            writer.u32(declaration.member.1);
            writer.len("erased declaration arguments", arguments.len())?;
        }
        ScalarTerm::Application {
            application,
            scalar_type,
        } => {
            writer.u8(41);
            writer.id(*application);
            encode_scalar_type(writer, *scalar_type)?;
        }
        ScalarTerm::StructuralRead {
            root,
            path,
            scalar_type,
        } => {
            writer.u8(40);
            writer.id(*root);
            encode_scalar_type(writer, *scalar_type)?;
            writer.len("scalar read path", path.len())?;
            for projection in path {
                match projection {
                    ScalarReadProjection::Referent => writer.u8(5),
                    ScalarReadProjection::Extent => writer.u8(6),
                    ScalarReadProjection::Alignment => writer.u8(7),
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
                    ScalarReadProjection::Index(_) => writer.u8(4),
                }
            }
        }
        ScalarTerm::IeeeFloatField {
            version,
            root,
            path,
            format,
        } => {
            writer.u8(match version {
                semantic_vocabulary::ContentPlaceVersion::Current => 38,
                semantic_vocabulary::ContentPlaceVersion::Entry => 44,
            });
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
            encode_scalar_type(writer, *scalar_type)?;
        }
        ScalarTerm::BooleanField {
            version,
            root,
            path,
        } => {
            writer.u8(match version {
                semantic_vocabulary::ContentPlaceVersion::Current => 34,
                semantic_vocabulary::ContentPlaceVersion::Entry => 42,
            });
            encode_canonical_structural_field(writer, *root, path, "Boolean field path")?;
        }
        ScalarTerm::IntegerField {
            version,
            root,
            path,
            scalar_type,
        } => {
            writer.u8(match version {
                semantic_vocabulary::ContentPlaceVersion::Current => 35,
                semantic_vocabulary::ContentPlaceVersion::Entry => 43,
            });
            encode_canonical_structural_field(writer, *root, path, "Integer field path")?;
            encode_integer_type(writer, *scalar_type)?;
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
            encode_integer_type(writer, *scalar_type)?;
        }
        ScalarTerm::Boolean(value) => {
            writer.u8(2);
            writer.u8(u8::from(*value));
        }
        ScalarTerm::BooleanNot { .. } => writer.u8(10),
        ScalarTerm::BooleanEqual { .. } => writer.u8(11),
        ScalarTerm::IntegerBitwiseNot { scalar_type, .. } => {
            writer.u8(20);
            encode_integer_type(writer, *scalar_type)?;
        }
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            ..
        } => typed_pair_row(writer, 21, *source_type, *target_type)?,
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            ..
        } => typed_pair_row(writer, 22, *source_type, *target_type)?,
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            ..
        } => typed_pair_row(writer, 18, *value_type, *count_type)?,
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            ..
        } => typed_pair_row(writer, 19, *value_type, *count_type)?,
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            ..
        } => typed_pair_row(writer, 23, *value_type, *count_type)?,
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            ..
        } => typed_pair_row(writer, 24, *value_type, *count_type)?,
        ScalarTerm::Integer { scalar_type, value } => {
            writer.u8(3);
            encode_integer_type(writer, *scalar_type)?;
            encode_integer_value(writer, *value);
        }
        ScalarTerm::IntegerEqual { scalar_type, .. } => typed_row(writer, 12, *scalar_type)?,
        ScalarTerm::IntegerLessThan { scalar_type, .. } => typed_row(writer, 13, *scalar_type)?,
        ScalarTerm::IntegerLessOrEqual { scalar_type, .. } => typed_row(writer, 14, *scalar_type)?,
        ScalarTerm::IntegerBitwiseAnd { scalar_type, .. } => typed_row(writer, 15, *scalar_type)?,
        ScalarTerm::IntegerBitwiseOr { scalar_type, .. } => typed_row(writer, 16, *scalar_type)?,
        ScalarTerm::IntegerBitwiseXor { scalar_type, .. } => typed_row(writer, 17, *scalar_type)?,
        ScalarTerm::ExactIntegerAdd { scalar_type, .. } => typed_row(writer, 25, *scalar_type)?,
        ScalarTerm::ExactIntegerSubtract { scalar_type, .. } => {
            typed_row(writer, 26, *scalar_type)?
        }
        ScalarTerm::ExactIntegerMultiply { scalar_type, .. } => {
            typed_row(writer, 27, *scalar_type)?
        }
        ScalarTerm::ExactIntegerDivide { scalar_type, .. } => typed_row(writer, 28, *scalar_type)?,
        ScalarTerm::ExactIntegerRemainder { scalar_type, .. } => {
            typed_row(writer, 29, *scalar_type)?
        }
        ScalarTerm::WrappingIntegerDivide { scalar_type, .. } => {
            typed_row(writer, 30, *scalar_type)?
        }
        ScalarTerm::WrappingIntegerRemainder { scalar_type, .. } => {
            typed_row(writer, 31, *scalar_type)?
        }
        ScalarTerm::SaturatingIntegerDivide { scalar_type, .. } => {
            typed_row(writer, 32, *scalar_type)?
        }
        ScalarTerm::SaturatingIntegerRemainder { scalar_type, .. } => {
            typed_row(writer, 33, *scalar_type)?
        }
        ScalarTerm::WrappingIntegerAdd { scalar_type, .. } => typed_row(writer, 4, *scalar_type)?,
        ScalarTerm::SaturatingIntegerAdd { scalar_type, .. } => typed_row(writer, 5, *scalar_type)?,
        ScalarTerm::WrappingIntegerSubtract { scalar_type, .. } => {
            typed_row(writer, 6, *scalar_type)?
        }
        ScalarTerm::SaturatingIntegerSubtract { scalar_type, .. } => {
            typed_row(writer, 7, *scalar_type)?
        }
        ScalarTerm::WrappingIntegerMultiply { scalar_type, .. } => {
            typed_row(writer, 8, *scalar_type)?
        }
        ScalarTerm::SaturatingIntegerMultiply { scalar_type, .. } => {
            typed_row(writer, 9, *scalar_type)?
        }
    }
    Ok(())
}

fn typed_row(writer: &mut Writer, tag: u8, scalar_type: IntegerType) -> Result<(), CodecError> {
    writer.u8(tag);
    encode_integer_type(writer, scalar_type)?;
    Ok(())
}

fn typed_pair_row(
    writer: &mut Writer,
    tag: u8,
    first: IntegerType,
    second: IntegerType,
) -> Result<(), CodecError> {
    writer.u8(tag);
    encode_integer_type(writer, first)?;
    encode_integer_type(writer, second)?;
    Ok(())
}

type Binary = fn(IntegerType, ScalarTerm, ScalarTerm) -> Result<ScalarTerm, PropositionError>;
type TypedPairBinary =
    fn(IntegerType, IntegerType, ScalarTerm, ScalarTerm) -> Result<ScalarTerm, PropositionError>;
type TypedPairUnary =
    fn(IntegerType, IntegerType, ScalarTerm) -> Result<ScalarTerm, PropositionError>;

fn two(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, ScalarTerm>,
) -> Result<(ScalarTerm, ScalarTerm), CodecError> {
    let mut operands = children.take(reader, 2)?.into_iter();
    match (operands.next(), operands.next()) {
        (Some(left), Some(right)) => Ok((left, right)),
        _ => Err(CodecError::NonCanonicalTermTable("scalar term")),
    }
}

fn one(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, ScalarTerm>,
) -> Result<ScalarTerm, CodecError> {
    children
        .take(reader, 1)?
        .pop()
        .ok_or(CodecError::NonCanonicalTermTable("scalar term"))
}

fn binary_row(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, ScalarTerm>,
    constructor: Binary,
) -> Result<ScalarTerm, CodecError> {
    let scalar_type = decode_integer_type(reader)?;
    let (left, right) = two(reader, children)?;
    constructor(scalar_type, left, right).map_err(CodecError::MalformedProposition)
}

fn typed_pair_binary_row(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, ScalarTerm>,
    constructor: TypedPairBinary,
) -> Result<ScalarTerm, CodecError> {
    let first = decode_integer_type(reader)?;
    let second = decode_integer_type(reader)?;
    let (value, count) = two(reader, children)?;
    constructor(first, second, value, count).map_err(CodecError::MalformedProposition)
}

fn typed_pair_unary_row(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, ScalarTerm>,
    constructor: TypedPairUnary,
) -> Result<ScalarTerm, CodecError> {
    let source = decode_integer_type(reader)?;
    let target = decode_integer_type(reader)?;
    let operand = one(reader, children)?;
    constructor(source, target, operand).map_err(CodecError::MalformedProposition)
}

fn read_scalar_row(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, ScalarTerm>,
) -> Result<ScalarTerm, CodecError> {
    Ok(match reader.u8()? {
        41 => ScalarTerm::Application {
            application: reader.id("ScalarApplicationId")?,
            scalar_type: decode_scalar_type(reader)?,
        },
        1 => ScalarTerm::value(reader.id("ValueId")?, decode_scalar_type(reader)?),
        2 => ScalarTerm::boolean(reader.boolean()?),
        3 => {
            let scalar_type = decode_integer_type(reader)?;
            let value = decode_integer_value(reader)?;
            ScalarTerm::integer(scalar_type, value).map_err(CodecError::MalformedProposition)?
        }
        4 => binary_row(reader, children, ScalarTerm::wrapping_integer_add)?,
        5 => binary_row(reader, children, ScalarTerm::saturating_integer_add)?,
        6 => binary_row(reader, children, ScalarTerm::wrapping_integer_subtract)?,
        7 => binary_row(reader, children, ScalarTerm::saturating_integer_subtract)?,
        8 => binary_row(reader, children, ScalarTerm::wrapping_integer_multiply)?,
        9 => binary_row(reader, children, ScalarTerm::saturating_integer_multiply)?,
        10 => ScalarTerm::boolean_not(one(reader, children)?)
            .map_err(CodecError::MalformedProposition)?,
        11 => {
            let (left, right) = two(reader, children)?;
            ScalarTerm::boolean_equal(left, right).map_err(CodecError::MalformedProposition)?
        }
        12 => binary_row(reader, children, ScalarTerm::integer_equal)?,
        13 => binary_row(reader, children, ScalarTerm::integer_less_than)?,
        14 => binary_row(reader, children, ScalarTerm::integer_less_or_equal)?,
        15 => binary_row(reader, children, ScalarTerm::integer_bitwise_and)?,
        16 => binary_row(reader, children, ScalarTerm::integer_bitwise_or)?,
        17 => binary_row(reader, children, ScalarTerm::integer_bitwise_xor)?,
        18 => typed_pair_binary_row(reader, children, ScalarTerm::wrapping_integer_shift_left)?,
        19 => typed_pair_binary_row(reader, children, ScalarTerm::wrapping_integer_shift_right)?,
        20 => {
            let scalar_type = decode_integer_type(reader)?;
            let operand = one(reader, children)?;
            ScalarTerm::integer_bitwise_not(scalar_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        21 => typed_pair_unary_row(reader, children, ScalarTerm::integer_widen)?,
        22 => typed_pair_unary_row(reader, children, ScalarTerm::integer_exact_cast)?,
        23 => typed_pair_binary_row(reader, children, ScalarTerm::exact_integer_shift_right)?,
        24 => typed_pair_binary_row(reader, children, ScalarTerm::exact_integer_shift_left)?,
        25 => binary_row(reader, children, ScalarTerm::exact_integer_add)?,
        26 => binary_row(reader, children, ScalarTerm::exact_integer_subtract)?,
        27 => binary_row(reader, children, ScalarTerm::exact_integer_multiply)?,
        28 => binary_row(reader, children, ScalarTerm::exact_integer_divide)?,
        29 => binary_row(reader, children, ScalarTerm::exact_integer_remainder)?,
        30 => binary_row(reader, children, ScalarTerm::wrapping_integer_divide)?,
        31 => binary_row(reader, children, ScalarTerm::wrapping_integer_remainder)?,
        32 => binary_row(reader, children, ScalarTerm::saturating_integer_divide)?,
        33 => binary_row(reader, children, ScalarTerm::saturating_integer_remainder)?,
        tag @ (34 | 42) => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            ScalarTerm::BooleanField {
                version: if tag == 34 {
                    semantic_vocabulary::ContentPlaceVersion::Current
                } else {
                    semantic_vocabulary::ContentPlaceVersion::Entry
                },
                root,
                path,
            }
        }
        tag @ (35 | 43) => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            ScalarTerm::IntegerField {
                version: if tag == 35 {
                    semantic_vocabulary::ContentPlaceVersion::Current
                } else {
                    semantic_vocabulary::ContentPlaceVersion::Entry
                },
                root,
                path,
                scalar_type: decode_integer_type(reader)?,
            }
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
        tag @ (38 | 44) => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            let term = ScalarTerm::IeeeFloatField {
                version: if tag == 38 {
                    semantic_vocabulary::ContentPlaceVersion::Current
                } else {
                    semantic_vocabulary::ContentPlaceVersion::Entry
                },
                root,
                path,
                format: decode_ieee_float_format(reader)?,
            };
            term.validate_after_operands()
                .map_err(CodecError::MalformedProposition)?;
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
            // `None` marks a dynamic index whose operand handle follows the path.
            let segments = decode_counted(reader, |reader| {
                Ok(match reader.u8()? {
                    1 => Some(ScalarReadProjection::Static(
                        CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
                    )),
                    2 => Some(ScalarReadProjection::Static(
                        CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
                    )),
                    3 => Some(ScalarReadProjection::Static(
                        CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
                    )),
                    4 => None,
                    5 => Some(ScalarReadProjection::Referent),
                    6 => Some(ScalarReadProjection::Extent),
                    7 => Some(ScalarReadProjection::Alignment),
                    tag => return Err(CodecError::InvalidTag("ScalarReadProjection", tag)),
                })
            })?;
            let index_count = segments.iter().filter(|segment| segment.is_none()).count();
            let mut indices = children.take(reader, index_count)?.into_iter();
            let mut path = Vec::with_capacity(segments.len());
            for segment in segments {
                path.push(match segment {
                    Some(segment) => segment,
                    None => ScalarReadProjection::Index(Box::new(
                        indices
                            .next()
                            .ok_or(CodecError::NonCanonicalTermTable("scalar term"))?,
                    )),
                });
            }
            let term = ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            };
            term.validate_after_operands()
                .map_err(CodecError::MalformedProposition)?;
            term
        }
        tag => return Err(CodecError::InvalidTag("ScalarTerm", tag)),
    })
}

#[cfg(test)]
mod scalar_read_projection_tests {
    use super::*;
    use semantic_vocabulary::{PlaceId, ScalarType};

    #[test]
    fn entry_field_observations_remain_distinct_on_the_wire() {
        use semantic_vocabulary::{ContentPlaceVersion, IeeeFloatFormat, IntegerSign, IntegerType};
        let root = PlaceId::new(1).unwrap();
        let path = vec![CanonicalStructuralPathSegment::Field(
            semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
        )];
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        for (current, current_tag, entry_tag) in [
            (ScalarTerm::boolean_field_path(root, path.clone()), 34, 42),
            (
                ScalarTerm::integer_field_path(root, path.clone(), integer),
                35,
                43,
            ),
            (
                ScalarTerm::ieee_float_field_path(root, path.clone(), IeeeFloatFormat::Binary32),
                38,
                44,
            ),
        ] {
            let mut entry = current.clone();
            match &mut entry {
                ScalarTerm::BooleanField { version, .. }
                | ScalarTerm::IntegerField { version, .. }
                | ScalarTerm::IeeeFloatField { version, .. } => {
                    *version = ContentPlaceVersion::Entry
                }
                _ => unreachable!("the cases are scalar field observations"),
            }
            let mut encoded = Vec::new();
            for (term, tag) in [(&current, current_tag), (&entry, entry_tag)] {
                let mut row = Writer::default();
                write_scalar_row(&mut row, term).unwrap();
                assert_eq!(row.finish()[0], tag);
                let mut writer = Writer::default();
                encode_scalar_term(&mut writer, term).unwrap();
                let bytes = writer.finish();
                let mut reader = Reader::new(&bytes);
                assert_eq!(decode_scalar_term(&mut reader).unwrap(), *term);
                assert_eq!(reader.remaining(), 0);
                encoded.push(bytes);
            }
            assert_ne!(
                encoded[0], encoded[1],
                "an entry read cannot become a current read"
            );
        }
    }

    #[test]
    fn explicit_reference_read_has_an_exact_canonical_wire_roundtrip() {
        let term = ScalarTerm::structural_read(
            PlaceId::new(1).unwrap(),
            vec![ScalarReadProjection::Referent],
            ScalarType::Boolean,
        )
        .unwrap();
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, &term).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let decoded = decode_scalar_term(&mut reader).unwrap();
        assert_eq!(decoded, term);
        assert_eq!(reader.remaining(), 0);
        let mut repeated = Writer::default();
        encode_scalar_term(&mut repeated, &decoded).unwrap();
        assert_eq!(repeated.finish(), bytes);
    }

    #[test]
    fn alignment_projection_roundtrips_only_as_a_terminal_u64_observation() {
        use semantic_vocabulary::{IntegerSign, IntegerType};
        let root = PlaceId::new(3).unwrap();
        let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let path = vec![
            ScalarReadProjection::Referent,
            ScalarReadProjection::Alignment,
        ];
        let term = ScalarTerm::structural_read(root, path.clone(), count).unwrap();
        let mut writer = Writer::default();
        encode_scalar_term(&mut writer, &term).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_scalar_term(&mut reader).unwrap(), term);
        assert_eq!(reader.remaining(), 0);
        assert!(ScalarTerm::structural_read(root, path.clone(), ScalarType::Boolean).is_err());
        let mut continued = path;
        continued.push(ScalarReadProjection::Extent);
        assert!(ScalarTerm::structural_read(root, continued, count).is_err());
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
            encode_scalar_term(&mut Writer::default(), &term),
            Err(CodecError::MalformedProposition(
                PropositionError::NonCanonicalScalarReadProjection
            ))
        ));
    }
}
