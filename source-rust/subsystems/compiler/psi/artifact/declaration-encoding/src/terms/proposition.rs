use semantic_vocabulary::{ContentConservation, Proposition, PropositionId, StructuralCaseSubject};

use crate::CodecError;
use crate::structural_fields::{
    decode_canonical_structural_field, decode_ieee_float_comparison_kind, decode_ieee_float_field,
    decode_ieee_float_format, encode_canonical_structural_field, encode_ieee_float_comparison_kind,
    encode_ieee_float_field, encode_ieee_float_format,
};
use crate::terms::content::{
    decode_content_algebra, decode_content_term, encode_content_algebra, encode_content_term,
};
use crate::terms::integer::{decode_integer_math_term, encode_integer_math_term};
use crate::terms::scalar::{decode_scalar_term, encode_scalar_term};
use crate::terms::scalar_types::{decode_integer_type, encode_integer_type};
use crate::terms::table::{TableChildren, TermEncoding, decode_term_table, encode_term_table};
use crate::wire::{Reader, Writer, bounded_count};

use crate::fields::{Field, List, u32_newtypes, wire_enum, wire_record};
use crate::terms::float_meaning as float_meaning_wire;
use semantic_vocabulary::{
    FloatIntegerConversion, FloatMeaningClauseOperand, FloatMeaningClauseValue, FloatMeaningSlot,
    IeeeFloatFormat, ScalarTerm,
};

struct MeaningSource;
impl Field<ScalarTerm> for MeaningSource {
    fn write(writer: &mut Writer, source: &ScalarTerm) -> Result<(), CodecError> {
        encode_scalar_term(writer, source)
    }
    fn read(reader: &mut Reader<'_>) -> Result<ScalarTerm, CodecError> {
        decode_scalar_term(reader)
    }
}
u32_newtypes! { MeaningSlot: FloatMeaningSlot; }
wire_record! {
    IntegerConversion: FloatIntegerConversion {
        source: MeaningSource, result: MeaningSource,
        projection: float_meaning_wire::ProjectionContract,
        operation: float_meaning_wire::ApplicationContract,
    }
}
wire_enum! {
    MeaningValue: FloatMeaningClauseValue, "FloatMeaningClauseValue" {
        0 => Projection { source: MeaningSource, contract: float_meaning_wire::ProjectionContract },
        1 => Application { contract: float_meaning_wire::ApplicationContract, format: float_meaning_wire::Format, operands: List<MeaningOperand> },
    }
    MeaningOperand: FloatMeaningClauseOperand, "FloatMeaningClauseOperand" {
        0 => Format(format: float_meaning_wire::Format),
        1 => Meaning(slot: MeaningSlot),
        2 => Integer(source: MeaningSource),
    }
}

/// Encodes one proposition as a postorder table (`crate::terms::table`);
/// its scalar, integer math and content operands are their own encodings
/// inside the row.
pub fn encode_proposition(
    writer: &mut Writer,
    proposition: &Proposition,
) -> Result<(), CodecError> {
    encode_term_table::<PropositionTable>(writer, proposition)
}

/// Canonical ordering retains symbolic widths during checking.
pub fn canonical_proposition_order_key(proposition: &Proposition) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::checking_identity();
    encode_proposition(&mut writer, proposition)?;
    Ok(writer.finish())
}

pub fn decode_proposition(reader: &mut Reader<'_>) -> Result<Proposition, CodecError> {
    decode_term_table::<PropositionTable>(reader)
}

struct PropositionTable;

impl TermEncoding for PropositionTable {
    type Term = Proposition;
    const LABEL: &'static str = "proposition";

