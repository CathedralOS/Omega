//! Current-wire custody for parameter-rooted structural qualification paths.

use terminal_psi::StructuralPathQualification;

use crate::FixedViewCopyDecodeError;

use super::declarations::decode_qualification_path;
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_list, length,
};
use selected_instructions::effects::program::identity::encode_path;

pub(super) fn encode_projected(
    bytes: &mut Vec<u8>,
    qualifications: &[StructuralPathQualification],
    retain: bool,
) {
    if !retain {
        return;
    }
    length(bytes, qualifications.len());
    for qualification in qualifications {
        encode_path(bytes, &qualification.path);
        crate::codec_bytes::push_encoded(bytes, qualification.domain.get().to_le_bytes());
    }
}

pub(super) fn decode_projected(
    cursor: &mut Cursor<'_>,
    retain: bool,
) -> Result<Vec<StructuralPathQualification>, FixedViewCopyDecodeError> {
    if !retain {
        return Ok(Vec::new());
    }
    decode_list(cursor, |cursor| {
        Ok(StructuralPathQualification {
            path: decode_qualification_path(cursor)?,
            domain: decode_id(cursor, semantic_vocabulary::StructuralDomainId::new)?,
        })
    })
}
