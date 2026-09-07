//! Optimizer module role: executable entrance. Versioned fixed-view-copy artifact envelope.
//!
//! Owns admission and authentication order. Public V13 binds semantic successors to explicit register transport.
//! Every older envelope is rejected before its payload is interpreted.

mod content;
mod copy;
mod decoding;
mod envelope;
mod evidence;
mod primitives;
mod selected;
mod values;

#[cfg(test)]
mod test_support;

use self::envelope::v13_identity;
use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan};

#[cfg(test)]
use self::test_support::{
    encode_v4, encode_v5, encode_v6, encode_v7, encode_v8, encode_v9, encode_v10, encode_v11,
};

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
#[cfg(test)]
const LEGACY_V4_VERSION: u32 = 4;
#[cfg(test)]
const LEGACY_V5_VERSION: u32 = 5;
#[cfg(test)]
const LEGACY_V6_VERSION: u32 = 6;
#[cfg(test)]
const LEGACY_V7_VERSION: u32 = 7;
#[cfg(test)]
const LEGACY_V8_VERSION: u32 = 8;
#[cfg(test)]
const LEGACY_V9_VERSION: u32 = 9;
#[cfg(test)]
const LEGACY_V10_VERSION: u32 = 10;
#[cfg(test)]
const LEGACY_V11_VERSION: u32 = 11;
const VERSION: u32 = 13;
impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Vec<u8> {
        let mut content = Vec::new();
        content::encode_v7(&mut content, self);
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&v13_identity(self, &content));
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        decoding::decode(encoded)
    }
}

#[cfg(test)]
mod tests;
