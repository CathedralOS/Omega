//! Propositions, content algebras and content terms on the wire.

use crate::sections::proof_bundle::ProofCodecError;
use crate::sections::proof_bundle::scalar_term_codec::{
    decode_ieee_float_comparison_kind, decode_ieee_float_field, decode_ieee_float_format,
    decode_integer_math_term, decode_scalar_term, encode_ieee_float_comparison_kind,
    encode_ieee_float_field, encode_ieee_float_format, encode_integer_math_term,
    encode_scalar_term,
};
use crate::sections::proof_bundle::wire::{Reader, Writer};
use semantic_vocabulary::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    Proposition, PropositionId, StructuralCaseSubject,
};

pub(crate) const MAX_PROPOSITION_DEPTH: usize = 256;

pub(crate) const MAX_CONTENT_TERM_DEPTH: usize = 256;

pub(crate) const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;

fn encode_byte_sequence_field(
    writer: &mut Writer,
    field: &ByteSequenceStructuralField,
) -> Result<(), ProofCodecError> {
    encode_canonical_structural_field(
        writer,
        field.root(),
        field.path(),
        "byte-sequence field path",
    )
}

fn decode_byte_sequence_field(
    reader: &mut Reader<'_>,
) -> Result<ByteSequenceStructuralField, ProofCodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    ByteSequenceStructuralField::new(root, path).map_err(ProofCodecError::MalformedProposition)
}

pub(crate) fn encode_canonical_structural_field(
    writer: &mut Writer,
    root: semantic_vocabulary::PlaceId,
    path: &[CanonicalStructuralPathSegment],
    length_label: &'static str,
) -> Result<(), ProofCodecError> {
    writer.id(root);
    writer.len(length_label, path.len())?;
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
    Ok(())
}

pub(crate) fn decode_canonical_structural_field(
    reader: &mut Reader<'_>,
) -> Result<
    (
        semantic_vocabulary::PlaceId,
        Vec<CanonicalStructuralPathSegment>,
    ),
    ProofCodecError,
> {
    let root = reader.id("PlaceId")?;
    let count = reader.count()?;
    let mut path = Vec::with_capacity(count as usize);
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
    Ok((root, path))
}

