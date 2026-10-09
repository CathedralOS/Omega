//! Shared byte-append primitives for the crate's canonical encoders: u64
//! length framing, option tags, and length-prefixed element lists. Each call
//! site names its own artifact so panic text stays authored at the domain.

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
