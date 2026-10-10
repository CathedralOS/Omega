//! Shared wire-encoding primitives for the crate's record encoders: the
//! append-half of `cursor.rs`. Each helper appends one already-decided wire
//! value; framing and field order stay with the encoder.

/// Append a fixed-width little-endian encoding (`value.to_le_bytes()`).
pub(crate) fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
    buffer.extend_from_slice(&encoded);
}

/// Append the standard record header: magic bytes then the u32 LE version.
pub(crate) fn encode_header(buffer: &mut Vec<u8>, magic: &[u8; 8], version: u32) {
    buffer.extend_from_slice(magic);
    push_encoded(buffer, version.to_le_bytes());
}

/// Consume the standard record header: exact magic then matching version,
/// each miss mapped onto the decoder's own error kind.
pub(crate) fn decode_header<E>(
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
