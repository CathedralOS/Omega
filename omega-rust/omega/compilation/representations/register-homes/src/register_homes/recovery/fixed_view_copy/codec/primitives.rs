pub(super) use crate::cursor::Cursor;

use crate::FixedViewCopyDecodeError;

pub(super) fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    encode_option(bytes, value, |bytes, value| {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
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
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
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
    crate::codec_bytes::option(bytes, value, encode);
}

pub(super) fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    encode_le_list(bytes, values, |value| value.to_le_bytes());
}

fn encode_le_list<T, const N: usize>(
    bytes: &mut Vec<u8>,
    values: impl ExactSizeIterator<Item = T>,
    to_le: impl Fn(T) -> [u8; N],
) {
    length(bytes, values.len());
    for value in values {
        crate::codec_bytes::push_encoded(bytes, to_le(value));
    }
}

pub(super) use crate::codec_bytes::decode_list;

pub(super) fn decode_ids<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<Vec<T>, FixedViewCopyDecodeError> {
    decode_list(cursor, |cursor| decode_id(cursor, constructor))
}

pub(super) fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    encode_le_list(bytes, values, |value| value.to_le_bytes());
}

pub(super) fn decode_u16s(cursor: &mut Cursor<'_>) -> Result<Vec<u16>, FixedViewCopyDecodeError> {
    decode_list(cursor, |cursor| Ok(cursor.u16()?))
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
    crate::codec_bytes::length(bytes, value, "fixed-view-copy artifact");
}
