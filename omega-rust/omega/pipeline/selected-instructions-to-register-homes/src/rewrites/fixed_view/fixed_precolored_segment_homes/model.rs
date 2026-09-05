use crate::{
    FixedPrecoloredIntervalError, FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSplitRequirementError, ValidatedFixedPrecoloredIntervals,
    ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
};

use crate::{OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality};

#[derive(Debug)]
pub struct StagedOptimizedFixedPrecoloredSegmentHomes {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) fixed: ValidatedFixedPrecoloredIntervals,
    pub(super) requirements: ValidatedFixedPrecoloredSplitRequirements,
    pub(super) homes: ValidatedFixedPrecoloredSegmentHomes,
    pub(super) custody: FixedPrecoloredSegmentHomeCustodyReceipt,
}

impl StagedOptimizedFixedPrecoloredSegmentHomes {
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.source
    }
    pub const fn fixed_intervals(&self) -> &ValidatedFixedPrecoloredIntervals {
        &self.fixed
    }
    pub const fn split_requirements(&self) -> &ValidatedFixedPrecoloredSplitRequirements {
        &self.requirements
    }
    pub const fn segment_homes(&self) -> &ValidatedFixedPrecoloredSegmentHomes {
        &self.homes
    }
    pub const fn custody(&self) -> FixedPrecoloredSegmentHomeCustodyReceipt {
        self.custody
    }
}

pub use register_homes::FixedPrecoloredSegmentHomeCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedFixedPrecoloredSegmentHomeCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
    FixedIntervals(FixedPrecoloredIntervalError),
    SplitRequirements(FixedPrecoloredSplitRequirementError),
    SegmentHomes(FixedPrecoloredSegmentHomeError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedFixedPrecoloredSegmentHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "fixed/precolored segment-home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedFixedPrecoloredSegmentHomeCustodyError {}
