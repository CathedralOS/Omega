//! Length-framed FNV primitives for report-only ELF compatibility fingerprints.
//! Callers retain their own domain tags, field encodings and ordering. A slice
//! includes its eight-byte little-endian length; a single byte has no prefix.

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub(super) struct Fnv1a(u64);

impl Fnv1a {
    pub(super) const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    pub(super) fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    pub(super) const fn finish(self) -> u64 {
        self.0
    }
}