    fn children<'term>(term: &'term Self::Term, children: &mut Vec<&'term Self::Term>) {
        match term {
            Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
                children.extend(members)
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => children.extend([premise.as_ref(), conclusion.as_ref()]),
            _ => {}
        }
    }

    fn write_row(term: &Self::Term, writer: &mut Writer) -> Result<(), CodecError> {
        write_proposition_row(writer, term)
    }

    fn read_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, Self::Term>,
    ) -> Result<Self::Term, CodecError> {
        read_proposition_row(reader, children)
    }
}

/// One layout serves both writers: a module publishes an Identity of inputs
/// and scalar terms, and the checker's ordering key, which never leaves the
/// compiler, may also name type parameters, formals and applications.
fn write_identity(
    writer: &mut Writer,
    clause: &semantic_vocabulary::ClauseIdentity,
) -> Result<(), CodecError> {
    use semantic_vocabulary::{ClauseCarrier, ClauseOperand};
    clause
        .validate()
        .map_err(CodecError::MalformedProposition)?;
    writer.u8(23);
    writer.len("clause operands", clause.values.len())?;
    for value in &clause.values {
        match value.carrier {
            ClauseCarrier::Parameter(binder) => {
                writer.u8(0);
                writer.u32(binder);
            }
            ClauseCarrier::Scalar(ty) => {
                writer.u8(1);
                crate::terms::scalar_types::encode_scalar_type(writer, ty)?;
            }
            ClauseCarrier::Structural(ty) => {
                writer.u8(2);
                writer.id(ty);
            }
            ClauseCarrier::FloatMeaning(format) => {
                writer.u8(3);
                <float_meaning_wire::Format as Field<IeeeFloatFormat>>::write(writer, &format)?;
            }
        }
        match &value.operand {
            ClauseOperand::Input(subject) => {
                writer.u8(0);
                encode_canonical_structural_field(
                    writer,
                    subject.root(),
                    subject.path(),
                    "identity subject",
                )?;
            }
            ClauseOperand::Scalar(term) => {
                writer.u8(1);
                encode_scalar_term(writer, term)?;
            }
            ClauseOperand::Formal(position) => {
                writer.u8(2);
                writer.u32(*position);
            }
            ClauseOperand::Application {
                declaration,
                arguments,
            } => {
                writer.u8(3);
                writer.u8(u8::from(declaration.supplier.is_some()));
                if let Some((slot, generation)) = declaration.supplier {
                    writer.u32(slot);
                    writer.u32(generation);
                }
                writer.u32(declaration.member.0);
                writer.u32(declaration.member.1);
                writer.len("clause arguments", arguments.len())?;
                for &argument in arguments {
                    writer.u32(argument);
                }
            }
            ClauseOperand::FloatMeaning(row) => {
                writer.u8(4);
                MeaningValue::write(writer, row)?;
            }
        }
    }
    writer.u32(clause.left);
    writer.u32(clause.right);
    Ok(())
}

/// A published Identity: each value an input at its data type, a scalar term
/// at its scalar type or a float meaning. The forms only the checker's ordering key holds
/// are refused by tag.
fn read_identity(
    reader: &mut Reader<'_>,
) -> Result<semantic_vocabulary::ClauseIdentity, CodecError> {
    use semantic_vocabulary::{ClauseCarrier, ClauseIdentity, ClauseOperand, ClauseValue};
    let count = bounded_count(reader, 2)?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let carrier = match reader.u8()? {
            1 => ClauseCarrier::Scalar(crate::terms::scalar_types::decode_scalar_type(reader)?),
            2 => ClauseCarrier::Structural(reader.id("StructuralTypeId")?),
            3 => ClauseCarrier::FloatMeaning(<float_meaning_wire::Format as Field<
                IeeeFloatFormat,
            >>::read(reader)?),
            tag => return Err(CodecError::InvalidTag("Identity carrier", tag)),
        };
        let operand = match reader.u8()? {
            0 => {
                let (root, path) = decode_canonical_structural_field(reader)?;
                ClauseOperand::Input(StructuralCaseSubject::new(root, path))
            }
            1 => ClauseOperand::Scalar(decode_scalar_term(reader)?),
            4 => ClauseOperand::FloatMeaning(MeaningValue::read(reader)?),
            tag => return Err(CodecError::InvalidTag("Identity operand", tag)),
        };
        values.push(ClauseValue { carrier, operand });
    }
    let clause = ClauseIdentity {
        values,
        left: reader.u32()?,
        right: reader.u32()?,
    };
    clause
        .validate()
        .map_err(CodecError::MalformedProposition)?;
    if !clause.is_portable() {
        return Err(CodecError::MalformedStructuralFoundation(
            "a published Identity pairs an operand with another carrier",
        ));
    }
    Ok(clause)
}

