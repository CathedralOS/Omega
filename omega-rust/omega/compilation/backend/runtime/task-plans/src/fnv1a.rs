//! Shared FNV-1a-64 hasher for the crate's non-authoritative report
//! fingerprints. Normalized IDs reserve zero as the invalid sentinel, so
//! `finish` substitutes the offset basis rather than manufacturing an
//! invalid ID.

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) struct Fnv1a(u64);

impl Fnv1a {
    pub(crate) const fn new() -> Self {
        Self(OFFSET)
    }

    pub(crate) fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(PRIME);
    }

    pub(crate) fn flag(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    pub(crate) fn word(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    /// Length-framed string feed: u64 little-endian length, then the bytes.
    pub(crate) fn string(&mut self, value: &str) {
        self.word(value.len() as u64);
        for byte in value.bytes() {
            self.byte(byte);
        }
    }

    pub(crate) const fn finish(self) -> u64 {
        if self.0 == 0 { OFFSET } else { self.0 }
    }
}
