//! Shared wire-encoding primitives for the crate's record encoders: the
//! append-half of `cursor.rs`. Each helper appends one already-decided wire
//! value; framing and field order stay with the encoder.

/// Append a fixed-width little-endian encoding (`value.to_le_bytes()`).
pub(crate) fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
    buffer.extend_from_slice(&encoded);
}

/// Write a fixed-width little-endian encoding at `offset` inside `bytes`.

/// Write a fixed-width little-endian encoding at `offset` inside `bytes`.
pub(crate) fn patch_encoded<const N: usize>(bytes: &mut [u8], offset: usize, encoded: [u8; N]) {
    bytes[offset..offset + N].copy_from_slice(&encoded);
}

/// The manifest envelope shared by publication codecs: magic + version +
/// identity + framed content, in that order.
pub(crate) fn encode_manifest_envelope(
    magic: &[u8],
    version: u32,
    identity: &[u8],
    content: &[u8],
) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(magic);
    push_encoded(&mut encoded, version.to_le_bytes());
    encoded.extend_from_slice(identity);
    encoded.extend_from_slice(content);
    encoded
}