fn write_proposition_row(writer: &mut Writer, proposition: &Proposition) -> Result<(), CodecError> {
    match proposition {
        Proposition::Truth => writer.u8(1),
        Proposition::Falsehood => writer.u8(2),
        Proposition::Atom(id) => {
            writer.u8(3);
            writer.id(*id);
        }
        Proposition::Equal(left, right) => {
            writer.u8(4);
            encode_scalar_term(writer, left)?;
            encode_scalar_term(writer, right)?;
        }
        Proposition::LessThan(left, right) => {
            writer.u8(5);
            encode_scalar_term(writer, left)?;
            encode_scalar_term(writer, right)?;
        }
        Proposition::LessOrEqual(left, right) => {
            writer.u8(6);
            encode_scalar_term(writer, left)?;
            encode_scalar_term(writer, right)?;
        }
        semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
            writer.u8(match proposition {
                Proposition::IntegerMathEqual(_, _) => 14,
                Proposition::IntegerMathLessThan(_, _) => 15,
                Proposition::IntegerMathLessOrEqual(_, _) => 16,
                _ => unreachable!(),
            });
            encode_integer_math_term(writer, left)?;
            encode_integer_math_term(writer, right)?;
        }
        Proposition::Conjunction(conjuncts) => {
            writer.u8(7);
            writer.len("conjuncts", conjuncts.len())?;
        }
        Proposition::Implication { .. } => writer.u8(8),
        Proposition::ContentConservation(conservation) => {
            writer.u8(9);
            encode_content_algebra(writer, conservation.algebra())?;
            encode_content_term(writer, conservation.left())?;
            encode_content_term(writer, conservation.right())?;
        }
        Proposition::Disjunction(disjuncts) => {
            writer.u8(10);
            writer.len("disjuncts", disjuncts.len())?;
        }
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            writer.u8(11);
            encode_ieee_float_comparison_kind(writer, *kind);
            encode_ieee_float_format(writer, *format);
            encode_ieee_float_field(writer, left)?;
            encode_ieee_float_field(writer, right)?;
        }
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            writer.u8(17);
            encode_ieee_float_comparison_kind(writer, *kind);
            encode_ieee_float_format(writer, *format);
            encode_scalar_term(writer, left)?;
            encode_scalar_term(writer, right)?;
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            writer.u8(13);
            encode_canonical_structural_field(
                writer,
                subject.root(),
                subject.path(),
                "structural case subject path",
            )?;
            writer.id(*case);
        }
        Proposition::DeclarationIdentity(clause) => {
            if !writer.is_checking_identity() && !clause.is_portable() {
                return Err(CodecError::MalformedStructuralFoundation(
                    "a declaration Identity needs its checked mathematical signature for publication",
                ));
            }
            write_identity(writer, clause)?;
        }
        Proposition::FloatIntegerConversion(clause) => {
            writer.u8(20);
            IntegerConversion::write(writer, clause)?;
        }
        Proposition::IeeeFloatIntegerExact { source, target } => {
            writer.u8(21);
            encode_scalar_term(writer, source)?;
            encode_integer_type(writer, *target)?;
        }
        Proposition::IntegerIeeeFloatExact { source, target } => {
            writer.u8(22);
            encode_scalar_term(writer, source)?;
            encode_ieee_float_format(writer, *target);
        }
    }
    Ok(())
}

