//! FNV-1a fingerprint primitives shared by the report-only revision
//! fingerprints. The exact replays are the authority; these hashes are cheap
//! staleness gates and diagnostic coordinates, never identity.

pub(super) fn byte(hash: &mut u64, value: u8) {
    *hash ^= u64::from(value);
    *hash = hash.wrapping_mul(0x100000001b3);
}
pub(super) fn bytes(hash: &mut u64, value: &[u8]) {
    for value in value {
        byte(hash, *value);
    }
}
pub(super) fn uint(hash: &mut u64, value: u64) {
    bytes(hash, &value.to_le_bytes());
}
pub(super) fn text(hash: &mut u64, value: &str) {
    uint(hash, value.len() as u64);
    bytes(hash, value.as_bytes());
}
pub(super) fn optional_text(hash: &mut u64, value: &Option<String>) {
    match value {
        Some(value) => {
            byte(hash, 1);
            text(hash, value);
        }
        None => byte(hash, 0),
    }
}
