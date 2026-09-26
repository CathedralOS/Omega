pub(super) fn write_string(bytes: &mut Vec<u8>, value: &str) {
    write_u32(
        bytes,
        u32::try_from(value.len()).expect("object string length overflow"),
    );
    bytes.extend(value.as_bytes());
}

pub(super) fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

pub(super) fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}

pub(super) fn write_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend(value.to_le_bytes());
}