fn read_proposition_row(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, Proposition>,
) -> Result<Proposition, CodecError> {
    Ok(match reader.u8()? {
        1 => Proposition::Truth,
        2 => Proposition::Falsehood,
        3 => Proposition::Atom(reader.id::<PropositionId>("PropositionId")?),
        4 => Proposition::Equal(decode_scalar_term(reader)?, decode_scalar_term(reader)?),
        5 => Proposition::LessThan(decode_scalar_term(reader)?, decode_scalar_term(reader)?),
        6 => Proposition::LessOrEqual(decode_scalar_term(reader)?, decode_scalar_term(reader)?),
        7 => {
            let count = bounded_count(reader, 4)?;
            Proposition::Conjunction(children.take(reader, count)?)
        }
        8 => {
            let mut operands = children.take(reader, 2)?.into_iter();
            match (operands.next(), operands.next()) {
                (Some(premise), Some(conclusion)) => Proposition::Implication {
                    premise: Box::new(premise),
                    conclusion: Box::new(conclusion),
                },
                _ => return Err(CodecError::NonCanonicalTermTable("proposition")),
            }
        }
        9 => {
            let algebra = decode_content_algebra(reader)?;
            let left = decode_content_term(reader)?;
            let right = decode_content_term(reader)?;
            Proposition::ContentConservation(ContentConservation::new(algebra, left, right))
        }
        10 => {
            let count = bounded_count(reader, 4)?;
            Proposition::Disjunction(children.take(reader, count)?)
        }
        11 => Proposition::IeeeFloatComparison {
            kind: decode_ieee_float_comparison_kind(reader)?,
            format: decode_ieee_float_format(reader)?,
            left: decode_ieee_float_field(reader)?,
            right: decode_ieee_float_field(reader)?,
        },
        13 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(root, path),
                case: reader.id("StructuralCaseId")?,
            }
        }
        14 => Proposition::IntegerMathEqual(
            decode_integer_math_term(reader)?,
            decode_integer_math_term(reader)?,
        ),
        15 => Proposition::IntegerMathLessThan(
            decode_integer_math_term(reader)?,
            decode_integer_math_term(reader)?,
        ),
        16 => Proposition::IntegerMathLessOrEqual(
            decode_integer_math_term(reader)?,
            decode_integer_math_term(reader)?,
        ),
        17 => Proposition::ScalarIeeeFloatComparison {
            kind: decode_ieee_float_comparison_kind(reader)?,
            format: decode_ieee_float_format(reader)?,
            left: decode_scalar_term(reader)?,
            right: decode_scalar_term(reader)?,
        },
        20 => Proposition::FloatIntegerConversion(IntegerConversion::read(reader)?),
        21 => Proposition::IeeeFloatIntegerExact {
            source: decode_scalar_term(reader)?,
            target: decode_integer_type(reader)?,
        },
        22 => Proposition::IntegerIeeeFloatExact {
            source: decode_scalar_term(reader)?,
            target: decode_ieee_float_format(reader)?,
        },
        23 => Proposition::DeclarationIdentity(read_identity(reader)?),
        tag => return Err(CodecError::InvalidTag("Proposition", tag)),
    })
}
#[cfg(test)]
mod meaning_equation_tests {
    use super::*;
    use numerics::float_semantics::{
        FloatSemanticOperation,
        FloatSemanticValueKind::{Format, Meaning},
    };
    use semantic_vocabulary::{ClauseIdentity, IeeeFloatFormat, ScalarType, ValueId};

