//! Optimizer module role: executable entrance. Versioned fixed-view-copy artifact envelope.
//!
//! Owns admission and authentication order: V4 is decode-only; public V5 binds
//! the semantic selected identity, canonical payload, and exact content.

mod content;
mod copy;
mod primitives;
mod selected;
mod values;

use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;
use sha2::{Digest, Sha256};

use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan, fixed_view_copy_identity};

use self::primitives::Cursor;

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
const LEGACY_VERSION: u32 = 4;
const VERSION: u32 = 5;
const V5_ENVELOPE_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v5\0";

fn v5_envelope_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V5_ENVELOPE_DOMAIN);
    hasher.update(fixed_view_copy_identity(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}

impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Vec<u8> {
        let mut content = Vec::new();
        content::encode_v5(&mut content, self);
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&v5_envelope_identity(self, &content));
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(MAGIC.len())? != MAGIC {
            return Err(FixedViewCopyDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != LEGACY_VERSION && version != VERSION {
            return Err(FixedViewCopyDecodeError::UnsupportedVersion(version));
        }
        let identity: [u8; 32] = cursor.array()?;
        let content_offset = cursor.offset;
        let decoded = match version {
            LEGACY_VERSION => content::decode_v4(&mut cursor)?,
            VERSION => content::decode_v5(&mut cursor)?,
            _ => unreachable!("version admission is exhaustive"),
        };
        if cursor.remaining() != 0 {
            return Err(FixedViewCopyDecodeError::TrailingBytes);
        }
        if version == VERSION && !decoded.transformed_payload_matches {
            return Err(FixedViewCopyDecodeError::TransformedPayloadMismatch);
        }
        if selected_instruction_plan_identity(&decoded.plan.transformed)
            != decoded.expected_transformed
        {
            return Err(FixedViewCopyDecodeError::TransformedIdentityMismatch);
        }
        let actual_identity = match version {
            LEGACY_VERSION => fixed_view_copy_identity(&decoded.plan).bytes(),
            VERSION => v5_envelope_identity(&decoded.plan, &encoded[content_offset..]),
            _ => unreachable!("version admission is exhaustive"),
        };
        if actual_identity != identity {
            return Err(FixedViewCopyDecodeError::IdentityMismatch);
        }
        Ok(decoded.plan)
    }
}

#[cfg(test)]
fn encode_v4(plan: &FixedViewCopyPlan) -> Vec<u8> {
    assert!(
        plan.transformed.structural_unit_functions.is_empty(),
        "the legacy V4 selected payload cannot represent structural Unit functions"
    );
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&LEGACY_VERSION.to_le_bytes());
    encoded.extend_from_slice(&fixed_view_copy_identity(plan).bytes());
    content::encode_v4(&mut encoded, plan);
    encoded
}

#[cfg(test)]
mod tests;
