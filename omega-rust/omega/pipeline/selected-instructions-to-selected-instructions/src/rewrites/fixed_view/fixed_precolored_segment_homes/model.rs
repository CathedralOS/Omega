use crate::{
    FixedPrecoloredIntervalError, FixedPrecoloredIntervalValidationReceipt,
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomeValidationReceipt,
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementValidationReceipt,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
    ValidatedFixedPrecoloredSplitRequirements,
};

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt,
};

#[derive(Debug)]
pub struct StagedOptimizedFixedPrecoloredSegmentHomes {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) fixed: ValidatedFixedPrecoloredIntervals,
    pub(super) requirements: ValidatedFixedPrecoloredSplitRequirements,
    pub(super) homes: ValidatedFixedPrecoloredSegmentHomes,
    pub(super) custody: StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
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
    pub const fn custody(&self) -> StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
    pub(super) upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
    pub(super) fixed: FixedPrecoloredIntervalValidationReceipt,
    pub(super) requirements: FixedPrecoloredSplitRequirementValidationReceipt,
    pub(super) homes: FixedPrecoloredSegmentHomeValidationReceipt,
}

impl StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
    pub const fn upstream(self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.upstream
    }
    pub const fn fixed(self) -> FixedPrecoloredIntervalValidationReceipt {
        self.fixed
    }
    pub const fn requirements(self) -> FixedPrecoloredSplitRequirementValidationReceipt {
        self.requirements
    }
    pub const fn homes(self) -> FixedPrecoloredSegmentHomeValidationReceipt {
        self.homes
    }
}

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
