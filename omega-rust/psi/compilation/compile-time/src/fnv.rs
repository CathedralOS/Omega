//! FNV-1a fingerprint primitives shared by the report-only revision
//! fingerprints. The exact replays are the authority; these hashes are cheap
//! staleness gates and diagnostic coordinates, never identity.

/// The FNV-1a offset basis every report fingerprint seeds from.
pub(crate) const OFFSET_BASIS: u64 = semantic_vocabulary::Fnv1a::OFFSET_BASIS;

pub(crate) fn byte(hash: &mut u64, value: u8) {
    *hash ^= u64::from(value);
    *hash = hash.wrapping_mul(0x100000001b3);
}
