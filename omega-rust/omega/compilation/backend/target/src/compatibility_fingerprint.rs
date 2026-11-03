//! Length-framed compatibility fingerprint primitive. Callers own their
//! domains and field encoding; each new writer starts an independent digest.
//! Unlike `semantic_vocabulary::Fnv1a`, `bytes` here is length-framed: changing
//! that would move every persisted compatibility fingerprint.

pub struct Fnv1a(u64);

impl Fnv1a {
    pub const fn new() -> Self {
        Self(semantic_vocabulary::Fnv1a::OFFSET_BASIS)
    }

    pub fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(semantic_vocabulary::Fnv1a::PRIME);
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    pub const fn finish(self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a {
    fn default() -> Self {
        Self::new()
    }
}
