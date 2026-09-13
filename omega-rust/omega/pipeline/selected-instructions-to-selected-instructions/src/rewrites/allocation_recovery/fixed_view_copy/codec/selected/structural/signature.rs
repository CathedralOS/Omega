//! Ordinary structural signature data; no separate executable function payload.
use super::declarations::*;
use crate::FixedViewCopyDecodeError;
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, encode_ids, length,
};
use legalized_operations::LegalizedStructuralContract;
use semantic_vocabulary::{PlaceId, ServiceId, StructuralDomainId, StructuralTypeId};
pub(super) fn encode_signature(bytes: &mut Vec<u8>, value: &LegalizedStructuralContract) {
    length(bytes, value.structural_types.len());
    for row in &value.structural_types {
        encode_type(bytes, row);
    }
    length(bytes, value.parameters.len());
    for row in &value.parameters {
        encode_parameter(bytes, row, true);
    }
    match &value.result {
        None => bytes.push(0),
        Some(result) => {
            bytes.push(1);
            bytes.extend_from_slice(&result.place.get().to_le_bytes());
            bytes.extend_from_slice(&result.structural_type.get().to_le_bytes());
            encode_multiplicity(bytes, result.multiplicity);
            encode_ids(
                bytes,
                result.qualifications.iter().map(|domain| domain.get()),
            );
            super::projected_qualifications::encode_projected(
                bytes,
                &result.projected_qualifications,
                true,
            );
            length(bytes, result.reference_sources.len());
            for reference in &result.reference_sources {
                encode_path(bytes, &reference.path);
                encode_semantic_argument(bytes, &reference.source);
            }
        }
    }
    length(bytes, value.structural_places.len());
    for row in &value.structural_places {
        encode_place(bytes, *row);
    }
    length(bytes, value.entry_claims.len());
    for row in &value.entry_claims {
        encode_entry_claim(bytes, row);
    }
    encode_ids(
        bytes,
        value.published_service_ceiling.iter().map(|id| id.get()),
    );
}
pub(super) fn decode_signature(
    cursor: &mut Cursor<'_>,
) -> Result<LegalizedStructuralContract, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut structural_types = Vec::new();
    for _ in 0..count {
        structural_types.push(decode_type(cursor)?);
    }
    let count = cursor.length()?;
    let mut parameters = Vec::new();
    for _ in 0..count {
        parameters.push(decode_parameter(cursor, true)?);
    }
    let result = match cursor.byte()? {
        0 => None,
        1 => Some(terminal_psi::StructuralResultDeclaration {
            place: decode_id(cursor, PlaceId::new)?,
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
            multiplicity: decode_multiplicity(cursor)?,
            qualifications: decode_ids(cursor, StructuralDomainId::new)?,
            projected_qualifications: super::projected_qualifications::decode_projected(
                cursor, true,
            )?,
            reference_sources: {
                let count = cursor.length()?;
                let mut sources = Vec::new();
                for _ in 0..count {
                    sources.push(terminal_psi::StructuralReferenceResultSource {
                        path: decode_path(cursor)?,
                        source: decode_semantic_argument(cursor)?,
                    });
                }
                sources
            },
        }),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let count = cursor.length()?;
    let mut structural_places = Vec::new();
    for _ in 0..count {
        structural_places.push(decode_place(cursor)?);
    }
    let count = cursor.length()?;
    let mut entry_claims = Vec::new();
    for _ in 0..count {
        entry_claims.push(decode_entry_claim(cursor)?);
    }
    Ok(LegalizedStructuralContract {
        structural_types: structural_types.into(),
        parameters,
        result,
        structural_places,
        entry_claims,
        published_service_ceiling: decode_ids(cursor, ServiceId::new)?,
    })
}
