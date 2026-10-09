//! Shared wire-encoding primitives for the crate's record encoders: the
//! append-half of `cursor.rs`. Each helper appends one already-decided wire
//! value; framing and field order stay with the encoder.

/// Append a fixed-width little-endian encoding (`value.to_le_bytes()`).
pub(crate) fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
    buffer.extend_from_slice(&encoded);
}

/// Write a fixed-width little-endian encoding at `offset` inside `bytes`.
pub(crate) fn patch_encoded<const N: usize>(bytes: &mut [u8], offset: usize, encoded: [u8; N]) {
    bytes[offset..offset + N].copy_from_slice(&encoded);
}

/// Append the canonical u64-le collection length tag.
pub(crate) fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    push_encoded(bytes, (length as u64).to_le_bytes());
}

/// Append a length-prefixed list of little-endian encodings.
pub(crate) fn encode_le_list<T, const N: usize>(
    bytes: &mut Vec<u8>,
    values: &[T],
    to_le: impl Fn(&T) -> [u8; N],
) {
    encode_len(bytes, values.len());
    for value in values {
        push_encoded(bytes, to_le(value));
    }
}
