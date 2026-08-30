//! Versioned fixed-view-copy artifact envelope.
//!
//! The entrance owns protocol admission and authentication order. `content`
//! owns the v4 field roster; lower modules own each encoded domain.

mod content;
mod copy;
mod primitives;
mod selected;
mod values;

use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::{
    FixedViewCopyDecodeError, FixedViewCopyIdentity, FixedViewCopyPlan, fixed_view_copy_identity,
};

use self::primitives::Cursor;

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
const VERSION: u32 = 4;

impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&fixed_view_copy_identity(self).bytes());
        content::encode(&mut encoded, self);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(MAGIC.len())? != MAGIC {
            return Err(FixedViewCopyDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != VERSION {
            return Err(FixedViewCopyDecodeError::UnsupportedVersion(version));
        }
        let identity = FixedViewCopyIdentity::from_bytes(cursor.array()?);
        let decoded = content::decode(&mut cursor)?;
        if cursor.remaining() != 0 {
            return Err(FixedViewCopyDecodeError::TrailingBytes);
        }
        if selected_instruction_plan_identity(&decoded.plan.transformed)
            != decoded.expected_transformed
        {
            return Err(FixedViewCopyDecodeError::TransformedIdentityMismatch);
        }
        if fixed_view_copy_identity(&decoded.plan) != identity {
            return Err(FixedViewCopyDecodeError::IdentityMismatch);
        }
        Ok(decoded.plan)
    }
}

#[cfg(test)]
mod tests;
