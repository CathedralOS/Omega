//! The crate-local domain-tagged SHA-256 digest encoder every native-artifact
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

    pub(crate) fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    /// Feeds bytes with no length frame — for callers whose protocol feeds
    /// fixed-width or self-delimiting content.
    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// The crate's single-content identity: u64 length, then the raw content.
    pub(crate) fn content(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
        let mut digest = Self::new(domain);
        digest.u64(bytes.len() as u64);
        digest.raw(bytes);
        digest.finish()
    }
}
