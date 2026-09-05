use crate::{
    AllocationLegalityError, AllocatorAvailabilityError, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability,
};

use crate::{OptimizedLiveRangeCustodyError, StagedOptimizedLiveRanges};

/// Opt-in physical-view legality staging over complete live-range custody.
/// It records exact candidates and required fixed-view transitions, but grants
/// no splitting, copy insertion, home assignment, emission, or publication.
#[derive(Debug)]
pub struct StagedOptimizedAllocationLegality {
    pub(super) ranges: StagedOptimizedLiveRanges,
    pub(super) availability: ValidatedAllocatorAvailability,
    pub(super) legality: ValidatedAllocationLegality,
    pub(super) custody: AllocationLegalityCustodyReceipt,
}

impl StagedOptimizedAllocationLegality {
    pub const fn live_range_stage(&self) -> &StagedOptimizedLiveRanges {
        &self.ranges
    }
    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        &self.legality
    }
    pub const fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
        &self.availability
    }
    pub const fn custody(&self) -> AllocationLegalityCustodyReceipt {
        self.custody
    }
}

pub use register_homes::AllocationLegalityCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAllocationLegalityCustodyError {
    UpstreamLiveRanges(OptimizedLiveRangeCustodyError),
    Availability(AllocatorAvailabilityError),
    Analysis(AllocationLegalityError),
    Revalidation(AllocationLegalityError),
    UnsupportedFramelessLeafConvention,
    MissingRequiredActiveResidentRematerializationView(&'static str),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedAllocationLegalityCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized allocation-legality staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAllocationLegalityCustodyError {}
