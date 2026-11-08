//! Shared canonical digest framing and lowercase hexadecimal encoding.

pub(crate) fn append_framed_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

pub(crate) fn format_sha256(bytes: &[u8]) -> String {
    super::encode_hex(bytes)
}

pub(crate) fn format_hex(bytes: &[u8]) -> String {
    super::encode_hex(bytes)
}
