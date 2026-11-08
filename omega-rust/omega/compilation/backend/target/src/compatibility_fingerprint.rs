/*
We provide the length-framed FNV-1a compatibility fingerprint primitive shared
by target locators and observation applications. Callers own their domain tags
and field ordering; unlike `semantic_vocabulary::Fnv1a`, our `bytes` method
frames each slice with its 64-bit little-endian length so coordinate boundaries
cannot alias.
*/

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
