//! Shared wire-encoding primitives for the crate's record encoders: the
//! append-half of [`optimization_core::cursor`]. Each helper appends one
//! already-decided wire value; framing and field order stay with the encoder.

#[cfg(test)]
pub(crate) use optimization_core::codec_bytes::patch_encoded;
pub(crate) use optimization_core::codec_bytes::push_encoded;

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
