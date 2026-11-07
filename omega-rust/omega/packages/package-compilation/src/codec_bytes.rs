// Shared wire-encoding primitives for record encoders in `package-compilation`:
// the append-half of `cursor.rs`. Each helper appends one already-decided wire
// value (`u64` little-endian or length-prefixed byte slice) while framing and
// field order remain with the caller.

pub(crate) use optimization_core::codec_bytes::push_encoded;

pub(crate) fn push_u64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn push_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) {
    push_u64(buffer, bytes.len() as u64);
    buffer.extend_from_slice(bytes);
}
