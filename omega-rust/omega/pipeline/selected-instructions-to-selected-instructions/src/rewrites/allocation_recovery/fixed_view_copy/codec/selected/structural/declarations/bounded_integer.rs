//! Exact fixed integer carrier and inclusive interval payload.
use super::{Cursor, FixedViewCopyDecodeError, decode_scalar, encode_scalar};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::values::{
    decode_integer, encode_integer,
};
use semantic_vocabulary::{BoundedIntegerType, ScalarType};

pub(super) fn encode(bytes: &mut Vec<u8>, bounds: BoundedIntegerType) {
    encode_scalar(bytes, ScalarType::Integer(bounds.integer_type()));
    encode_integer(bytes, bounds.minimum());
    encode_integer(bytes, bounds.maximum());
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
) -> Result<BoundedIntegerType, FixedViewCopyDecodeError> {
    let ScalarType::Integer(integer) = decode_scalar(cursor)? else {
        return Err(FixedViewCopyDecodeError::UnknownStructuralFieldType(6));
    };
    let minimum = decode_integer(cursor)?;
    let maximum = decode_integer(cursor)?;
    BoundedIntegerType::new(integer, minimum, maximum)
        .map_err(|_| FixedViewCopyDecodeError::UnknownStructuralFieldType(6))
}
