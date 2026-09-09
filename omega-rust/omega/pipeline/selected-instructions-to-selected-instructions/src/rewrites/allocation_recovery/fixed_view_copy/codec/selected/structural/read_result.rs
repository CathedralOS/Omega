//! Preserve read-result identity and submitted layout; decoding does not derive either.
use calling_conventions::{ConventionalSumCaseLayout, ConventionalSumLayout, PackedFieldLayout};
use semantic_vocabulary::{ClaimId, PlaceId, StructuralDomainId, StructuralTypeId};
use terminal_psi::{StructuralOperationResult, StructuralResultClaimBinding};

use super::calling::{decode_shape, encode_shape};
use super::declarations::{decode_multiplicity, decode_path, encode_multiplicity, encode_path};
use super::projected_qualifications::{decode_projected, encode_projected};
use crate::FixedViewCopyDecodeError;
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, encode_ids, length,
};

pub(super) fn encode(
    bytes: &mut Vec<u8>,
    result: &StructuralOperationResult,
    layout: &ConventionalSumLayout,
) {
    encode_result(bytes, result);
    encode_shape(bytes, layout.shape);
    bytes.extend_from_slice(&layout.tag_byte_offset.to_le_bytes());
    encode_shape(bytes, layout.tag_shape);
    encode_fields(bytes, &layout.common_fields);
    bytes.extend_from_slice(&layout.payload_byte_offset.to_le_bytes());
    length(bytes, layout.cases.len());
    for case in &layout.cases {
        encode_fields(bytes, &case.fields);
    }
}

pub(super) fn encode_result(bytes: &mut Vec<u8>, result: &StructuralOperationResult) {
    bytes.extend_from_slice(&result.place.get().to_le_bytes());
    bytes.extend_from_slice(&result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected(bytes, &result.projected_qualifications, true);
    length(bytes, result.claims.len());
    for claim in &result.claims {
        bytes.extend_from_slice(&claim.claim.get().to_le_bytes());
        encode_path(bytes, &claim.path);
    }
}

pub(super) fn decode_result(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralOperationResult, FixedViewCopyDecodeError> {
    let place = decode_id(cursor, PlaceId::new)?;
    let structural_type = decode_id(cursor, StructuralTypeId::new)?;
    let multiplicity = decode_multiplicity(cursor)?;
    let qualifications = decode_ids(cursor, StructuralDomainId::new)?;
    let projected_qualifications = decode_projected(cursor, true)?;
    let count = cursor.length()?;
    let mut claims = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        claims.push(StructuralResultClaimBinding {
            claim: decode_id(cursor, ClaimId::new)?,
            path: decode_path(cursor)?,
        });
    }
    Ok(StructuralOperationResult {
        place,
        structural_type,
        multiplicity,
        qualifications,
        projected_qualifications,
        claims,
    })
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
) -> Result<(StructuralOperationResult, ConventionalSumLayout), FixedViewCopyDecodeError> {
    let result = decode_result(cursor)?;
    let shape = decode_shape(cursor)?;
    let tag_byte_offset = cursor.u16()?;
    let tag_shape = decode_shape(cursor)?;
    let common_fields = decode_fields(cursor)?;
    let payload_byte_offset = cursor.u16()?;
    let count = cursor.length()?;
    let mut cases = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        cases.push(ConventionalSumCaseLayout {
            fields: decode_fields(cursor)?,
        });
    }
    Ok((
        result,
        ConventionalSumLayout {
            shape,
            tag_byte_offset,
            tag_shape,
            common_fields,
            payload_byte_offset,
            cases,
        },
    ))
}

fn encode_fields(bytes: &mut Vec<u8>, fields: &[PackedFieldLayout]) {
    length(bytes, fields.len());
    for field in fields {
        encode_shape(bytes, field.shape);
        bytes.extend_from_slice(&field.byte_offset.to_le_bytes());
    }
}

fn decode_fields(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<PackedFieldLayout>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut fields = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        fields.push(PackedFieldLayout {
            shape: decode_shape(cursor)?,
            byte_offset: cursor.u16()?,
        });
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_codec_retains_qualifications_claims_and_submitted_layout() {
        let result = StructuralOperationResult {
            place: PlaceId::new(1).unwrap(),
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            qualifications: vec![StructuralDomainId::new(3).unwrap()],
            projected_qualifications: vec![terminal_psi::StructuralPathQualification {
                path: Vec::new(),
                domain: StructuralDomainId::new(4).unwrap(),
            }],
            claims: vec![StructuralResultClaimBinding {
                claim: ClaimId::new(5).unwrap(),
                path: Vec::new(),
            }],
        };
        let mut layout = calling_conventions::evaluate_conventional_sum_layout(
            &[],
            &[vec![], vec![calling_conventions::ValueShape::integer(4, 4)]],
        )
        .unwrap();
        // The codec must retain even a noncanonical submitted layout for later rejection.
        layout.payload_byte_offset = 8;
        let mut bytes = Vec::new();
        encode(&mut bytes, &result, &layout);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode(&mut cursor).unwrap(), (result, layout));
        assert_eq!(cursor.remaining(), 0);
        for end in 0..bytes.len() {
            assert!(decode(&mut Cursor::new(&bytes[..end])).is_err());
        }
    }
}
