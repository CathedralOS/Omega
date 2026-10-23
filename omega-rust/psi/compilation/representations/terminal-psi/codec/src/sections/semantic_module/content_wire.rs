//! Canonical content-custody and content-term wire format.
//!
//! This module owns exact content claims, partition compositions,
//! conservation/reshuffle rows, algebras, recursive terms, and structural
//! places. It does not validate content authority or interpret projections.
//! Content theorem signatures cannot declare machine-local block roots.

use semantic_vocabulary::{
    ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, StructuralPlaceKind,
};
use terminal_psi::{
    ClaimContentProjection, ContentConservationGuarantee, ContentEntryClaim,
    ContentIdentityReshuffle, ContentPartitionComposition, ContentPlaceSubstitution,
    StructuralPlaceDeclaration,
};

use super::wire::{Reader, Writer};
use super::{CodecError, MAX_CONTENT_TERM_DEPTH};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_place_kind, encode_structural_place_kind,
};
use crate::sections::semantic_module::wire::decode_counted;

pub(crate) fn encode_content_entry_claim(
    writer: &mut Writer,
    binding: &ContentEntryClaim,
) -> Result<(), CodecError> {
    writer.id(binding.claim);
    encode_content_structural_place(writer, &binding.input)?;
    encode_claim_content_projections(writer, &binding.projections)
}

pub(crate) fn encode_content_partition_composition(
    writer: &mut Writer,
    composition: &ContentPartitionComposition,
) -> Result<(), CodecError> {
    writer.id(composition.producer_operation);
    writer.u64(composition.source_report_fingerprint);
    writer.len(
        "partition source structural places",
        composition.source_structural_places.len(),
    )?;
    for place in &composition.source_structural_places {
        if matches!(place.kind, StructuralPlaceKind::BlockParameter { .. }) {
            return Err(CodecError::InvalidTag("ContentStructuralPlaceKind", 8));
        }
        writer.id(place.id);
        encode_structural_place_kind(writer, place.kind);
    }
    encode_content_conservation(writer, &composition.source)?;
    writer.len("partition input claims", composition.input_claims.len())?;
    for claim in &composition.input_claims {
        writer.id(*claim);
    }
    writer.len(
        "partition place substitutions",
        composition.substitutions.len(),
    )?;
    for substitution in &composition.substitutions {
        encode_content_structural_place(writer, &substitution.source)?;
        encode_content_structural_place(writer, &substitution.target)?;
    }
    encode_content_conservation(writer, &composition.derived)
}

pub(crate) fn encode_content_conservation_guarantee(
    writer: &mut Writer,
    guarantee: &ContentConservationGuarantee,
) -> Result<(), CodecError> {
    writer.u64(guarantee.report_fingerprint);
    writer.len(
        "content guarantee structural places",
        guarantee.structural_places.len(),
    )?;
    for place in &guarantee.structural_places {
        if matches!(place.kind, StructuralPlaceKind::BlockParameter { .. }) {
            return Err(CodecError::InvalidTag("ContentStructuralPlaceKind", 8));
        }
        writer.id(place.id);
        encode_structural_place_kind(writer, place.kind);
    }
    encode_content_conservation(writer, &guarantee.conservation)
}

