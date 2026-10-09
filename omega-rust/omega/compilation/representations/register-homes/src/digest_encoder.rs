//! The crate-local domain-tagged SHA-256 digest encoder every installation
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

    /// Feeds bytes with no length frame — for callers whose protocol feeds
    /// fixed-width or self-delimiting content.
    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}

/// Content digest with no domain frame — the `Sha256(content)` one-shot.
pub(crate) fn content_identity(content: &[u8]) -> [u8; 32] {
    let mut digest = DomainDigestEncoder::new(b"");
    digest.raw(content);
    digest.finish().into()
}
