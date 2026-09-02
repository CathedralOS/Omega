use crate::{
    FixedPrecoloredIntervalPlanIdentity, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedViewCopyDecodeError,
    FixedViewCopySourceEvidence,
};

use super::primitives::Cursor;

pub(super) fn encode(bytes: &mut Vec<u8>, evidence: FixedViewCopySourceEvidence) {
    match evidence {
        FixedViewCopySourceEvidence::LegacyLegalityTransitionsV1 => bytes.push(0),
        FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
            fixed_intervals,
            split_requirements,
            segment_homes,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&fixed_intervals.bytes());
            bytes.extend_from_slice(&split_requirements.bytes());
            bytes.extend_from_slice(&segment_homes.bytes());
        }
    }
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
) -> Result<FixedViewCopySourceEvidence, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(FixedViewCopySourceEvidence::LegacyLegalityTransitionsV1),
        1 => Ok(FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
            fixed_intervals: FixedPrecoloredIntervalPlanIdentity::from_bytes(cursor.array()?),
            split_requirements: FixedPrecoloredSplitRequirementPlanIdentity::from_bytes(
                cursor.array()?,
            ),
            segment_homes: FixedPrecoloredSegmentHomePlanIdentity::from_bytes(cursor.array()?),
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownSourceEvidence(tag)),
    }
}
