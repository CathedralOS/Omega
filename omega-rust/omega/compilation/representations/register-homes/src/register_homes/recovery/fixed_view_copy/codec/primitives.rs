pub(super) use crate::cursor::Cursor;

use crate::FixedViewCopyDecodeError;

pub(super) fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    encode_option(bytes, value, |bytes, value| {
        bytes.extend_from_slice(&value.to_le_bytes());
    });
}

pub(super) fn decode_option_u64(
    cursor: &mut Cursor<'_>,
) -> Result<Option<u64>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.u64()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

pub(super) fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    encode_option(bytes, value, |bytes, value| {
        bytes.extend_from_slice(&value.to_le_bytes());
    });
}

pub(super) fn decode_option_u16(
    cursor: &mut Cursor<'_>,
) -> Result<Option<u16>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.u16()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

fn encode_option<T>(bytes: &mut Vec<u8>, value: Option<T>, encode: impl FnOnce(&mut Vec<u8>, T)) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            encode(bytes, value);
        }
    }
}

pub(super) fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    length(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

pub(super) fn decode_list<T>(
    cursor: &mut Cursor<'_>,
    mut decode: impl FnMut(&mut Cursor<'_>) -> Result<T, FixedViewCopyDecodeError>,
) -> Result<Vec<T>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(decode(cursor)?);
    }
    Ok(values)
}

pub(super) fn decode_ids<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<Vec<T>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(decode_id(cursor, constructor)?);
    }
    Ok(values)
}

pub(super) fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    length(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

pub(super) fn decode_u16s(cursor: &mut Cursor<'_>) -> Result<Vec<u16>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(cursor.u16()?);
    }
    Ok(values)
}

pub(super) fn decode_id<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<T, FixedViewCopyDecodeError> {
    let raw = cursor.u64()?;
    constructor(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))
}

/// The 32-byte accepted-obligation receipt paired beside every obligation id
/// in the fixed-view artifact.
pub(super) fn decode_accepted_fact(
    cursor: &mut Cursor<'_>,
) -> Result<optimization_core::AcceptedObligationFactIdentity, FixedViewCopyDecodeError> {
    Ok(optimization_core::AcceptedObligationFactIdentity::from_bytes(cursor.array()?))
}

pub(super) fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("fixed-view-copy artifact length fits u64")
            .to_le_bytes(),
    );
}
