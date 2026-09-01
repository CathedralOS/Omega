//! Optimizer module role: stage group. Canonical core artifact codec.
//!
//! Encoding frames identity-bearing content. Decoding reconstructs every row,
//! rejects trailing data, and authenticates the semantic identity before
//! returning an unchecked plan for independent rule validation.

mod cursor;
mod decode;
mod encode;

use super::{Aarch64SameViewCopyElisionDecodeError, Aarch64SameViewCopyElisionPlan};

pub(super) fn encode(plan: &Aarch64SameViewCopyElisionPlan) -> Vec<u8> {
    encode::encode(plan)
}

pub(super) fn decode(
    encoded: &[u8],
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionDecodeError> {
    let plan = decode::decode(encoded)?;
    decode::authenticate(plan)
}
