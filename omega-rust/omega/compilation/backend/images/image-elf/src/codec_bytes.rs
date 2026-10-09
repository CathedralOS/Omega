//! Shared wire-encoding primitives for the crate's record encoders: the
//! append-half of `cursor.rs`. Each helper appends one already-decided wire
//! value; framing and field order stay with the encoder.

/// Write a fixed-width little-endian encoding at `offset` inside `bytes`.

/// Write a fixed-width little-endian encoding at `offset` inside `bytes`.
pub(crate) fn patch_encoded<const N: usize>(bytes: &mut [u8], offset: usize, encoded: [u8; N]) {
    bytes[offset..offset + N].copy_from_slice(&encoded);
}
