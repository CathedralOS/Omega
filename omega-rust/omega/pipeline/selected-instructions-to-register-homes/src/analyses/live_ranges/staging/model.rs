use crate::{LiveRangeError, ValidatedLiveRanges};

use crate::{OptimizedLivenessCustodyError, StagedOptimizedLiveness};

/// Opt-in CFG-aware live-range staging over complete liveness custody. This
/// grants no splitting, allocation, spill, frame, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveRanges {
    pub(super) liveness: StagedOptimizedLiveness,
    pub(super) ranges: ValidatedLiveRanges,
    pub(super) custody: LiveRangeCustodyReceipt,
}

impl StagedOptimizedLiveRanges {
    pub const fn liveness_stage(&self) -> &StagedOptimizedLiveness {
        &self.liveness
    }

    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        &self.ranges
    }

    pub const fn custody(&self) -> LiveRangeCustodyReceipt {
        self.custody
    }
}

pub use register_homes::LiveRangeCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLiveRangeCustodyError {
    UpstreamLiveness(OptimizedLivenessCustodyError),
    Analysis(LiveRangeError),
    Revalidation(LiveRangeError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedLiveRangeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized live-range staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedLiveRangeCustodyError {}
