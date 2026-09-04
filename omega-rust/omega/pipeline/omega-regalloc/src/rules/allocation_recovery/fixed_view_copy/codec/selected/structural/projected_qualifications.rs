//! Current-wire custody for parameter-rooted structural qualification paths.

use psi_terminal::StructuralPathQualification;

use crate::FixedViewCopyDecodeError;

use super::declarations::{decode_path, encode_path};
use crate::rules::allocation_recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, length,
};

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
        bytes.extend_from_slice(&qualification.domain.get().to_le_bytes());
    }
}

pub(super) fn decode_projected(
    cursor: &mut Cursor<'_>,
    retain: bool,
) -> Result<Vec<StructuralPathQualification>, FixedViewCopyDecodeError> {
    if !retain {
        return Ok(Vec::new());
    }
    let count = cursor.length()?;
    let mut qualifications = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        qualifications.push(StructuralPathQualification {
            path: decode_path(cursor)?,
            domain: decode_id(cursor, psi_core::StructuralDomainId::new)?,
        });
    }
    Ok(qualifications)
}
