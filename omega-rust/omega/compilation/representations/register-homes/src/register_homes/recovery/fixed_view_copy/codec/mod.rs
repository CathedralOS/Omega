//! Optimizer module role: executable entrance. Versioned fixed-view-copy artifact envelope.
//!
//! Owns admission and authentication order. V44 retains foreign structural
//! result origins and layouts alongside scalar completion custody.
//! Atomic field identity remains distinct from its scalar representation.
//! Every older envelope is rejected before its payload is interpreted.

mod content;
mod copy;
mod decoding;
mod envelope;
mod evidence;
mod primitives;
mod selected;
mod values;

use self::envelope::v44_identity;
use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan};

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
const VERSION: u32 = 44;
impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Result<Vec<u8>, terminal_codec::CodecError> {
        let mut content = Vec::new();
        content::encode_v7(&mut content, self)?;
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&v44_identity(self, &content));
        encoded.extend_from_slice(&content);
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        decoding::decode(encoded)
    }
}

#[cfg(test)]
mod tests;
