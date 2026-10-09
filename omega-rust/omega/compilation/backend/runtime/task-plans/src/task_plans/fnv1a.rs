//! The crate-local FNV-1a fingerprint hasher every task-plan report shares.
//! `finish` normalizes a zero digest back to the offset basis: normalized IDs
//! reserve zero as the invalid sentinel, so the representation stays total.

pub(crate) struct Fnv1a(u64);

impl Fnv1a {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    pub(crate) const fn new() -> Self {
        Self(Self::OFFSET)
    }

    pub(crate) fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    pub(crate) fn flag(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    pub(crate) fn word(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    /// Length-framed string feed: a u64 word prefix then the raw bytes.
    pub(crate) fn string_framed(&mut self, value: &str) {
        self.word(value.len() as u64);
        for byte in value.bytes() {
            self.byte(byte);
        }
    }

    /// NUL-terminated string feed.
    pub(crate) fn string_terminated(&mut self, value: &str) {
        for byte in value.as_bytes() {
            self.byte(*byte);
        }
        self.byte(0);
    }

    pub(crate) fn finish(self) -> u64 {
        if self.0 == 0 { Self::OFFSET } else { self.0 }
    }
}
