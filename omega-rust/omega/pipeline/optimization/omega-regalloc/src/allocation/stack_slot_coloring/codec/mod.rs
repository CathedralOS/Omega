//! Optimizer module role: stage group. Versioned stack-slot-coloring transport.

mod cursor;
mod decoding;
mod encoding;

use super::{StackSlotColoringDecodeError, StackSlotColoringPlan};

const MAGIC: &[u8; 8] = b"OMGSSC\0\0";
const VERSION: u32 = 1;

pub(super) use encoding::encode_content;

pub(super) fn encode(plan: &StackSlotColoringPlan) -> Vec<u8> {
    encoding::encode(plan)
}

pub(super) fn decode(
    encoded: &[u8],
) -> Result<StackSlotColoringPlan, StackSlotColoringDecodeError> {
    decoding::decode(encoded)
}
