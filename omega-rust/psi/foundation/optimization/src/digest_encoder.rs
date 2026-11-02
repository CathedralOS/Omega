//! The shared domain-tagged SHA-256 digest encoder every Psi identity shares.
//! Methods length-frame where the protocol frames; `finish` yields the raw
//! digest for each domain's newtype to wrap. Every crate that once carried a
//! crate-local copy of this file now imports this one: the byte protocol is
//! identical, so previously computed identities do not change.

use sha2::{Digest, Sha256};

pub struct DomainDigestEncoder(Sha256);

impl DomainDigestEncoder {
    pub fn new(domain: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(domain);
        Self(digest)
    }

    pub fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    pub fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }

    pub fn len(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    pub fn u16(&mut self, value: u16) {
        self.0.update(value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    /// Feeds bytes with no length frame — for callers whose protocol feeds
    /// fixed-width or self-delimiting content.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.0.update(bytes);
    }
}
