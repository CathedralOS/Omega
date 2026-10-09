//! Shared byte-append primitives for the crate's canonical encoders: u64
//! length framing, option tags, and length-prefixed element lists. Each call
//! site names its own artifact so panic text stays authored at the domain.

/// Append a fixed-width little-endian encoding (value.to_le_bytes()).
pub(crate) fn push_encoded<const N: usize>(bytes: &mut Vec<u8>, encoded: [u8; N]) {
    bytes.extend_from_slice(&encoded);
}

/// u64 length prefix. `artifact` keeps each encoder's own panic text.
pub(crate) fn length(bytes: &mut Vec<u8>, value: usize, artifact: &'static str) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .unwrap_or_else(|_| panic!("{artifact} length fits u64"))
            .to_le_bytes(),
    );
}

/// 0/1 tag followed by the payload only when present.
pub(crate) fn option<T>(
    bytes: &mut Vec<u8>,
    value: Option<T>,
    encode: impl FnOnce(&mut Vec<u8>, T),
) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            encode(bytes, value);
        }
    }
}

/// Length prefix followed by each element in order.
pub(crate) fn list<'a, T: 'a>(
    bytes: &mut Vec<u8>,
    values: &'a [T],
    artifact: &'static str,
    mut encode: impl FnMut(&mut Vec<u8>, &'a T),
) {
    length(bytes, values.len(), artifact);
    for value in values {
        encode(bytes, value);
    }
}

/// Writes the shared artifact envelope header: `MAGIC[8] ++ VERSION_LE`.
pub(crate) fn encode_header(bytes: &mut Vec<u8>, magic: &[u8; 8], version: u32) {
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&version.to_le_bytes());
}

/// Reads the shared artifact envelope header from `cursor`: the magic must
/// match exactly and the version must be the expected one, or the decoder's
/// own wrong-magic / unsupported-version error is returned.
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

/// The shared canonical identity formula: `Sha256(domain-tag ++ content)`.
/// Every terminal artifact identity in this crate is this digest over its own
/// authored domain tag and canonical content bytes.
pub(crate) fn domain_identity(domain: &[u8], content: &[u8]) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
    digest.raw(content);
    digest.finish().into()
}
