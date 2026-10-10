//! Shared FNV-1a-64 hasher for the crate's non-authoritative fingerprints;
//! the canonical implementation lives in `semantic_vocabulary`.

pub(crate) use semantic_vocabulary::Fnv1a;
pub(crate) const OFFSET_BASIS: u64 = Fnv1a::OFFSET_BASIS;
