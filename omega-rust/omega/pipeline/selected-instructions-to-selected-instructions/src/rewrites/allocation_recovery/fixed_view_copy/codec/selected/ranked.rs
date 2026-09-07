//! Canonical ranked proof/fuel/frontier evidence, never an executable body.
use crate::FixedViewCopyDecodeError;
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{Cursor, length};
use abstract_operations::{
    RankedU32CountdownCustody, decode_ranked_u32_countdown_custody,
    encode_ranked_u32_countdown_custody,
};

pub(super) fn encode(bytes: &mut Vec<u8>, custody: Option<&RankedU32CountdownCustody>) {
    match custody {
        None => bytes.push(0),
        Some(custody) => match encode_ranked_u32_countdown_custody(custody) {
            Ok(content) => {
                bytes.push(1);
                length(bytes, content.len());
                bytes.extend_from_slice(&content);
            }
            // Preserve an explicit invalid-proposal state in this infallible raw encoder.
            Err(_) => bytes.push(2),
        },
    }
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
) -> Result<Option<RankedU32CountdownCustody>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => {
            let count = cursor.length()?;
            decode_ranked_u32_countdown_custody(cursor.take(count)?)
                .map(Some)
                .map_err(|_| FixedViewCopyDecodeError::InvalidRankedCustody)
        }
        2 => Err(FixedViewCopyDecodeError::InvalidRankedCustody),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}
