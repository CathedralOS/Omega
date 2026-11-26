//! Length-framed byte encoding shared by the identities retained with a Build evaluation.

pub fn push_framed(out: &mut Vec<u8>, bytes: &[u8]) {
    crate::codec_bytes::push_encoded(out, (bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}
