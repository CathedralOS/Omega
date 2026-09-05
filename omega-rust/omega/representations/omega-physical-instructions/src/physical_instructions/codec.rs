//! Version-5 post-allocation machine-plan framing and content authentication.
//!
//! The entrance owns the wire marker/version, canonical content boundary,
//! trailing-byte rejection, and final identity authentication. Ordered content,
//! instruction vocabulary, cursor translation, and errors descend explicitly.

mod cursor;
mod error;
#[cfg(test)]
mod tests;
mod v3;

pub use error::PostAllocationMachineDecodeError;

use crate::{
    PostAllocationMachineIdentity, PostAllocationMachinePlan, post_allocation_machine_identity,
};
use omega_selected_instructions::selected_instructions::effects::program::encoding as effect_codec;

const MAGIC: &[u8; 8] = b"OMGPMX\0\0";
const LEGACY_V3_VERSION: u32 = 3;
const LEGACY_V4_VERSION: u32 = 4;
const VERSION: u32 = 5;

pub(crate) fn encode_terminal_post_allocation_machine_plan(
    plan: &PostAllocationMachinePlan,
) -> Vec<u8> {
    let content = super::identity::encode_terminal_post_allocation_machine_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
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
    if !matches!(version, LEGACY_V3_VERSION | LEGACY_V4_VERSION | VERSION) {
        return Err(PostAllocationMachineDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = PostAllocationMachineIdentity::from_bytes(cursor::array(&mut cursor)?);
    let plan = v3::decode_content(
        &mut cursor,
        identity,
        matches!(version, LEGACY_V4_VERSION | VERSION),
        version == VERSION,
    )?;
    if cursor.remaining() != 0 {
        return Err(PostAllocationMachineDecodeError::TrailingBytes);
    }
    let expected_identity = match version {
        LEGACY_V3_VERSION => super::identity::post_allocation_machine_identity_v4_legacy(&plan),
        LEGACY_V4_VERSION => super::identity::post_allocation_machine_identity_v5_legacy(&plan),
        VERSION => post_allocation_machine_identity(&plan),
        _ => unreachable!("wire version admitted above"),
    };
    if plan.identity != expected_identity {
        return Err(PostAllocationMachineDecodeError::InvalidIdentity);
    }
    Ok(plan)
}
