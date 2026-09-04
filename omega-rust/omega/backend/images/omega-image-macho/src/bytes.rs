//! Little-endian appends for the file, and big-endian ones for the code signature,
//! which is the one structure here that is not little-endian.

pub(super) fn write_be_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_be_bytes());
}

pub(super) fn write_be_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_be_bytes());
}

pub(super) fn write_fixed_string_16(bytes: &mut Vec<u8>, value: &str) {
    let value_bytes = value.as_bytes();
    assert!(
        value_bytes.len() <= 16,
        "fixed Mach-O string is longer than 16 bytes"
    );
    bytes.extend(value_bytes);
    bytes.resize(bytes.len() + (16 - value_bytes.len()), 0);
}

pub(super) fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

pub(super) fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}
