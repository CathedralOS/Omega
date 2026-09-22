//! Optimizer module role: executable entrance. Complete reanalysis after selected-CFG transformation.
//!
//! No source analysis fact is reused. This entrance validates transformed
//! source custody, recomputes liveness/ranges/legality, independently replays
//! the complete chain, and only then grants reanalysis custody.

mod compute;
mod custody;
mod invariants;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::validate_optimized_selected_reanalysis_custody;

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
    validation::validate_source(&transformation)?;
    let (liveness, ranges, legality) = compute::compute_selected_reanalysis(&transformation)?;
    let custody = validate_optimized_selected_reanalysis_custody(
        &transformation,
        &liveness,
        &ranges,
        &legality,
    )?;
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
    source: StagedOptimizedFixedViewCopyCustodyReceipt,
    transformed_liveness: selected_instructions::LivenessIdentity,
    transformed_ranges: selected_instructions::LiveRangeIdentity,
    transformed_legality: register_homes::AllocationLegalityIdentity,
    allocator_availability: register_homes::AllocatorAvailabilityIdentity,
    function_count: usize,
    virtual_register_count: usize,
    entry_transition_count: usize,
}

impl StagedOptimizedSelectedReanalysisCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedFixedViewCopyCustodyReceipt {
        self.source
    }
    pub const fn transformed_liveness(self) -> selected_instructions::LivenessIdentity {
        self.transformed_liveness
    }
    pub const fn transformed_ranges(self) -> selected_instructions::LiveRangeIdentity {
        self.transformed_ranges
    }
    pub const fn transformed_legality(self) -> register_homes::AllocationLegalityIdentity {
        self.transformed_legality
    }
    pub const fn allocator_availability(self) -> register_homes::AllocatorAvailabilityIdentity {
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
