//! Canonical content-custody and content-term wire format.
//!
//! This module owns exact content claims, partition compositions,
//! conservation/reshuffle rows, algebras, recursive terms, and structural
//! places. It does not validate content authority or interpret projections.

use semantic_vocabulary::{
    ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm,
};
use terminal_psi::{
    ClaimContentProjection, ContentConservationGuarantee, ContentEntryClaim,
    ContentIdentityReshuffle, ContentPartitionComposition, ContentPlaceSubstitution,
    StructuralPlaceDeclaration,
};

use super::wire::{Reader, Writer};
use super::{
    CodecError, MAX_CONTENT_TERM_DEPTH, decode_counted, decode_structural_place_kind,
    encode_structural_place_kind,
};

pub(super) fn encode_content_entry_claim(
    writer: &mut Writer,
    binding: &ContentEntryClaim,
) -> Result<(), CodecError> {
    writer.id(binding.claim);
    encode_content_structural_place(writer, &binding.input)?;
    encode_claim_content_projections(writer, &binding.projections)
}

pub(super) fn encode_content_partition_composition(
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

pub(super) fn encode_content_conservation_guarantee(
    writer: &mut Writer,
    guarantee: &ContentConservationGuarantee,
) -> Result<(), CodecError> {
    writer.u64(guarantee.report_fingerprint);
    writer.len(
        "content guarantee structural places",
        guarantee.structural_places.len(),
    )?;
    for place in &guarantee.structural_places {
        writer.id(place.id);
        encode_structural_place_kind(writer, place.kind);
    }
    encode_content_conservation(writer, &guarantee.conservation)
}

fn encode_content_conservation(
    writer: &mut Writer,
    conservation: &ContentConservation,
) -> Result<(), CodecError> {
    encode_content_algebra(writer, conservation.algebra())?;
    encode_content_term(writer, conservation.left(), 0)?;
    encode_content_term(writer, conservation.right(), 0)
}

pub(super) fn encode_content_identity_reshuffle(
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

pub(super) fn encode_content_algebra(
    writer: &mut Writer,
    algebra: &ContentAlgebra,
) -> Result<(), CodecError> {
    writer.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    writer.string("content algebra parameter", &algebra.parameter)
}

pub(super) fn encode_content_term(
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

pub(super) fn decode_content_entry_claim(
    reader: &mut Reader<'_>,
) -> Result<ContentEntryClaim, CodecError> {
    Ok(ContentEntryClaim {
        claim: reader.id("ClaimId")?,
        input: decode_content_structural_place(reader)?,
        projections: decode_claim_content_projections(reader)?,
    })
}

pub(super) fn decode_content_partition_composition(
    reader: &mut Reader<'_>,
) -> Result<ContentPartitionComposition, CodecError> {
    let producer_operation = reader.id("OperationId")?;
    let source_report_fingerprint = reader.u64()?;
    let source_place_count = reader.count()?;
    let mut source_structural_places = Vec::new();
    for _ in 0..source_place_count {
        source_structural_places.push(StructuralPlaceDeclaration {
            id: reader.id("PlaceId")?,
            kind: decode_structural_place_kind(reader)?,
        });
    }
    let source = decode_content_conservation(reader)?;
    let input_claim_count = reader.count()?;
    let mut input_claims = Vec::new();
    for _ in 0..input_claim_count {
        input_claims.push(reader.id("ClaimId")?);
    }
    let substitution_count = reader.count()?;
    let mut substitutions = Vec::new();
    for _ in 0..substitution_count {
        substitutions.push(ContentPlaceSubstitution {
            source: decode_content_structural_place(reader)?,
            target: decode_content_structural_place(reader)?,
        });
    }
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

pub(super) fn decode_content_conservation_guarantee(
    reader: &mut Reader<'_>,
) -> Result<ContentConservationGuarantee, CodecError> {
    let report_fingerprint = reader.u64()?;
    let structural_places = decode_counted(reader, |reader| {
        Ok(StructuralPlaceDeclaration {
            id: reader.id("PlaceId")?,
            kind: decode_structural_place_kind(reader)?,
        })
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

pub(super) fn decode_content_identity_reshuffle(
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
    let count = reader.count()?;
    let mut projections = Vec::new();
    for _ in 0..count {
        projections.push(ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: reader.id("ContentDomainId")?,
                projection_report_fingerprint: reader.u64()?,
            },
            algebra: decode_content_algebra(reader)?,
        });
    }
    Ok(projections)
}

pub(super) fn decode_content_algebra(
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

pub(super) fn decode_content_term(
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
            let count = reader.count()?;
            let mut terms = Vec::new();
            for _ in 0..count {
                terms.push(decode_content_term(reader, depth + 1)?);
            }
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
    let count = reader.count()?;
    let mut segments = Vec::new();
    for _ in 0..count {
        segments.push(match reader.u8()? {
            1 => ContentPlaceSegment::Field(reader.string("content field")?),
            2 => ContentPlaceSegment::FixedIndex(reader.u64()?),
            3 => ContentPlaceSegment::Case(reader.string("content case")?),
            tag => return Err(CodecError::InvalidTag("ContentPlaceSegment", tag)),
        });
    }
    Ok(ContentStructuralPlace {
        version,
        root,
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};

    fn place() -> ContentStructuralPlace {
        ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: PlaceId::new(3).expect("place identity"),
            segments: Vec::new(),
        }
    }

    fn conservation() -> ContentConservation {
        let term = ContentTerm::Projection {
            projection: ContentProjectionIdentity {
                domain: ContentDomainId::new(5).expect("content domain identity"),
                projection_report_fingerprint: 0xfeed,
            },
            subject: place(),
        };
        ContentConservation::new(
            ContentAlgebra {
                kind: ContentAlgebraKind::IntervalSet,
                parameter: "Address".to_owned(),
            },
            term.clone(),
            term,
        )
    }

    #[test]
    fn producer_operation_and_boundary_guarantee_round_trip_exactly() {
        let guarantee = ContentConservationGuarantee {
            report_fingerprint: 0x1234,
            structural_places: vec![StructuralPlaceDeclaration {
                id: PlaceId::new(3).expect("place identity"),
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            }],
            conservation: conservation(),
        };
        let composition = ContentPartitionComposition {
            producer_operation: OperationId::new(17).expect("operation identity"),
            source_report_fingerprint: guarantee.report_fingerprint,
            source_structural_places: guarantee.structural_places.clone(),
            source: guarantee.conservation.clone(),
            input_claims: Vec::new(),
            substitutions: Vec::new(),
            derived: guarantee.conservation.clone(),
        };

        let mut writer = Writer::default();
        encode_content_conservation_guarantee(&mut writer, &guarantee).expect("encode guarantee");
        encode_content_partition_composition(&mut writer, &composition)
            .expect("encode composition");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);

        assert_eq!(
            decode_content_conservation_guarantee(&mut reader),
            Ok(guarantee)
        );
        assert_eq!(
            decode_content_partition_composition(&mut reader),
            Ok(composition)
        );
        assert_eq!(reader.remaining(), 0);
    }
}
