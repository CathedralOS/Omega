//! Shared wire-encoding primitives for the workspace's record encoders: the
//! append-half and the fixed-width read-back of `cursor.rs`. Each helper moves
//! one already-decided wire value; framing and field order stay with the
//! encoder. Crates that once carried a copy of these functions import this
//! module instead.

/// Append a fixed-width little-endian encoding (`value.to_le_bytes()`).
pub fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
    buffer.extend_from_slice(&encoded);
}

/// Write a fixed-width little-endian encoding at `offset` inside `bytes`.
pub fn patch_encoded<const N: usize>(bytes: &mut [u8], offset: usize, encoded: [u8; N]) {
    bytes[offset..offset + N].copy_from_slice(&encoded);
}

/// Read a little-endian u32 at `offset`, bounded.
pub fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}
