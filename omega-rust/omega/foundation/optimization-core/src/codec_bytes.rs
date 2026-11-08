// Shared fixed-width wire-encoding helpers (`push_encoded`, `patch_encoded`,
// `push_count_u32`, `push_count_u64`, `u32_at`) paired with `cursor.rs`.
//
// Wire records frame counted sequence lengths as `u32`, while canonical
// identity rows frame them as `u64`; sequences exceeding their declared count
// width fail closed rather than truncating.

pub fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
    buffer.extend_from_slice(&encoded);
}

pub fn patch_encoded<const N: usize>(bytes: &mut [u8], offset: usize, encoded: [u8; N]) {
    bytes[offset..offset + N].copy_from_slice(&encoded);
}

pub fn push_count_u32(buffer: &mut Vec<u8>, count: usize) {
    let count = u32::try_from(count).expect("counted row fits u32");
    push_encoded(buffer, count.to_le_bytes());
}

pub fn push_count_u64(buffer: &mut Vec<u8>, count: usize) {
    let count = u64::try_from(count).expect("counted row fits u64");
    push_encoded(buffer, count.to_le_bytes());
}

pub fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}
