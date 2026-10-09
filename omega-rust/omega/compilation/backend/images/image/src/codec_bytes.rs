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
