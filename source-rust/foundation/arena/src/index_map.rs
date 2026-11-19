//! Hash storage for integer keys, arena handles and tuples of those coordinates.

use core::hash::{BuildHasher, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default)]
pub struct IndexHasher(u64);

impl IndexHasher {
    #[inline]
    fn add(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(SEED);
    }
}

impl Hasher for IndexHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.add(byte as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.add(value as u64);
    }

    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.add(value as u64);
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.add(value as u64);
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    #[inline]
    fn write_u128(&mut self, value: u128) {
        self.add(value as u64);
        self.add((value >> 64) as u64);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}

#[derive(Clone, Default)]
pub struct BuildIndexHasher;

impl BuildHasher for BuildIndexHasher {
    type Hasher = IndexHasher;

    #[inline]
    fn build_hasher(&self) -> IndexHasher {
        IndexHasher(0)
    }
}

/// A keyed index over handles or their integer coordinates.
pub type HandleKeyMap<K, V> = std::collections::HashMap<K, V, BuildIndexHasher>;
