//! Fast hashing for the compiler's integer-keyed maps.
//!
//! `std::collections::HashMap` defaults to SipHash, whose per-call state build
//! is measurable across the program-scoped index maps (symbol->entry rosters,
//! per-state collections, operational memos) hit once per demand site. The
//! keys are already well-mixed handles, so an FxHash-style multiply-rotate
//! keeps lookup cheap without an external dependency.
//!
//! `SymbolMap<V>` covers the common `SymbolHandle -> V` shape; `SymbolKeyMap`
//! covers tuple keys over handles and indexes.

use core::hash::{BuildHasher, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default)]
pub struct SymbolHasher(u64);

impl SymbolHasher {
    #[inline]
    fn add(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(SEED);
    }
}

impl Hasher for SymbolHasher {
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
pub struct BuildSymbolHasher;

impl BuildHasher for BuildSymbolHasher {
    type Hasher = SymbolHasher;

    #[inline]
    fn build_hasher(&self) -> SymbolHasher {
        SymbolHasher(0)
    }
}

/// `SymbolHandle -> V` map with fast integer hashing.
pub type SymbolMap<V> = std::collections::HashMap<crate::SymbolHandle, V, BuildSymbolHasher>;

/// `K -> V` map with fast integer hashing for handle/index tuple keys.
pub type SymbolKeyMap<K, V> = std::collections::HashMap<K, V, BuildSymbolHasher>;
