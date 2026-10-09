//! The crate-local domain-tagged SHA-256 digest encoder every selections
//! identity shares.
//! methods length-frame where the protocol frames; `finish` yields the raw
//! digest for each domains newtype to wrap.

use sha2::{Digest, Sha256};

pub(crate) struct DomainDigestEncoder(Sha256);

impl DomainDigestEncoder {
    pub(crate) fn new(domain: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(domain);
        Self(digest)
    }

    pub(crate) fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    pub(crate) fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.0.update(value.to_le_bytes());
    }

    /// Feeds bytes with no length frame — for callers whose protocol feeds
    /// fixed-width or self-delimiting content.
    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}
