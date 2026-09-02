//! Optimizer module role: executable entrance. Versioned fixed-view-copy artifact envelope.
//!
//! Owns admission and authentication order. Public V8 carries the append-only
//! predicate-aware selected vocabulary while retaining V4 through V7 decode.

mod content;
mod copy;
mod decoding;
mod envelope;
mod primitives;
mod selected;
mod values;

#[cfg(test)]
mod test_support;

use self::envelope::v8_identity;
use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan};

#[cfg(test)]
use self::test_support::{encode_v4, encode_v5, encode_v6, encode_v7};

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
const LEGACY_V4_VERSION: u32 = 4;
const LEGACY_V5_VERSION: u32 = 5;
const LEGACY_V6_VERSION: u32 = 6;
const LEGACY_V7_VERSION: u32 = 7;
const VERSION: u32 = 8;
impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Vec<u8> {
        let mut content = Vec::new();
        content::encode_v6(&mut content, self);
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&v8_identity(self, &content));
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        decoding::decode(encoded)
    }
}

#[cfg(test)]
mod tests;