    #[test]
    fn integer_conversion_predicates_round_trip_the_complete_contract_and_carriers() {
        use numerics::float_semantics::FloatSemanticValueKind;
        use numerics::integer_carrier::FixedInteger;
        use semantic_vocabulary::{FloatIntegerConversion, IntegerSign, IntegerType};
        let target = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let source = ScalarTerm::value(
            ValueId::new(1).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        );
        let result = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Integer(target));
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "to_i32",
            &[Meaning],
            FloatSemanticValueKind::Integer(FixedInteger::signed(32)),
        )
        .unwrap();
        let graph =
            FloatIntegerConversion::new(source.clone(), result, operation.contract_identity())
                .unwrap();
        for original in [
            Proposition::IntegerIeeeFloatExact {
                source: ScalarTerm::integer(target, semantic_vocabulary::IntegerValue::Signed(42))
                    .unwrap(),
                target: IeeeFloatFormat::Binary32,
            },
            Proposition::FloatIntegerConversion(graph.clone()),
            Proposition::IeeeFloatIntegerExact { source, target },
        ] {
            let mut writer = Writer::default();
            encode_proposition(&mut writer, &original).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            let decoded = decode_proposition(&mut reader).unwrap();
            decoded.validate().unwrap();
            assert_eq!(reader.remaining(), 0);
            assert_eq!(decoded, original);
            if matches!(original, Proposition::FloatIntegerConversion(_)) {
                for commitment in [graph.operation.commitment, graph.projection.commitment] {
                    let mut changed = bytes.clone();
                    let position = changed
                        .windows(commitment.len())
                        .position(|window| window == commitment)
                        .unwrap();
                    changed[position] ^= 1;
                    assert!(
                        decode_proposition(&mut Reader::new(&changed))
                            .unwrap()
                            .validate()
                            .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn integer_float_meaning_operands_round_trip_and_reject_carrier_substitution() {
        use numerics::float_semantics::FloatSemanticValueKind::Integer;
        use numerics::integer_carrier::FixedInteger;
        let carrier = FixedInteger::signed(32);
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[Format, Integer(carrier)],
            Meaning,
        )
        .unwrap();
        let original = Proposition::DeclarationIdentity(
            ClauseIdentity::float_result_equation(
                ScalarTerm::value(
                    ValueId::new(2).unwrap(),
                    ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
                ),
                operation.contract_identity(),
                IeeeFloatFormat::Binary64,
                [ScalarTerm::value(
                    ValueId::new(1).unwrap(),
                    ScalarType::Integer(
                        semantic_vocabulary::IntegerType::from_fixed(carrier).unwrap(),
                    ),
                )],
            )
            .unwrap(),
        );
        let mut writer = Writer::default();
        encode_proposition(&mut writer, &original).unwrap();
        let encoded = writer.finish();
        let decoded = decode_proposition(&mut Reader::new(&encoded)).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, original);
        let Proposition::DeclarationIdentity(mut clause) = decoded else {
            unreachable!()
        };
        let FloatMeaningClauseValue::Application { operands, .. } =
            clause.float_meaning_rows_mut().next_back().unwrap()
        else {
            unreachable!()
        };
        operands[1] = FloatMeaningClauseOperand::Integer(ScalarTerm::value(
            ValueId::new(1).unwrap(),
            ScalarType::Integer(
                semantic_vocabulary::IntegerType::from_fixed(FixedInteger::unsigned(32)).unwrap(),
            ),
        ));
        // An Identity is checked before it is written, so a changed integer
        // carrier never reaches the wire.
        let mut writer = Writer::default();
        assert!(matches!(
            encode_proposition(&mut writer, &Proposition::DeclarationIdentity(clause)),
            Err(CodecError::MalformedProposition(_))
        ));
    }

    #[test]
    fn float_meaning_equations_round_trip_the_full_catalog_and_scalar_sources() {
        let format = IeeeFloatFormat::Binary64;
        let scalar = ScalarType::IeeeFloat(format);
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "divide",
            &[Format, Meaning, Meaning],
            Meaning,
        )
        .unwrap();
        let original = Proposition::DeclarationIdentity(
            ClauseIdentity::float_result_equation(
                value(3),
                operation.contract_identity(),
                format,
                [value(1), value(2)],
            )
            .unwrap(),
        );
        let mut writer = Writer::default();
        encode_proposition(&mut writer, &original).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let decoded = decode_proposition(&mut reader).unwrap();
        decoded.validate().unwrap();
        assert_eq!(reader.remaining(), 0);
        assert_eq!(decoded, original);
        let mut changed = bytes;
        let commitment = operation.contract_identity().commitment;
        let position = changed
            .windows(commitment.len())
            .position(|window| window == commitment)
            .unwrap();
        changed[position] ^= 1;
        assert!(
            matches!(
                decode_proposition(&mut Reader::new(&changed)),
                Err(CodecError::MalformedProposition(_))
            ),
            "catalog commitment mutation cannot become an opaque accepted equality"
        );
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, ClauseCarrier, ClauseIdentity, ClauseOperand, ClauseValue,
        IntegerSign, IntegerType, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
        ValueId,
    };

    fn identity(values: Vec<ClauseValue>) -> Proposition {
        Proposition::DeclarationIdentity(ClauseIdentity {
            values,
            left: 0,
            right: 1,
        })
    }

    fn round_trip(original: &Proposition) -> Result<Proposition, CodecError> {
        let mut writer = Writer::default();
        encode_proposition(&mut writer, original)?;
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let decoded = decode_proposition(&mut reader)?;
        assert_eq!(reader.remaining(), 0);
        Ok(decoded)
    }

    #[test]
    fn an_identity_of_inputs_and_scalar_terms_is_published_as_tag_23() {
        let record = ClauseCarrier::Structural(StructuralTypeId::new(3).unwrap());
        let input = |root, path| ClauseValue {
            carrier: record,
            operand: ClauseOperand::Input(StructuralCaseSubject::new(
                PlaceId::new(root).unwrap(),
                path,
            )),
        };
        let field = vec![CanonicalStructuralPathSegment::Field(
            StructuralFieldId::new(2).unwrap(),
        )];
        let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let scalar = |id| ClauseValue {
            carrier: ClauseCarrier::Scalar(integer),
            operand: ClauseOperand::Scalar(ScalarTerm::value(ValueId::new(id).unwrap(), integer)),
        };
        for original in [
            identity(vec![input(1, Vec::new()), input(2, field.clone())]),
            identity(vec![scalar(1), scalar(2)]),
        ] {
            assert_eq!(round_trip(&original).unwrap(), original);
        }
        // An applied declaration or a type parameter names what only the
        // source checker's signature defines, so a module refuses it.
        let applied = ClauseValue {
            carrier: record,
            operand: ClauseOperand::Application {
                declaration: semantic_vocabulary::ClauseDeclaration {
                    supplier: None,
                    member: (1, 1),
                },
                arguments: vec![0],
            },
        };
        let parameter = ClauseValue {
            carrier: ClauseCarrier::Parameter(0),
            operand: ClauseOperand::Formal(0),
        };
        for unpublished in [
            identity(vec![input(1, Vec::new()), applied]),
            identity(vec![parameter.clone(), parameter]),
        ] {
            assert_eq!(
                round_trip(&unpublished),
                Err(CodecError::MalformedStructuralFoundation(
                    "a declaration Identity needs its checked mathematical signature for publication",
                ))
            );
        }
    }

    #[test]
    fn the_retired_structural_byte_sequence_and_float_meaning_tags_are_rejected() {
        for retired in [12, 18, 19] {
            let mut writer = Writer::default();
            encode_proposition(&mut writer, &Proposition::Truth).unwrap();
            let mut bytes = writer.finish();
            *bytes.last_mut().unwrap() = retired;
            assert_eq!(
                decode_proposition(&mut Reader::new(&bytes)),
                Err(CodecError::InvalidTag("Proposition", retired))
            );
        }
    }
}
