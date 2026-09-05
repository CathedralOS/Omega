//! Canonical proof-proposition wire format.
//!
//! This module owns recursive proposition envelopes and their exact variant
//! tags. Scalar terms, content terms, and structural-field payloads remain
//! encoded by their respective sibling wire modules.

use semantic_vocabulary::{ContentConservation, Proposition, PropositionId, StructuralCaseSubject};

use super::content_wire::{
    decode_content_algebra, decode_content_term, encode_content_algebra, encode_content_term,
};
use super::integer_math_term_wire::{decode_integer_math_term, encode_integer_math_term};
use super::structural_field_wire::{
    decode_byte_sequence_field, decode_canonical_structural_field,
    decode_ieee_float_comparison_kind, decode_ieee_float_field, decode_ieee_float_format,
    encode_byte_sequence_field, encode_canonical_structural_field,
    encode_ieee_float_comparison_kind, encode_ieee_float_field, encode_ieee_float_format,
};
use super::wire::{Reader, Writer};
use super::{CodecError, MAX_PROPOSITION_DEPTH, decode_scalar_term, encode_scalar_term};

pub(super) fn encode_proposition(
    writer: &mut Writer,
    proposition: &Proposition,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    match proposition {
        Proposition::Truth => writer.u8(1),
        Proposition::Falsehood => writer.u8(2),
        Proposition::Atom(id) => {
            writer.u8(3);
            writer.id(*id);
        }
        Proposition::Equal(left, right) => {
            writer.u8(4);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::LessThan(left, right) => {
            writer.u8(5);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::LessOrEqual(left, right) => {
            writer.u8(6);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::IntegerMathEqual(left, right)
        | Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => {
            writer.u8(match proposition {
                Proposition::IntegerMathEqual(_, _) => 14,
                Proposition::IntegerMathLessThan(_, _) => 15,
                Proposition::IntegerMathLessOrEqual(_, _) => 16,
                _ => unreachable!(),
            });
            encode_integer_math_term(writer, left, 0)?;
            encode_integer_math_term(writer, right, 0)?;
        }
        Proposition::Conjunction(conjuncts) => {
            writer.u8(7);
            writer.len("conjuncts", conjuncts.len())?;
            for conjunct in conjuncts {
                encode_proposition(writer, conjunct, depth + 1)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            writer.u8(8);
            encode_proposition(writer, premise, depth + 1)?;
            encode_proposition(writer, conclusion, depth + 1)?;
        }
        Proposition::ContentConservation(conservation) => {
            writer.u8(9);
            encode_content_algebra(writer, conservation.algebra())?;
            encode_content_term(writer, conservation.left(), 0)?;
            encode_content_term(writer, conservation.right(), 0)?;
        }
        Proposition::Disjunction(disjuncts) => {
            writer.u8(10);
            writer.len("disjuncts", disjuncts.len())?;
            for disjunct in disjuncts {
                encode_proposition(writer, disjunct, depth + 1)?;
            }
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
        Proposition::ByteSequenceEqual { left, right } => {
            writer.u8(12);
            encode_byte_sequence_field(writer, left)?;
            encode_byte_sequence_field(writer, right)?;
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
    }
    Ok(())
}

pub(super) fn decode_proposition(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<Proposition, CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => Proposition::Truth,
        2 => Proposition::Falsehood,
        3 => Proposition::Atom(reader.id::<PropositionId>("PropositionId")?),
        4 => Proposition::Equal(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        5 => Proposition::LessThan(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        6 => Proposition::LessOrEqual(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        7 => {
            let count = reader.count()?;
            let mut conjuncts = Vec::new();
            for _ in 0..count {
                conjuncts.push(decode_proposition(reader, depth + 1)?);
            }
            Proposition::Conjunction(conjuncts)
        }
        8 => Proposition::Implication {
            premise: Box::new(decode_proposition(reader, depth + 1)?),
            conclusion: Box::new(decode_proposition(reader, depth + 1)?),
        },
        9 => {
            let algebra = decode_content_algebra(reader)?;
            let left = decode_content_term(reader, 0)?;
            let right = decode_content_term(reader, 0)?;
            Proposition::ContentConservation(ContentConservation::new(algebra, left, right))
        }
        10 => {
            let count = reader.count()?;
            let mut disjuncts = Vec::new();
            for _ in 0..count {
                disjuncts.push(decode_proposition(reader, depth + 1)?);
            }
            Proposition::Disjunction(disjuncts)
        }
        11 => Proposition::IeeeFloatComparison {
            kind: decode_ieee_float_comparison_kind(reader)?,
            format: decode_ieee_float_format(reader)?,
            left: decode_ieee_float_field(reader)?,
            right: decode_ieee_float_field(reader)?,
        },
        12 => Proposition::ByteSequenceEqual {
            left: decode_byte_sequence_field(reader)?,
            right: decode_byte_sequence_field(reader)?,
        },
        13 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(root, path),
                case: reader.id("StructuralCaseId")?,
            }
        }
        14 => Proposition::IntegerMathEqual(
            decode_integer_math_term(reader, 0)?,
            decode_integer_math_term(reader, 0)?,
        ),
        15 => Proposition::IntegerMathLessThan(
            decode_integer_math_term(reader, 0)?,
            decode_integer_math_term(reader, 0)?,
        ),
        16 => Proposition::IntegerMathLessOrEqual(
            decode_integer_math_term(reader, 0)?,
            decode_integer_math_term(reader, 0)?,
        ),
        tag => return Err(CodecError::InvalidTag("Proposition", tag)),
    })
}