pub(crate) fn encode_content_transformations(
    writer: &mut Writer,
    derivations: &[terminal_psi::ContentTransformationDerivation],
) -> Result<(), CodecError> {
    writer.len("content transformation derivations", derivations.len())?;
    for derived in derivations {
        encode_content_conservation(writer, &derived.conservation)?;
        writer.id(derived.lineage_anchor);
        writer.len("content ancestry", derived.ancestry.len())?;
        for step in &derived.ancestry {
            writer.id(step.producer_operation);
            for claims in [&step.inputs, &step.outputs] {
                writer.len("content ancestry claims", claims.len())?;
                for claim in claims {
                    writer.id(*claim);
                }
            }
        }
        for bindings in [&derived.inputs, &derived.outputs] {
            writer.len("content transformation claim projections", bindings.len())?;
            for binding in bindings {
                writer.id(binding.claim);
                writer.id(binding.projection.domain);
                writer.u64(binding.projection.projection_report_fingerprint);
                encode_content_structural_place(writer, &binding.subject)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_content_transformations(
    reader: &mut Reader<'_>,
) -> Result<Vec<terminal_psi::ContentTransformationDerivation>, CodecError> {
    fn bindings(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<terminal_psi::ContentClaimProjection>, CodecError> {
        super::wire::decode_counted(reader, |reader| {
            Ok(terminal_psi::ContentClaimProjection {
                claim: reader.id("ClaimId")?,
                projection: ContentProjectionIdentity {
                    domain: reader.id("ContentDomainId")?,
                    projection_report_fingerprint: reader.u64()?,
                },
                subject: decode_content_structural_place(reader)?,
            })
        })
    }
    super::wire::decode_counted(reader, |reader| {
        Ok(terminal_psi::ContentTransformationDerivation {
            conservation: decode_content_conservation(reader)?,
            lineage_anchor: reader.id("ClaimId")?,
            ancestry: super::wire::decode_counted(reader, |reader| {
                Ok(terminal_psi::ContentLineageStep {
                    producer_operation: reader.id("OperationId")?,
                    inputs: super::wire::decode_counted(reader, |reader| reader.id("ClaimId"))?,
                    outputs: super::wire::decode_counted(reader, |reader| reader.id("ClaimId"))?,
                })
            })?,
            inputs: bindings(reader)?,
            outputs: bindings(reader)?,
        })
    })
}

fn encode_content_conservation(
    writer: &mut Writer,
    conservation: &ContentConservation,
) -> Result<(), CodecError> {
    encode_content_algebra(writer, conservation.algebra())?;
    encode_content_term(writer, conservation.left(), 0)?;
    encode_content_term(writer, conservation.right(), 0)
}

pub(crate) fn encode_content_identity_reshuffle(
    writer: &mut Writer,
    reshuffle: &ContentIdentityReshuffle,
) -> Result<(), CodecError> {
    writer.id(reshuffle.claim);
    encode_content_structural_place(writer, &reshuffle.input)?;
    encode_content_structural_place(writer, &reshuffle.output)?;
    encode_claim_content_projections(writer, &reshuffle.projections)
}

fn encode_claim_content_projections(
    writer: &mut Writer,
    projections: &[ClaimContentProjection],
) -> Result<(), CodecError> {
    writer.len("claim content projections", projections.len())?;
    for content in projections {
        writer.id(content.projection.domain);
        writer.u64(content.projection.projection_report_fingerprint);
        encode_content_algebra(writer, &content.algebra)?;
    }
    Ok(())
}

pub(crate) fn encode_content_algebra(
    writer: &mut Writer,
    algebra: &ContentAlgebra,
) -> Result<(), CodecError> {
    writer.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    writer.string("content algebra parameter", &algebra.parameter)
}

pub(crate) fn encode_content_term(
    writer: &mut Writer,
    term: &ContentTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            writer.u8(1);
            writer.id(projection.domain);
            writer.u64(projection.projection_report_fingerprint);
            encode_content_structural_place(writer, subject)?;
        }
        ContentTerm::Separate(terms) => {
            writer.u8(2);
            writer.len("separated content terms", terms.len())?;
            for term in terms {
                encode_content_term(writer, term, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn encode_content_structural_place(
    writer: &mut Writer,
    subject: &ContentStructuralPlace,
) -> Result<(), CodecError> {
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
    Ok(())
}

pub(crate) fn decode_content_entry_claim(
    reader: &mut Reader<'_>,
) -> Result<ContentEntryClaim, CodecError> {
    Ok(ContentEntryClaim {
        claim: reader.id("ClaimId")?,
        input: decode_content_structural_place(reader)?,
        projections: decode_claim_content_projections(reader)?,
    })
}

pub(crate) fn decode_content_partition_composition(
    reader: &mut Reader<'_>,
) -> Result<ContentPartitionComposition, CodecError> {
    let producer_operation = reader.id("OperationId")?;
    let source_report_fingerprint = reader.u64()?;
    let source_structural_places = decode_counted(reader, |reader| {
        let id = reader.id("PlaceId")?;
        let kind = decode_structural_place_kind(reader)?;
        if matches!(kind, StructuralPlaceKind::BlockParameter { .. }) {
            return Err(CodecError::InvalidTag("ContentStructuralPlaceKind", 8));
        }
        Ok(StructuralPlaceDeclaration { id, kind })
    })?;
    let source = decode_content_conservation(reader)?;
    let input_claims = decode_counted(reader, |reader| reader.id("ClaimId"))?;
    let substitutions = decode_counted(reader, |reader| {
        Ok(ContentPlaceSubstitution {
            source: decode_content_structural_place(reader)?,
            target: decode_content_structural_place(reader)?,
        })
    })?;
    let derived = decode_content_conservation(reader)?;
    Ok(ContentPartitionComposition {
        producer_operation,
        source_report_fingerprint,
        source_structural_places,
        source,
        input_claims,
        substitutions,
        derived,
    })
}

pub(crate) fn decode_content_conservation_guarantee(
    reader: &mut Reader<'_>,
) -> Result<ContentConservationGuarantee, CodecError> {
    let report_fingerprint = reader.u64()?;
    let structural_places = decode_counted(reader, |reader| {
        let id = reader.id("PlaceId")?;
        let kind = decode_structural_place_kind(reader)?;
        if matches!(kind, StructuralPlaceKind::BlockParameter { .. }) {
            return Err(CodecError::InvalidTag("ContentStructuralPlaceKind", 8));
        }
        Ok(StructuralPlaceDeclaration { id, kind })
    })?;
    let conservation = decode_content_conservation(reader)?;
    Ok(ContentConservationGuarantee {
        report_fingerprint,
        structural_places,
        conservation,
    })
}

fn decode_content_conservation(reader: &mut Reader<'_>) -> Result<ContentConservation, CodecError> {
    Ok(ContentConservation::new(
        decode_content_algebra(reader)?,
        decode_content_term(reader, 0)?,
        decode_content_term(reader, 0)?,
    ))
}

pub(crate) fn decode_content_identity_reshuffle(
    reader: &mut Reader<'_>,
) -> Result<ContentIdentityReshuffle, CodecError> {
    let claim = reader.id::<ClaimId>("ClaimId")?;
    let input = decode_content_structural_place(reader)?;
    let output = decode_content_structural_place(reader)?;
    let projections = decode_claim_content_projections(reader)?;
    Ok(ContentIdentityReshuffle {
        claim,
        input,
        output,
        projections,
    })
}

fn decode_claim_content_projections(
    reader: &mut Reader<'_>,
) -> Result<Vec<ClaimContentProjection>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: reader.id("ContentDomainId")?,
                projection_report_fingerprint: reader.u64()?,
            },
            algebra: decode_content_algebra(reader)?,
        })
    })
}

pub(crate) fn decode_content_algebra(
    reader: &mut Reader<'_>,
) -> Result<ContentAlgebra, CodecError> {
    let kind = match reader.u8()? {
        1 => ContentAlgebraKind::IntervalSet,
        2 => ContentAlgebraKind::CountedQuantity,
        tag => return Err(CodecError::InvalidTag("ContentAlgebraKind", tag)),
    };
    Ok(ContentAlgebra {
        kind,
        parameter: reader.string("content algebra parameter")?,
    })
}

pub(crate) fn decode_content_term(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ContentTerm, CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => {
            let projection = ContentProjectionIdentity {
                domain: reader.id::<ContentDomainId>("ContentDomainId")?,
                projection_report_fingerprint: reader.u64()?,
            };
            ContentTerm::Projection {
                projection,
                subject: decode_content_structural_place(reader)?,
            }
        }
        2 => {
            let terms = decode_counted(reader, |reader| decode_content_term(reader, depth + 1))?;
            ContentTerm::separate(terms).map_err(CodecError::MalformedProposition)?
        }
        tag => return Err(CodecError::InvalidTag("ContentTerm", tag)),
    })
}

fn decode_content_structural_place(
    reader: &mut Reader<'_>,
) -> Result<ContentStructuralPlace, CodecError> {
    let version = match reader.u8()? {
        1 => ContentPlaceVersion::Entry,
        2 => ContentPlaceVersion::Current,
        tag => return Err(CodecError::InvalidTag("ContentPlaceVersion", tag)),
    };
    let root = reader.id("PlaceId")?;
    let segments = decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(ContentPlaceSegment::Field(reader.string("content case")?)),
        2 => Ok(ContentPlaceSegment::FixedIndex(reader.u64()?)),
        3 => Ok(ContentPlaceSegment::Case(reader.string("content case")?)),
        tag => Err(CodecError::InvalidTag("ContentPlaceSegment", tag)),
    })?;
    Ok(ContentStructuralPlace {
        version,
        root,
        segments,
    })
}
