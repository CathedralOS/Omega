use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    PropositionError, StructuralPlaceKind,
};
use terminal_psi::{
    ClaimContentProjection, ContentBackingAddressTransport, ContentBodyProjectionSubstitution,
    ContentBoundaryBodyRequirement, ContentClaimAddress, ContentClaimProjection,
    ContentConservationGuarantee, ContentEntryClaim, ContentIdentityReshuffle, ContentLineageStep,
    ContentPartitionComposition, ContentPlaceSubstitution, ContentTransformationCustody,
    ContentTransformationDerivation, StructuralPlaceDeclaration,
};

use crate::CodecError;
use crate::fields::{
    ContentAlgebra as Algebra, Field, Id, Ids, List, ProjectionIdentity, Text, U64, wire_enum,
    wire_record,
};
use crate::places::{decode_structural_place_kind, encode_structural_place_kind};
use crate::wire::decode_counted;
use crate::wire::{Reader, Writer};

wire_record! {
    EntryClaim: ContentEntryClaim {
        claim: Id,
        input: ContentPlace,
        projections: List<ClaimProjection>,
    }
    PartitionComposition: ContentPartitionComposition {
        producer_operation: Id,
        source_report_fingerprint: U64,
        source_structural_places: List<ContentPlaceDeclaration>,
        source: Conservation,
        input_claims: Ids,
        substitutions: List<PlaceSubstitution>,
        derived: Conservation,
    }
    PlaceSubstitution: ContentPlaceSubstitution { source: ContentPlace, target: ContentPlace }
    Guarantee: ContentConservationGuarantee {
        report_fingerprint: U64,
        structural_places: List<ContentPlaceDeclaration>,
        conservation: Conservation,
    }
    Reshuffle: ContentIdentityReshuffle {
        claim: Id,
        input: ContentPlace,
        output: ContentPlace,
        projections: List<ClaimProjection>,
    }
    ClaimProjection: ClaimContentProjection { projection: ProjectionIdentity, algebra: Algebra }
    Derivation: ContentTransformationDerivation {
        conservation: Conservation,
        lineage_roots: Ids,
        ancestry: List<LineageStep>,
        inputs: List<ClaimSubject>,
        outputs: List<ClaimSubject>,
        custody: Custody,
    }
    LineageStep: ContentLineageStep { producer_operation: Id, inputs: Ids, outputs: Ids }
    ClaimSubject: ContentClaimProjection {
        claim: Id,
        projection: ProjectionIdentity,
        subject: ContentPlace,
    }
    BodyRequirement: ContentBoundaryBodyRequirement {
        boundary: Id,
        guarantee: Guarantee,
        substitutions: List<BodySubstitution>,
    }
    BodySubstitution: ContentBodyProjectionSubstitution { source: Term, target: Term }
    AddressTransport: ContentBackingAddressTransport { input: ClaimAddress, output: ClaimAddress }
    ClaimAddress: ContentClaimAddress { claim: Id, subject: ContentPlace }
    ContentPlace: ContentStructuralPlace {
        version: PlaceVersion,
        root: Id,
        segments: List<PlaceSegment>,
    }
}

wire_enum! {
    Custody: ContentTransformationCustody, "ContentTransformationCustody" {
        0 => Derived,
        1 => Boundary { requirements: List<BodyRequirement>, addresses: List<AddressTransport> },
    }
    PlaceVersion: ContentPlaceVersion, "ContentPlaceVersion" { 1 => Entry, 2 => Current }
    PlaceSegment: ContentPlaceSegment, "ContentPlaceSegment" {
        1 => Field(name: Text),
        2 => FixedIndex(index: U64),
        3 => Case(name: Text),
    }
}

