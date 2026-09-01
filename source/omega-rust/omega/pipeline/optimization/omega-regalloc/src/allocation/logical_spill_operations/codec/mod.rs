//! Optimizer module role: stage group. Versioned logical-spill transport leaves.

mod cursor;
mod decoding;
mod encoding;
mod values;

use super::{LogicalSpillOperationDecodeError, LogicalSpillOperationPlan};

const MAGIC: &[u8; 8] = b"OMGSLP\0\0";
const VERSION: u32 = 1;

pub(super) use encoding::encode_content;

pub(super) fn encode(plan: &LogicalSpillOperationPlan) -> Vec<u8> {
    encoding::encode(plan)
}

pub(super) fn decode(
    encoded: &[u8],
) -> Result<LogicalSpillOperationPlan, LogicalSpillOperationDecodeError> {
    decoding::decode(encoded)
}
