// Shared fixed-width wire-encoding helpers (`push_encoded`, `patch_encoded`,
// `push_count_u32`, `push_count_u64`, `u32_at`) paired with `cursor.rs`.
//
// Wire records frame counted sequence lengths as `u32`, while canonical
// identity rows frame them as `u64`; sequences exceeding their declared count
// width fail closed rather than truncating.

pub fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
    buffer.extend_from_slice(&encoded);
}

pub fn patch_encoded<const N: usize>(bytes: &mut [u8], offset: usize, encoded: [u8; N]) {
    bytes[offset..offset + N].copy_from_slice(&encoded);
}

pub fn push_count_u32(buffer: &mut Vec<u8>, count: usize) {
    let count = u32::try_from(count).expect("counted row fits u32");
    push_encoded(buffer, count.to_le_bytes());
}

pub fn push_count_u64(buffer: &mut Vec<u8>, count: usize) {
    let count = u64::try_from(count).expect("counted row fits u64");
    push_encoded(buffer, count.to_le_bytes());
}

pub fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

/// Encode an optional unsigned value using a one-byte presence tag.
pub fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_encoded(bytes, value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

pub fn encode_header(buffer: &mut Vec<u8>, magic: &[u8; 8], version: u32) {
    buffer.extend_from_slice(magic);
    push_encoded(buffer, version.to_le_bytes());
}

pub fn decode_header<E>(
    cursor: &mut crate::cursor::Cursor<'_>,
    magic: &[u8; 8],
    expected_version: u32,
    wrong_magic: impl FnOnce() -> E,
    unsupported_version: impl FnOnce(u32) -> E,
) -> Result<(), E>
where
    E: From<crate::cursor::Truncated> + From<crate::cursor::TakeError>,
{
    if cursor.take(magic.len())? != magic.as_slice() {
        return Err(wrong_magic());
    }
    let version = cursor.u32()?;
    if version != expected_version {
        return Err(unsupported_version(version));
    }
    Ok(())
}