pub struct ContentPlaceDeclaration;
impl Field<StructuralPlaceDeclaration> for ContentPlaceDeclaration {
    fn write(writer: &mut Writer, value: &StructuralPlaceDeclaration) -> Result<(), CodecError> {
        if matches!(value.kind, StructuralPlaceKind::BlockParameter { .. }) {
            return Err(CodecError::InvalidTag("ContentStructuralPlaceKind", 8));
        }
        writer.id(value.id);
        encode_structural_place_kind(writer, value.kind);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<StructuralPlaceDeclaration, CodecError> {
        let id = reader.id("PlaceId")?;
        let kind = decode_structural_place_kind(reader)?;
        if matches!(kind, StructuralPlaceKind::BlockParameter { .. }) {
            return Err(CodecError::InvalidTag("ContentStructuralPlaceKind", 8));
        }
        Ok(StructuralPlaceDeclaration { id, kind })
    }
}

struct Conservation;
impl Field<ContentConservation> for Conservation {
    fn write(writer: &mut Writer, value: &ContentConservation) -> Result<(), CodecError> {
        encode_content_algebra(writer, value.algebra())?;
        encode_content_term(writer, value.left())?;
        encode_content_term(writer, value.right())
    }
    fn read(reader: &mut Reader<'_>) -> Result<ContentConservation, CodecError> {
        Ok(ContentConservation::new(
            decode_content_algebra(reader)?,
            decode_content_term(reader)?,
            decode_content_term(reader)?,
        ))
    }
}

struct Term;
impl Field<ContentTerm> for Term {
    fn write(writer: &mut Writer, value: &ContentTerm) -> Result<(), CodecError> {
        encode_content_term(writer, value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<ContentTerm, CodecError> {
        decode_content_term(reader)
    }
}

pub fn encode_content_transformations(
    writer: &mut Writer,
    derivations: &[ContentTransformationDerivation],
) -> Result<(), CodecError> {
    writer.counted(
        "content transformation derivations",
        derivations,
        Derivation::write,
    )
}

pub fn decode_content_transformations(
    reader: &mut Reader<'_>,
) -> Result<Vec<ContentTransformationDerivation>, CodecError> {
    List::<Derivation>::read(reader)
}

pub fn encode_content_algebra(
    writer: &mut Writer,
    algebra: &ContentAlgebra,
) -> Result<(), CodecError> {
    writer.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    writer.string("content algebra parameter", &algebra.parameter)
}

/// A separation's members are projections, never separations, so this
/// encoding nests at most once and needs no table.
pub fn encode_content_term(writer: &mut Writer, term: &ContentTerm) -> Result<(), CodecError> {
    match term {
        ContentTerm::Projection { .. } => encode_content_projection(writer, term),
        ContentTerm::Separate(terms) => {
            writer.u8(2);
            writer.len("separated content terms", terms.len())?;
            for term in terms {
                encode_content_projection(writer, term)?;
            }
            Ok(())
        }
    }
}

fn encode_content_projection(writer: &mut Writer, term: &ContentTerm) -> Result<(), CodecError> {
    let ContentTerm::Projection {
        projection,
        subject,
    } = term
    else {
        return Err(CodecError::MalformedProposition(
            PropositionError::NestedContentSeparation,
        ));
    };
    writer.u8(1);
    writer.id(projection.domain);
    writer.u64(projection.projection_report_fingerprint);
    ContentPlace::write(writer, subject)
}

pub fn decode_content_algebra(reader: &mut Reader<'_>) -> Result<ContentAlgebra, CodecError> {
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

pub fn decode_content_term(reader: &mut Reader<'_>) -> Result<ContentTerm, CodecError> {
    match reader.u8()? {
        1 => decode_content_projection(reader),
        2 => {
            let terms = decode_counted(reader, |reader| match reader.u8()? {
                1 => decode_content_projection(reader),
                2 => Err(CodecError::MalformedProposition(
                    PropositionError::NestedContentSeparation,
                )),
                tag => Err(CodecError::InvalidTag("ContentTerm", tag)),
            })?;
            ContentTerm::separate(terms).map_err(CodecError::MalformedProposition)
        }
        tag => Err(CodecError::InvalidTag("ContentTerm", tag)),
    }
}

fn decode_content_projection(reader: &mut Reader<'_>) -> Result<ContentTerm, CodecError> {
    let projection = ContentProjectionIdentity {
        domain: reader.id::<ContentDomainId>("ContentDomainId")?,
        projection_report_fingerprint: reader.u64()?,
    };
    Ok(ContentTerm::Projection {
        projection,
        subject: ContentPlace::read(reader)?,
    })
}
