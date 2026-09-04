use omega_regalloc::{
    AllocationLegalityError, LiveRangeError, LivenessError, ValidatedAllocationLegality,
    ValidatedLiveRanges, ValidatedLiveness,
};

use omega_allocation_legality_to_fixed_view_copies::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt,
};

/// Complete mandatory reanalysis of one independently validated transformed
/// selected CFG. No source analysis fact is reused after the rewrite.
#[derive(Debug)]
pub struct StagedOptimizedSelectedReanalysis {
    pub(super) transformation: StagedOptimizedFixedViewCopies,
    pub(super) liveness: ValidatedLiveness,
    pub(super) ranges: ValidatedLiveRanges,
    pub(super) legality: ValidatedAllocationLegality,
    pub(super) custody: StagedOptimizedSelectedReanalysisCustodyReceipt,
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
    pub const fn custody(&self) -> StagedOptimizedSelectedReanalysisCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedSelectedReanalysisCustodyReceipt {
    pub(super) source: StagedOptimizedFixedViewCopyCustodyReceipt,
    pub(super) transformed_liveness: omega_regalloc::LivenessIdentity,
    pub(super) transformed_ranges: omega_regalloc::LiveRangeIdentity,
    pub(super) transformed_legality: omega_regalloc::AllocationLegalityIdentity,
    pub(super) allocator_availability: omega_regalloc::AllocatorAvailabilityIdentity,
    pub(super) function_count: usize,
    pub(super) virtual_register_count: usize,
    pub(super) entry_transition_count: usize,
}

impl StagedOptimizedSelectedReanalysisCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedFixedViewCopyCustodyReceipt {
        self.source
    }
    pub const fn transformed_liveness(self) -> omega_regalloc::LivenessIdentity {
        self.transformed_liveness
    }
    pub const fn transformed_ranges(self) -> omega_regalloc::LiveRangeIdentity {
        self.transformed_ranges
    }
    pub const fn transformed_legality(self) -> omega_regalloc::AllocationLegalityIdentity {
        self.transformed_legality
    }
    pub const fn allocator_availability(self) -> omega_regalloc::AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

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
