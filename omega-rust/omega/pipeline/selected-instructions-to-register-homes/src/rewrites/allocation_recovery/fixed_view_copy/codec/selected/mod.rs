//! Optimizer module role: executable entrance. Versioned selected-plan wire coordination.
//!
//! V4 owns the historical scalar roster. V5 introduced the authenticated
//! structural taxonomy; V6 extends its call leaf with proof/crash custody.
//! V8 appended predicate-aware instruction and terminator tags. V9 appends the
//! direct scalar-call tag and callee payload to that byte-stable taxonomy.

mod block;
mod function;
mod instruction;
mod payload;
mod provenance;
mod register;
mod scalar;
mod structural;

use crate::FixedViewCopyDecodeError;
use selected_instructions::SelectedInstructionPlan;

use self::payload::DecodedSelectedPlan;
use super::primitives::Cursor;

#[cfg(test)]
pub(super) use self::instruction::decode_kind;

pub(super) fn encode_selected_plan_v4(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    scalar::encode(bytes, plan);
}

pub(super) fn decode_selected_plan_v4(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionPlan, FixedViewCopyDecodeError> {
    scalar::decode(cursor)
}

#[cfg(test)]
pub(super) fn encode_selected_plan_v5(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    payload::encode(bytes, plan, false);
}

pub(super) fn encode_selected_plan_v6(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    payload::encode(bytes, plan, true);
}

pub(super) fn decode_selected_plan_v5(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedSelectedPlan, FixedViewCopyDecodeError> {
    payload::decode(cursor, false)
}

pub(super) fn decode_selected_plan_v6(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedSelectedPlan, FixedViewCopyDecodeError> {
    payload::decode(cursor, true)
}
