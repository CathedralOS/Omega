//! Version-21 post-allocation machine-plan framing and content authentication.
//!
//! The entrance owns the wire marker/version, canonical content boundary,
//! trailing-byte rejection, and final identity authentication. Ordered content,
//! instruction vocabulary, cursor translation, and errors descend explicitly.
//! Current framing retains u16 alternative-family identities and exact consumed
//! cleanup claims, rejecting earlier envelopes before interpreting their bytes.

mod cursor;
mod error;
#[cfg(test)]
mod tests;
mod v3;

pub(crate) use error::PostAllocationMachineDecodeError;

use crate::{
    PostAllocationMachineIdentity, PostAllocationMachinePlan, post_allocation_machine_identity,
};
use selected_instructions::selected_instructions::effects::program::encoding as effect_codec;

const MAGIC: &[u8; 8] = b"OMGPMX\0\0";
// Version 21 retains the widened u16 alternative-family tags.
const VERSION: u32 = 21;

pub(crate) fn encode_terminal_post_allocation_machine_plan(
    plan: &PostAllocationMachinePlan,
) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(44);
    encoded.extend_from_slice(MAGIC);
    crate::codec_bytes::push_encoded(&mut encoded, VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    super::identity::write_terminal_post_allocation_machine_content(plan, |chunk| {
        encoded.extend_from_slice(chunk);
    });
    encoded
}

pub(crate) fn decode_terminal_post_allocation_machine_plan(
    encoded: &[u8],
) -> Result<PostAllocationMachinePlan, PostAllocationMachineDecodeError> {
    let mut cursor = effect_codec::Cursor::new(encoded);
    if cursor::take(&mut cursor, MAGIC.len())? != MAGIC {
        return Err(PostAllocationMachineDecodeError::WrongMagic);
    }
    let version = cursor::u32_field(&mut cursor)?;
    if version != VERSION {
        return Err(PostAllocationMachineDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = PostAllocationMachineIdentity::from_bytes(cursor::array(&mut cursor)?);
    let plan = v3::decode_content(&mut cursor, identity, true, true, true)?;
    if cursor.remaining() != 0 {
        return Err(PostAllocationMachineDecodeError::TrailingBytes);
    }
    if plan.identity != post_allocation_machine_identity(&plan) {
        return Err(PostAllocationMachineDecodeError::InvalidIdentity);
    }
    Ok(plan)
}
