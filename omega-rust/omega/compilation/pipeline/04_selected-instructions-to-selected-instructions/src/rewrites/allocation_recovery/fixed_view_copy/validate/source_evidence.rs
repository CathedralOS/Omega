use crate::{
    FixedViewCopyError, FixedViewCopySourceEvidence, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
    ValidatedFixedPrecoloredSplitRequirements, ValidatedLiveRanges,
    rewrites::allocation_recovery::fixed_view_copy::evidence::FixedViewBoundaryEvidence,
};

pub(super) fn reconstruct(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
    actual: FixedViewCopySourceEvidence,
) -> Result<FixedViewBoundaryEvidence, FixedViewCopyError> {
    if actual == FixedViewCopySourceEvidence::LegacyLegalityTransitionsV1 {
        return Err(FixedViewCopyError::LegacySourceEvidence);
    }
    let expected = FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
        fixed_intervals: fixed.receipt().identity(),
        split_requirements: requirements.receipt().identity(),
        segment_homes: homes.receipt().identity(),
    };
    if actual != expected {
        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
    }
    super::super::evidence::reconstruct_by_key(ranges, legality, fixed, requirements, homes)
}
