//! Incremental FNV-1a over caller-supplied bytes and field framing.

#[derive(Clone, Copy)]
pub struct Fnv1a(u64);

impl Fnv1a {
    pub const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    pub const PRIME: u64 = 0x100000001b3;

    pub const fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

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

    pub fn bytes_terminated(&mut self, bytes: &[u8], terminator: u8) {
        self.bytes(bytes);
        self.byte(terminator);
    }

    pub fn bytes_framed(&mut self, bytes: &[u8]) {
        self.u64(bytes.len() as u64);
        self.bytes(bytes);
    }

    pub fn string_terminated(&mut self, value: &str) {
        self.bytes(value.as_bytes());
        self.byte(0);
    }

    pub fn string_framed(&mut self, value: &str) {
        self.u64(value.len() as u64);
        self.bytes(value.as_bytes());
    }

    pub fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    pub const fn finish(self) -> u64 {
        self.0
    }

    pub const fn finish_or(self, fallback: u64) -> u64 {
        if self.0 == 0 { fallback } else { self.0 }
    }
}

impl Default for Fnv1a {
    fn default() -> Self {
        Self::new()
    }
}
