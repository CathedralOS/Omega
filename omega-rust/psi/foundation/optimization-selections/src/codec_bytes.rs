//! Shared wire-encoding primitives for the crate's record encoders: the
//! append-half of `cursor.rs`. Each helper appends one already-decided wire
//! value; framing and field order stay with the encoder.

/// Append a fixed-width little-endian encoding (`value.to_le_bytes()`).
pub(crate) fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
    buffer.extend_from_slice(&encoded);
}
