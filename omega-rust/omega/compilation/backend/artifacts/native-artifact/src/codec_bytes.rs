//! Shared wire-encoding primitives for the crate's wrapper-object codec encoders: the
//! append-half of `cursor.rs`. Each helper appends one already-decided wire
//! value; framing and field order stay with the encoder.

/// Append a fixed-width little-endian encoding (`value.to_le_bytes()`).
pub(crate) fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
    buffer.extend_from_slice(&encoded);
}

/// Append a u64 little-endian value.
pub(crate) fn push_u64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

/// Append a length-prefixed byte field (u64 little-endian length + bytes).
pub(crate) fn push_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) {
    push_u64(buffer, bytes.len() as u64);
    buffer.extend_from_slice(bytes);
}
