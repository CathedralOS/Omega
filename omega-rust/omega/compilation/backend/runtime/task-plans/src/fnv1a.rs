//! Shared FNV-1a-64 hasher for the crate's non-authoritative report
//! fingerprints; the canonical implementation lives in
//! `semantic_vocabulary`. Zero digests stay a site concern: callers
//! finish with `finish_or(Fnv1a::OFFSET_BASIS)`.

pub(crate) use semantic_vocabulary::Fnv1a;
