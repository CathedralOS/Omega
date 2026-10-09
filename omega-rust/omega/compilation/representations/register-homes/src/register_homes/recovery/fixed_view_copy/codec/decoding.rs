//! Current-version admission precedes payload interpretation and authentication.
use super::{
    FixedViewCopyDecodeError, FixedViewCopyPlan, MAGIC, VERSION, content, envelope::v57_identity,
    primitives::Cursor,
};
use selected_instructions::selected_instruction_plan_identity;
pub(super) fn decode(encoded: &[u8]) -> Result<FixedViewCopyPlan, FixedViewCopyDecodeError> {
    let mut cursor = Cursor::new(encoded);
    crate::codec_bytes::decode_header(
        &mut cursor,
        MAGIC,
        VERSION,
        || FixedViewCopyDecodeError::WrongMagic,
        FixedViewCopyDecodeError::UnsupportedVersion,
    )?;
    let identity: [u8; 32] = cursor.array()?;
    let content_offset = cursor.offset;
    let decoded = content::decode_v7(&mut cursor)?;
    if cursor.remaining() != 0 {
        return Err(FixedViewCopyDecodeError::TrailingBytes);
    }
    if !decoded.transformed_payload_matches {
        return Err(FixedViewCopyDecodeError::TransformedPayloadMismatch);
    }
    if selected_instruction_plan_identity(&decoded.plan.transformed) != decoded.expected_transformed
    {
        return Err(FixedViewCopyDecodeError::TransformedIdentityMismatch);
    }
    if v57_identity(&decoded.plan, &encoded[content_offset..]) != identity {
        return Err(FixedViewCopyDecodeError::IdentityMismatch);
    }
    Ok(decoded.plan)
}
