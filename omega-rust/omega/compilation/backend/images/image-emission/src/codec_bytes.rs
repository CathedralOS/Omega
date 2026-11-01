//! Shared wire-encoding primitives for the crate's artifact replay encoders: the
//! append-half of `cursor.rs`. Each helper appends one already-decided wire
//! value; framing and field order stay with the encoder.

pub(crate) use optimization_core::codec_bytes::u32_at;

/// Read a u16 little-endian value at `offset`, or None when out of range.
pub(crate) fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

/// Read a u64 little-endian value at `offset`, or None when out of range.
pub(crate) fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}
