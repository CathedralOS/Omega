//! Optimizer module role: executable entrance. Selected-plan payload coordination inside the versioned artifact envelope.
//!
//! One ordinary function graph; structural contracts are instruction-keyed data.

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

pub(super) fn encode_ordinary_plan(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    scalar::encode(bytes, plan);
}

pub(super) fn decode_ordinary_plan(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionPlan, FixedViewCopyDecodeError> {
    scalar::decode(cursor)
}

pub(super) fn encode_selected_plan(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    payload::encode(bytes, plan);
}

pub(super) fn decode_selected_plan(
    cursor: &mut Cursor<'_>,
) -> Result<DecodedSelectedPlan, FixedViewCopyDecodeError> {
    payload::decode(cursor)
}
