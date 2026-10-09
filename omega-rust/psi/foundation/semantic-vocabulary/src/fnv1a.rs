//! The FNV-1a-64 fingerprint hasher shared across report-identity
//! computations. `finish` returns the raw digest; sites with nonzero-digest
//! contracts apply their own sentinel normalization on top.

pub struct Fnv1a(u64);

impl Fnv1a {
    /// The offset basis a fresh digest starts from.
    pub const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    /// Start a digest at the FNV offset basis.
    pub const fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    /// Resume a digest from a prior `finish` value (chained fingerprints).
    pub const fn resume(state: u64) -> Self {
        Self(state)
    }

    pub fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    pub fn flag(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.byte(*byte);
        }
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    /// NUL-terminated string feed.
    pub fn string_terminated(&mut self, value: &str) {
        self.bytes(value.as_bytes());
        self.byte(0);
    }

    /// Length-framed string feed: a u64 word prefix then the raw bytes.
    pub fn string_framed(&mut self, value: &str) {
        self.u64(value.len() as u64);
        self.bytes(value.as_bytes());
    }

    /// The raw digest. Sites that reserve a zero sentinel normalize on top.
    pub const fn finish(self) -> u64 {
        self.0
    }
}
