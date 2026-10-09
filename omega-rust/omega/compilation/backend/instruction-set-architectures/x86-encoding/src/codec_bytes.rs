//! Shared wire-encoding primitives for the crate's record encoders: the
//! append-half of `cursor.rs`. Each helper appends one already-decided wire
//! value; framing and field order stay with the encoder.

/// Append a u64 little-endian value.
pub(crate) fn push_u64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend_from_slice(&value.to_le_bytes());
}
