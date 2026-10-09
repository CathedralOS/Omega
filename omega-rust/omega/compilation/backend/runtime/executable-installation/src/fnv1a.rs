//! Shared FNV-1a-64 hasher for the crate's non-authoritative fingerprints.
//! Zero sentinels stay with each caller's normalization contract.

pub(crate) const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) struct Fnv1a(u64);

impl Fnv1a {
    pub(crate) const fn new() -> Self {
        Self(OFFSET_BASIS)
    }

    /// Whole-word XOR-mix feed for slot fingerprints.
    pub(crate) fn u64_mix(&mut self, value: u64) {
        self.0 ^= value;
        self.0 = self.0.wrapping_mul(PRIME);
    }

    pub(crate) const fn finish(self) -> u64 {
        self.0
    }
}
