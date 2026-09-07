//! Ordinary structural signature data; no separate executable function payload.
use super::declarations::*;
use crate::FixedViewCopyDecodeError;
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_ids, encode_ids, length,
};
use legalized_operations::LegalizedStructuralContract;
use semantic_vocabulary::ServiceId;
pub(super) fn encode_signature(bytes: &mut Vec<u8>, value: &LegalizedStructuralContract) {
    length(bytes, value.structural_types.len());
    for row in &value.structural_types {
        encode_type(bytes, row);
    }
    length(bytes, value.parameters.len());
    for row in &value.parameters {
        encode_parameter(bytes, row, true);
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
        structural_types,
        parameters,
        structural_places,
        entry_claims,
        published_service_ceiling: decode_ids(cursor, ServiceId::new)?,
    })
}