pub(crate) fn encode_proposition(
    writer: &mut Writer,
    proposition: &Proposition,
    depth: usize,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(ProofCodecError::PropositionNestingTooDeep);
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
            encode_scalar_term(writer, left, 0, format_marker)?;
            encode_scalar_term(writer, right, 0, format_marker)?;
        }
        Proposition::LessThan(left, right) => {
            writer.u8(5);
            encode_scalar_term(writer, left, 0, format_marker)?;
            encode_scalar_term(writer, right, 0, format_marker)?;
        }
        Proposition::LessOrEqual(left, right) => {
            writer.u8(6);
            encode_scalar_term(writer, left, 0, format_marker)?;
            encode_scalar_term(writer, right, 0, format_marker)?;
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
            writer.len("proof proposition conjuncts", conjuncts.len())?;
            for conjunct in conjuncts {
                encode_proposition(writer, conjunct, depth + 1, format_marker)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            writer.u8(8);
            encode_proposition(writer, premise, depth + 1, format_marker)?;
            encode_proposition(writer, conclusion, depth + 1, format_marker)?;
        }
        Proposition::ContentConservation(conservation) => {
            writer.u8(9);
            encode_content_algebra(writer, conservation.algebra())?;
            encode_content_term(writer, conservation.left(), 0, format_marker)?;
            encode_content_term(writer, conservation.right(), 0, format_marker)?;
        }
        Proposition::Disjunction(disjuncts) => {
            writer.u8(10);
            writer.len("proof proposition disjuncts", disjuncts.len())?;
            for disjunct in disjuncts {
                encode_proposition(writer, disjunct, depth + 1, format_marker)?;
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
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            writer.u8(17);
            encode_ieee_float_comparison_kind(writer, *kind);
            encode_ieee_float_format(writer, *format);
            encode_scalar_term(writer, left, 0, format_marker)?;
            encode_scalar_term(writer, right, 0, format_marker)?;
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

fn encode_content_algebra(
    writer: &mut Writer,
    algebra: &ContentAlgebra,
) -> Result<(), ProofCodecError> {
    writer.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    writer.string("content algebra parameter", &algebra.parameter)
}

#[allow(
    clippy::only_used_in_recursion,
    reason = "the format marker is deliberately threaded through recursive proof encoding"
)]
fn encode_content_term(
    writer: &mut Writer,
    term: &ContentTerm,
    depth: usize,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(ProofCodecError::ContentTermNestingTooDeep);
    }
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            writer.u8(1);
            writer.id(projection.domain);
            writer.u64(projection.projection_report_fingerprint);
            writer.u8(match subject.version {
                ContentPlaceVersion::Entry => 1,
                ContentPlaceVersion::Current => 2,
            });
            writer.id(subject.root);
            writer.len("content place segments", subject.segments.len())?;
            for segment in &subject.segments {
                match segment {
                    ContentPlaceSegment::Case(name) => {
                        writer.u8(3);
                        writer.string("content case", name)?;
                    }
                    ContentPlaceSegment::Field(name) => {
                        writer.u8(1);
                        writer.string("content field", name)?;
                    }
                    ContentPlaceSegment::FixedIndex(index) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                }
            }
        }
        ContentTerm::Separate(terms) => {
            writer.u8(2);
            writer.len("separated content terms", terms.len())?;
            for term in terms {
                encode_content_term(writer, term, depth + 1, format_marker)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_proposition(
    reader: &mut Reader<'_>,
    depth: usize,
    format_marker: u16,
) -> Result<Proposition, ProofCodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(ProofCodecError::PropositionNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => Proposition::Truth,
        2 => Proposition::Falsehood,
        3 => Proposition::Atom(reader.id::<PropositionId>("PropositionId")?),
        4 => Proposition::Equal(
            decode_scalar_term(reader, 0, format_marker)?,
            decode_scalar_term(reader, 0, format_marker)?,
        ),
        5 => Proposition::LessThan(
            decode_scalar_term(reader, 0, format_marker)?,
            decode_scalar_term(reader, 0, format_marker)?,
        ),
        6 => Proposition::LessOrEqual(
            decode_scalar_term(reader, 0, format_marker)?,
            decode_scalar_term(reader, 0, format_marker)?,
        ),
        7 => {
            let count = reader.count()?;
            let mut conjuncts = Vec::new();
            for _ in 0..count {
                conjuncts.push(decode_proposition(reader, depth + 1, format_marker)?);
            }
            Proposition::Conjunction(conjuncts)
        }
        8 => Proposition::Implication {
            premise: Box::new(decode_proposition(reader, depth + 1, format_marker)?),
            conclusion: Box::new(decode_proposition(reader, depth + 1, format_marker)?),
        },
        9 => {
            let algebra = decode_content_algebra(reader)?;
            let left = decode_content_term(reader, 0, format_marker)?;
            let right = decode_content_term(reader, 0, format_marker)?;
            Proposition::ContentConservation(ContentConservation::new(algebra, left, right))
        }
        10 => {
            let count = reader.count()?;
            let mut disjuncts = Vec::new();
            for _ in 0..count {
                disjuncts.push(decode_proposition(reader, depth + 1, format_marker)?);
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
        17 => Proposition::ScalarIeeeFloatComparison {
            kind: decode_ieee_float_comparison_kind(reader)?,
            format: decode_ieee_float_format(reader)?,
            left: decode_scalar_term(reader, 0, format_marker)?,
            right: decode_scalar_term(reader, 0, format_marker)?,
        },
        tag => return Err(ProofCodecError::InvalidTag("Proposition", tag)),
    })
}

fn decode_content_algebra(reader: &mut Reader<'_>) -> Result<ContentAlgebra, ProofCodecError> {
    let kind = match reader.u8()? {
        1 => ContentAlgebraKind::IntervalSet,
        2 => ContentAlgebraKind::CountedQuantity,
        tag => return Err(ProofCodecError::InvalidTag("ContentAlgebraKind", tag)),
    };
    Ok(ContentAlgebra {
        kind,
        parameter: reader.string("content algebra parameter")?,
    })
}

#[allow(
    clippy::only_used_in_recursion,
    reason = "the format marker is deliberately threaded through recursive proof decoding"
)]
fn decode_content_term(
    reader: &mut Reader<'_>,
    depth: usize,
    format_marker: u16,
) -> Result<ContentTerm, ProofCodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(ProofCodecError::ContentTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => {
            let projection = ContentProjectionIdentity {
                domain: reader.id::<ContentDomainId>("ContentDomainId")?,
                projection_report_fingerprint: reader.u64()?,
            };
            let version = match reader.u8()? {
                1 => ContentPlaceVersion::Entry,
                2 => ContentPlaceVersion::Current,
                tag => return Err(ProofCodecError::InvalidTag("ContentPlaceVersion", tag)),
            };
            let root = reader.id("PlaceId")?;
            let count = reader.count()?;
            let mut segments = Vec::new();
            for _ in 0..count {
                segments.push(match reader.u8()? {
                    1 => ContentPlaceSegment::Field(reader.string("content field")?),
                    2 => ContentPlaceSegment::FixedIndex(reader.u64()?),
                    3 => ContentPlaceSegment::Case(reader.string("content case")?),
                    tag => return Err(ProofCodecError::InvalidTag("ContentPlaceSegment", tag)),
                });
            }
            ContentTerm::Projection {
                projection,
                subject: ContentStructuralPlace {
                    version,
                    root,
                    segments,
                },
            }
        }
        2 => {
            let count = reader.count()?;
            let mut terms = Vec::new();
            for _ in 0..count {
                terms.push(decode_content_term(reader, depth + 1, format_marker)?);
            }
            ContentTerm::separate(terms).map_err(ProofCodecError::MalformedProposition)?
        }
        tag => return Err(ProofCodecError::InvalidTag("ContentTerm", tag)),
    })
}
