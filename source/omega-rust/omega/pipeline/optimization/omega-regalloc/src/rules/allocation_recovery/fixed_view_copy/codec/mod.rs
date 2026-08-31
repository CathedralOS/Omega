//! Optimizer module role: executable entrance. Versioned fixed-view-copy artifact envelope.
//!
//! Owns admission and authentication order: V4/V5 are decode-compatible;
//! public V6 binds semantic call custody, canonical payload, and exact content.

mod content;
mod copy;
mod envelope;
mod primitives;
mod selected;
mod values;

#[cfg(test)]
mod test_support;

use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::identity::fixed_view_copy_identity_v3_legacy;

use self::envelope::{v5_identity, v6_identity};
use self::primitives::Cursor;

#[cfg(test)]
use self::test_support::{encode_v4, encode_v5};

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
const LEGACY_V4_VERSION: u32 = 4;
const LEGACY_V5_VERSION: u32 = 5;
const VERSION: u32 = 6;
impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Vec<u8> {
        let mut content = Vec::new();
        content::encode_v6(&mut content, self);
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&v6_identity(self, &content));
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(MAGIC.len())? != MAGIC {
            return Err(FixedViewCopyDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if !matches!(version, LEGACY_V4_VERSION | LEGACY_V5_VERSION | VERSION) {
            return Err(FixedViewCopyDecodeError::UnsupportedVersion(version));
        }
        let identity: [u8; 32] = cursor.array()?;
        let content_offset = cursor.offset;
        let decoded = match version {
            LEGACY_V4_VERSION => content::decode_v4(&mut cursor)?,
            LEGACY_V5_VERSION => content::decode_v5(&mut cursor)?,
            VERSION => content::decode_v6(&mut cursor)?,
            _ => unreachable!("version admission is exhaustive"),
        };
        if cursor.remaining() != 0 {
            return Err(FixedViewCopyDecodeError::TrailingBytes);
        }
        if version == VERSION && !decoded.transformed_payload_matches {
            return Err(FixedViewCopyDecodeError::TransformedPayloadMismatch);
        }
        let transformed = if version == VERSION {
            selected_instruction_plan_identity(&decoded.plan.transformed)
        } else {
            omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v11_legacy(
                &decoded.plan.transformed,
            )
        };
        if transformed != decoded.expected_transformed {
            return Err(FixedViewCopyDecodeError::TransformedIdentityMismatch);
        }
        let actual_identity = match version {
            LEGACY_V4_VERSION => fixed_view_copy_identity_v3_legacy(&decoded.plan).bytes(),
            LEGACY_V5_VERSION => v5_identity(&decoded.plan, &encoded[content_offset..]),
            VERSION => v6_identity(&decoded.plan, &encoded[content_offset..]),
            _ => unreachable!("version admission is exhaustive"),
        };
        if actual_identity != identity {
            return Err(FixedViewCopyDecodeError::IdentityMismatch);
        }
        Ok(decoded.plan)
    }
}

#[cfg(test)]
mod tests;
