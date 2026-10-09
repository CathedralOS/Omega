//! The crate-local FNV-1a fingerprint hasher every task-plan report shares.
//! `finish` normalizes a zero digest back to the offset basis: normalized IDs
//! reserve zero as the invalid sentinel, so the representation stays total.

pub(crate) struct Fnv1a(semantic_vocabulary::Fnv1a);

impl Fnv1a {
    pub(crate) const fn new() -> Self {
        Self(semantic_vocabulary::Fnv1a::new())
    }

    pub(crate) fn byte(&mut self, byte: u8) {
        self.0.byte(byte);
    }

    pub(crate) fn flag(&mut self, value: bool) {
        self.0.flag(value);
    }

    pub(crate) fn word(&mut self, value: u64) {
        self.0.u64(value);
    }

    /// Length-framed string feed: a u64 word prefix then the raw bytes.
    pub(crate) fn string_framed(&mut self, value: &str) {
        self.0.string_framed(value);
    }

    /// NUL-terminated string feed.
    pub(crate) fn string_terminated(&mut self, value: &str) {
        self.0.string_terminated(value);
    }

    pub(crate) fn finish(self) -> u64 {
        let digest = self.0.finish();
        if digest == 0 {
            semantic_vocabulary::Fnv1a::OFFSET_BASIS
        } else {
            digest
        }
    }
}
