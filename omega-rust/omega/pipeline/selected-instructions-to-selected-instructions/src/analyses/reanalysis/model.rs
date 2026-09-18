use crate::{
    AllocationLegalityError, LiveRangeError, LivenessError, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedLiveness,
};

use crate::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt,
};
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;

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
    /// The retained producer stage. Replay and custody validation inspect it;
    /// ordinary consumers read the transformed program facts directly.
    pub const fn transformation_stage(&self) -> &StagedOptimizedFixedViewCopies {
        &self.transformation
    }

    /// The target register environment governing the transformed program.
    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        self.transformation.register_environment()
    }

    /// The environment-derived allocator availability governing the program.
    pub const fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
        self.transformation.allocator_availability()
    }

    /// The governing optimizer selections for this admission.
    pub fn selections(&self) -> &OptimizationSelections {
        self.transformation.selections()
    }

    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.transformation.budget_per_pass()
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
    pub(super) transformed_liveness: crate::LivenessIdentity,
    pub(super) transformed_ranges: crate::LiveRangeIdentity,
    pub(super) transformed_legality: crate::AllocationLegalityIdentity,
    pub(super) allocator_availability: crate::AllocatorAvailabilityIdentity,
    pub(super) function_count: usize,
    pub(super) virtual_register_count: usize,
    pub(super) entry_transition_count: usize,
}

impl StagedOptimizedSelectedReanalysisCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedFixedViewCopyCustodyReceipt {
        self.source
    }
    pub const fn transformed_liveness(self) -> crate::LivenessIdentity {
        self.transformed_liveness
    }
    pub const fn transformed_ranges(self) -> crate::LiveRangeIdentity {
        self.transformed_ranges
    }
    pub const fn transformed_legality(self) -> crate::AllocationLegalityIdentity {
        self.transformed_legality
    }
    pub const fn allocator_availability(self) -> crate::AllocatorAvailabilityIdentity {
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
