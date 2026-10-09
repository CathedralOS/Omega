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

    pub(crate) fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }

    pub(crate) fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    pub(crate) fn len(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.0.update(value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.0.update(value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    pub(crate) fn i64(&mut self, value: i64) {
        self.0.update(value.to_le_bytes());
    }

    /// Feeds bytes with no length frame — for callers whose protocol feeds
    /// fixed-width or self-delimiting content.
    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.0.update(bytes);
    }

    pub(crate) fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    pub(crate) fn strings(&mut self, values: &[String]) {
        self.len(values.len());
        for value in values {
            self.string(value);
        }
    }

    pub(crate) fn optional_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.byte(1);
                self.string(value);
            }
            None => self.byte(0),
        }
    }
}

/// Content digest with no domain frame — the `Sha256(content)` one-shot.
pub(crate) fn content_identity(content: &[u8]) -> [u8; 32] {
    let mut digest = DomainDigestEncoder::new(b"");
    digest.raw(content);
    digest.finish().into()
}
