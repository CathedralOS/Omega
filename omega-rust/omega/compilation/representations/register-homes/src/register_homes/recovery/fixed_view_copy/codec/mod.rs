//! Optimizer module role: executable entrance. Versioned fixed-view-copy artifact envelope.
//!
//! Owns admission and authentication order. V52 adds evaluated runtime selector
//! operands for normalized foreign-call arguments to V51's owned cleanup paths.
//! Atomic field identity remains distinct from its scalar representation.
//! Every older envelope is rejected before its payload is interpreted.

mod content;
mod copy;
mod decoding;
mod envelope;
pub(crate) mod evidence;
mod primitives;
mod selected;
mod values;

use self::envelope::envelope_identity;
use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan};

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
// Version 56 retains typed claim-transfer paths and their Case guards.
// Version 57 uses source-free Terminal-scoped callback function identities.
// Version 58 retains identity-only reference targets separately from storage.
const VERSION: u32 = 58;
impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Result<Vec<u8>, terminal_codec::CodecError> {
        let mut content = Vec::new();
        content::encode_v7(&mut content, self)?;
        let mut encoded = Vec::new();
        crate::codec_bytes::encode_header(&mut encoded, MAGIC, VERSION);
        encoded.extend_from_slice(&envelope_identity(self, &content));
        encoded.extend_from_slice(&content);
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        decoding::decode(encoded)
    }
}

#[cfg(test)]
mod tests;
