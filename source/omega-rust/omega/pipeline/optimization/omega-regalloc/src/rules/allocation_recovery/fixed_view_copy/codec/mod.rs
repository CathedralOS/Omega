//! Optimizer module role: executable entrance. Versioned fixed-view-copy artifact envelope.
//!
//! Owns admission and authentication order. Public V10 carries the append-only
//! signed-branch selected vocabulary while retaining V4 through V9 decode.

mod content;
mod copy;
mod decoding;
mod envelope;
mod primitives;
mod selected;
mod values;

#[cfg(test)]
mod test_support;

use self::envelope::v10_identity;
use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan};

#[cfg(test)]
use self::test_support::{encode_v4, encode_v5, encode_v6, encode_v7, encode_v8, encode_v9};

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
const LEGACY_V4_VERSION: u32 = 4;
const LEGACY_V5_VERSION: u32 = 5;
const LEGACY_V6_VERSION: u32 = 6;
const LEGACY_V7_VERSION: u32 = 7;
const LEGACY_V8_VERSION: u32 = 8;
const LEGACY_V9_VERSION: u32 = 9;
const VERSION: u32 = 10;
impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Vec<u8> {
        let mut content = Vec::new();
        content::encode_v6(&mut content, self);
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&v10_identity(self, &content));
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        decoding::decode(encoded)
    }
}

#[cfg(test)]
mod tests;
