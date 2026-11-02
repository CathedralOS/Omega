//! FNV-1a fingerprint primitives shared by the report-only revision
//! fingerprints. The exact replays are the authority; these hashes are cheap
//! staleness gates and diagnostic coordinates, never identity.

/// The FNV-1a offset basis every report fingerprint seeds from.
pub(crate) const OFFSET_BASIS: u64 = semantic_vocabulary::Fnv1a::OFFSET_BASIS;

pub(crate) fn byte(hash: &mut u64, value: u8) {
    *hash ^= u64::from(value);
    *hash = hash.wrapping_mul(0x100000001b3);
}
pub(crate) fn bytes(hash: &mut u64, value: &[u8]) {
    for value in value {
        byte(hash, *value);
    }
}
pub(crate) fn uint(hash: &mut u64, value: u64) {
    bytes(hash, &value.to_le_bytes());
}
pub(crate) fn text(hash: &mut u64, value: &str) {
    uint(hash, value.len() as u64);
    bytes(hash, value.as_bytes());
}
pub(crate) fn optional_text(hash: &mut u64, value: &Option<String>) {
    match value {
        Some(value) => {
            byte(hash, 1);
            text(hash, value);
        }
        None => byte(hash, 0),
    }
}
pub(crate) fn optional_uint(hash: &mut u64, value: Option<u64>) {
    match value {
        Some(value) => {
            byte(hash, 1);
            uint(hash, value);
        }
        None => byte(hash, 0),
    }
}

/// One query authority: its scope tag then the requester's identity.
pub(super) fn query_authority(hash: &mut u64, authority: &super::SchemaQueryAuthority) {
    match authority {
        super::SchemaQueryAuthority::OwningScope { requester_identity } => {
            byte(hash, 0);
            text(hash, requester_identity);
        }
        super::SchemaQueryAuthority::ForeignScope { requester_identity } => {
            byte(hash, 1);
            text(hash, requester_identity);
        }
    }
}
