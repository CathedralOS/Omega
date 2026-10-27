//! Preserve read-result identity and submitted layout; decoding does not derive either.
use calling_conventions::{ConventionalSumCaseLayout, ConventionalSumLayout, PackedFieldLayout};
use semantic_vocabulary::{ClaimId, PlaceId, StructuralDomainId, StructuralTypeId};
use terminal_psi::{StructuralOperationResult, StructuralResultClaimBinding};

use super::calling::decode_shape;
use super::declarations::{decode_multiplicity, decode_path, encode_multiplicity};
use super::projected_qualifications::{decode_projected, encode_projected};
use crate::FixedViewCopyDecodeError;
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, decode_list, encode_ids, length,
};
use legalized_operations::encode_shape;
use selected_instructions::effects::program::identity::encode_path;

pub(super) fn encode(
    bytes: &mut Vec<u8>,
    result: &StructuralOperationResult,
    layout: &ConventionalSumLayout,
) {
    encode_result(bytes, result);
    encode_layout(bytes, layout);
}

pub(super) fn encode_layout(bytes: &mut Vec<u8>, layout: &ConventionalSumLayout) {
    encode_shape(bytes, layout.shape);
    crate::codec_bytes::push_encoded(bytes, layout.tag_byte_offset.to_le_bytes());
    encode_shape(bytes, layout.tag_shape);
    encode_fields(bytes, &layout.common_fields);
    crate::codec_bytes::push_encoded(bytes, layout.payload_byte_offset.to_le_bytes());
    length(bytes, layout.cases.len());
    for case in &layout.cases {
        encode_fields(bytes, &case.fields);
    }
}

pub(super) fn encode_result(bytes: &mut Vec<u8>, result: &StructuralOperationResult) {
    crate::codec_bytes::push_encoded(bytes, result.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected(bytes, &result.projected_qualifications, true);
    length(bytes, result.claims.len());
    for claim in &result.claims {
        crate::codec_bytes::push_encoded(bytes, claim.claim.get().to_le_bytes());
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
    let claims = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(StructuralResultClaimBinding {
            claim: decode_id(cursor, ClaimId::new)?,
            path: decode_path(cursor)?,
        })
    })?;
    Ok(StructuralOperationResult {
        qualification_establishments: Vec::new(),
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
    Ok((result, decode_layout(cursor)?))
}

pub(super) fn decode_layout(
    cursor: &mut Cursor<'_>,
) -> Result<ConventionalSumLayout, FixedViewCopyDecodeError> {
    let shape = decode_shape(cursor)?;
    let tag_byte_offset = cursor.u32()?;
    let tag_shape = decode_shape(cursor)?;
    let common_fields = decode_fields(cursor)?;
    let payload_byte_offset = cursor.u32()?;
    let cases = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(ConventionalSumCaseLayout {
            fields: decode_fields(cursor)?,
        })
    })?;
    Ok(ConventionalSumLayout {
        shape,
        tag_byte_offset,
        tag_shape,
        common_fields,
        payload_byte_offset,
        cases,
    })
}

fn encode_fields(bytes: &mut Vec<u8>, fields: &[PackedFieldLayout]) {
    length(bytes, fields.len());
    for field in fields {
        encode_shape(bytes, field.shape);
        crate::codec_bytes::push_encoded(bytes, field.byte_offset.to_le_bytes());
    }
}

fn decode_fields(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<PackedFieldLayout>, FixedViewCopyDecodeError> {
    decode_list(cursor, |cursor| {
        Ok(PackedFieldLayout {
            shape: decode_shape(cursor)?,
            byte_offset: cursor.u32()?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ClaimId, Cursor, PlaceId, StructuralDomainId, StructuralOperationResult,
        StructuralResultClaimBinding, StructuralTypeId, decode, encode,
    };

    #[test]
    fn result_codec_retains_qualifications_claims_and_submitted_layout() {
        let result = StructuralOperationResult {
            qualification_establishments: Vec::new(),
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
