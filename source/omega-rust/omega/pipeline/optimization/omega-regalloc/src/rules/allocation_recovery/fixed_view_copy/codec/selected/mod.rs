//! Optimizer module role: executable entrance. Versioned selected-plan wire coordination.
//!
//! V4 owns the historical scalar roster. V5 wraps one canonical selected-plan
//! payload, authenticates its exact bytes, and descends into the structural
//! function taxonomy without changing the legacy format.

mod block;
mod function;
mod instruction;
mod provenance;
mod register;
mod scalar;
mod structural;

use omega_selected_instructions::SelectedInstructionPlan;
use sha2::{Digest, Sha256};

use crate::FixedViewCopyDecodeError;

use self::structural::{decode_structural_function, encode_structural_function};
use super::primitives::{Cursor, length};

#[cfg(test)]
pub(super) use self::instruction::decode_kind;

pub(super) struct DecodedSelectedPlan {
    pub(super) plan: SelectedInstructionPlan,
    pub(super) payload_matches: bool,
}

pub(super) fn encode_selected_plan_v4(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    scalar::encode(bytes, plan);
}

pub(super) fn decode_selected_plan_v4(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionPlan, FixedViewCopyDecodeError> {
    scalar::decode(cursor)
}

pub(super) fn encode_selected_plan_v5(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    let payload = encode_v5_payload(plan);
    bytes.extend_from_slice(&<[u8; 32]>::from(Sha256::digest(&payload)));
    length(bytes, payload.len());
    bytes.extend_from_slice(&payload);
}

pub(super) fn decode_selected_plan_v5(
    cursor: &mut Cursor<'_>,
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
        structural_unit_functions.push(decode_structural_function(&mut payload_cursor)?);
    }
    if payload_cursor.remaining() != 0 {
        return Err(FixedViewCopyDecodeError::TrailingBytes);
    }
    plan.structural_unit_functions = structural_unit_functions;
    let digest_matches = <[u8; 32]>::from(Sha256::digest(payload)) == expected_digest;
    let canonical_matches = encode_v5_payload(&plan).as_slice() == payload;
    Ok(DecodedSelectedPlan {
        plan,
        payload_matches: digest_matches && canonical_matches,
    })
}

fn encode_v5_payload(plan: &SelectedInstructionPlan) -> Vec<u8> {
    let mut payload = Vec::new();
    encode_selected_plan_v4(&mut payload, plan);
    length(&mut payload, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        encode_structural_function(&mut payload, function);
    }
    payload
}
