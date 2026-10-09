//! Little-endian append encoders shared by the image backends' section
//! writers: positional byte emission without bounds checking, since every
//! caller appends into a buffer it sizes itself.

pub fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend(value.to_le_bytes());
}

pub fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

pub fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}

/// Write a fixed-width little-endian encoding at `offset` inside `bytes`.
pub(crate) fn patch_encoded<const N: usize>(bytes: &mut [u8], offset: usize, encoded: [u8; N]) {
    bytes[offset..offset + N].copy_from_slice(&encoded);
}
