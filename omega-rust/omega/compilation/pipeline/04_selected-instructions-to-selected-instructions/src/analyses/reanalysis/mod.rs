//! Optimizer module role: executable entrance. Complete reanalysis after selected-CFG transformation.
//!
//! No source analysis fact is reused. This entrance validates transformed
//! source custody, recomputes liveness/ranges/legality, independently replays
//! the complete chain, and only then grants reanalysis custody.

mod compute;
mod custody;
mod invariants;

use crate::StagedOptimizedFixedViewCopies;
use crate::{
    AllocationLegalityError, LiveRangeError, LivenessError, OptimizedFixedViewCopyCustodyError,
    StagedOptimizedFixedViewCopyCustodyReceipt, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedLiveness,
};
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;

pub fn stage_optimized_selected_reanalysis(
    transformation: StagedOptimizedFixedViewCopies,
) -> Result<StagedOptimizedSelectedReanalysis, OptimizedSelectedReanalysisError> {
    let (liveness, ranges, legality) = compute::compute_selected_reanalysis(&transformation)?;
    invariants::require_no_transitions(&legality)?;
    let custody = custody::selected_reanalysis_custody_receipt(
        transformation.custody(),
        &liveness,
        &ranges,
        &legality,
    );
    Ok(StagedOptimizedSelectedReanalysis {
        transformation,
        liveness,
        ranges,
        legality,
        custody,
    })
}

/// Complete mandatory reanalysis of one independently validated transformed
/// selected CFG. No source analysis fact is reused after the rewrite.
#[derive(Debug)]
pub struct StagedOptimizedSelectedReanalysis {
    transformation: StagedOptimizedFixedViewCopies,
    liveness: ValidatedLiveness,
    ranges: ValidatedLiveRanges,
    legality: ValidatedAllocationLegality,
    custody: StagedOptimizedSelectedReanalysisCustodyReceipt,
}

impl StagedOptimizedSelectedReanalysis {
    /// The retained producer stage. Replay and custody validation inspect it;
    /// ordinary consumers read the transformed program facts directly.
    pub const fn transformation_stage(&self) -> &StagedOptimizedFixedViewCopies {
        &self.transformation
    }

    staged_producer_accessors!(
        transformation; register_environment, allocator_availability, selections, budget_per_pass
    );

    validated_analysis_accessors!();
    receipt_accessor_rows!(ref StagedOptimizedSelectedReanalysis {
        custody: StagedOptimizedSelectedReanalysisCustodyReceipt,
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedSelectedReanalysisCustodyReceipt {
    source: StagedOptimizedFixedViewCopyCustodyReceipt,
    transformed_liveness: selected_instructions::LivenessIdentity,
    transformed_ranges: selected_instructions::LiveRangeIdentity,
    transformed_legality: register_homes::AllocationLegalityIdentity,
    allocator_availability: register_homes::AllocatorAvailabilityIdentity,
}

impl StagedOptimizedSelectedReanalysisCustodyReceipt {
    receipt_accessor_rows!(StagedOptimizedSelectedReanalysisCustodyReceipt {
        source: StagedOptimizedFixedViewCopyCustodyReceipt,
        transformed_liveness: selected_instructions::LivenessIdentity,
        transformed_ranges: selected_instructions::LiveRangeIdentity,
        transformed_legality: register_homes::AllocationLegalityIdentity,
        allocator_availability: register_homes::AllocatorAvailabilityIdentity,
    });
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

failed_display!(
    OptimizedSelectedReanalysisError,
    "optimized transformed-selected reanalysis failed"
);

impl std::error::Error for OptimizedSelectedReanalysisError {}
