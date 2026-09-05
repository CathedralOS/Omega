use crate::{
    AllocationLegalityError, LiveRangeError, LivenessError, ValidatedAllocationLegality,
    ValidatedLiveRanges, ValidatedLiveness,
};

use crate::{OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopies};

/// Complete mandatory reanalysis of one independently validated transformed
/// selected CFG. No source analysis fact is reused after the rewrite.
#[derive(Debug)]
pub struct StagedOptimizedSelectedReanalysis {
    pub(super) transformation: StagedOptimizedFixedViewCopies,
    pub(super) liveness: ValidatedLiveness,
    pub(super) ranges: ValidatedLiveRanges,
    pub(super) legality: ValidatedAllocationLegality,
    pub(super) custody: SelectedReanalysisCustodyReceipt,
}

impl StagedOptimizedSelectedReanalysis {
    pub const fn transformation_stage(&self) -> &StagedOptimizedFixedViewCopies {
        &self.transformation
    }
    pub const fn liveness(&self) -> &ValidatedLiveness {
        &self.liveness
    }
    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        &self.ranges
    }
    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        &self.legality
    }
    pub const fn custody(&self) -> SelectedReanalysisCustodyReceipt {
        self.custody
    }
}

pub use register_homes::SelectedReanalysisCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedSelectedReanalysisError {
    UpstreamTransformation(OptimizedFixedViewCopyCustodyError),
    Liveness(LivenessError),
    LivenessRevalidation(LivenessError),
    LiveRanges(LiveRangeError),
    LiveRangeRevalidation(LiveRangeError),
    AllocationLegality(AllocationLegalityError),
    AllocationLegalityRevalidation(AllocationLegalityError),
    RemainingTransitions { count: usize },
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedSelectedReanalysisError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized transformed-selected reanalysis failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedSelectedReanalysisError {}
