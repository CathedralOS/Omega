//! Authenticated scalar-plus-structural selected-plan payload.

use selected_instructions::SelectedInstructionPlan;
use sha2::{Digest, Sha256};

use crate::FixedViewCopyDecodeError;

use super::{
    decode_selected_plan_v4, encode_selected_plan_v4,
    structural::{
        decode_structural_function_v5, decode_structural_function_v6,
        encode_structural_function_v5, encode_structural_function_v6,
    },
};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{Cursor, length};

pub(crate) struct DecodedSelectedPlan {
    pub(crate) plan: SelectedInstructionPlan,
    pub(crate) payload_matches: bool,
}

pub(super) fn encode(
    bytes: &mut Vec<u8>,
    plan: &SelectedInstructionPlan,
    retain_call_contract: bool,
) {
    let payload = canonical_bytes(plan, retain_call_contract);
    bytes.extend_from_slice(&<[u8; 32]>::from(Sha256::digest(&payload)));
    length(bytes, payload.len());
    bytes.extend_from_slice(&payload);
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
    retain_call_contract: bool,
) -> Result<DecodedSelectedPlan, FixedViewCopyDecodeError> {
    let expected_digest: [u8; 32] = cursor.array()?;
    let payload_length = cursor.length()?;
    let payload = cursor.take(payload_length)?;
    let mut payload_cursor = Cursor::new(payload);
    let mut plan = decode_selected_plan_v4(&mut payload_cursor)?;
    let structural_count = payload_cursor.length()?;
    let mut structural_unit_functions =
        Vec::with_capacity(structural_count.min(payload_cursor.remaining()));
    for _ in 0..structural_count {
        structural_unit_functions.push(if retain_call_contract {
            decode_structural_function_v6(&mut payload_cursor)?
        } else {
            decode_structural_function_v5(&mut payload_cursor)?
        });
    }
    if payload_cursor.remaining() != 0 {
        return Err(FixedViewCopyDecodeError::TrailingBytes);
    }
    plan.structural_unit_functions = structural_unit_functions;
    let digest_matches = <[u8; 32]>::from(Sha256::digest(payload)) == expected_digest;
    let canonical_matches = canonical_bytes(&plan, retain_call_contract).as_slice() == payload;
    Ok(DecodedSelectedPlan {
        plan,
        payload_matches: digest_matches && canonical_matches,
    })
}

fn canonical_bytes(plan: &SelectedInstructionPlan, retain_call_contract: bool) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_selected_plan_v4(&mut payload, plan);
    length(&mut payload, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        if retain_call_contract {
            encode_structural_function_v6(&mut payload, function);
        } else {
            encode_structural_function_v5(&mut payload, function);
        }
    }
    payload
}
