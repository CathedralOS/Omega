//! Authenticated ordinary selected-plan payload.

use selected_instructions::SelectedInstructionPlan;
use sha2::{Digest, Sha256};

use crate::FixedViewCopyDecodeError;

use super::{decode_ordinary_plan, encode_ordinary_plan};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{Cursor, length};

pub(crate) struct DecodedSelectedPlan {
    pub(crate) plan: SelectedInstructionPlan,
    pub(crate) payload_matches: bool,
}

pub(super) fn encode(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    let payload = canonical_bytes(plan);
    bytes.extend_from_slice(&<[u8; 32]>::from(Sha256::digest(&payload)));
    length(bytes, payload.len());
    bytes.extend_from_slice(&payload);
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedSelectedPlan, FixedViewCopyDecodeError> {
    let expected_digest: [u8; 32] = cursor.array()?;
    let payload_length = cursor.length()?;
    let payload = cursor.take(payload_length)?;
    let mut payload_cursor = Cursor::new(payload);
    let plan = decode_ordinary_plan(&mut payload_cursor)?;
    if payload_cursor.remaining() != 0 {
        return Err(FixedViewCopyDecodeError::TrailingBytes);
    }
    let digest_matches = <[u8; 32]>::from(Sha256::digest(payload)) == expected_digest;
    let canonical_matches = canonical_bytes(&plan).as_slice() == payload;
    Ok(DecodedSelectedPlan {
        plan,
        payload_matches: digest_matches && canonical_matches,
    })
}

fn canonical_bytes(plan: &SelectedInstructionPlan) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_ordinary_plan(&mut payload, plan);
    payload
}
