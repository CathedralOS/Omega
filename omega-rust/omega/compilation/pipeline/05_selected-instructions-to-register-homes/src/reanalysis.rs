/*
We re-derive validated liveness, live-range, and allocation-legality facts over
a selected program transformed by `ValidatedFixedViewCopies` so post-copy home
assignment and residual runtime-spill recovery operate on current checked facts.
*/

pub(crate) mod compute {
    use crate::{
        ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
        analyze_allocation_legality, analyze_live_ranges, analyze_liveness,
    };

    use crate::StagedOptimizedFixedViewCopies;

    use super::OptimizedSelectedReanalysisError;
    use super::invariants::require_no_transitions;

    pub(crate) fn compute_selected_reanalysis(
        transformation: &StagedOptimizedFixedViewCopies,
    ) -> Result<
        (
            ValidatedLiveness,
            ValidatedLiveRanges,
            ValidatedAllocationLegality,
        ),
        OptimizedSelectedReanalysisError,
    > {
        let copies = transformation.copies();
        let liveness =
            analyze_liveness(copies).map_err(OptimizedSelectedReanalysisError::Liveness)?;
        let ranges = analyze_live_ranges(copies, &liveness)
            .map_err(OptimizedSelectedReanalysisError::LiveRanges)?;
        let environment = transformation.register_environment();
        let availability = transformation.allocator_availability();
        let legality = analyze_allocation_legality(
            &ranges,
            availability,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
        )
        .map_err(OptimizedSelectedReanalysisError::AllocationLegality)?;
        require_no_transitions(&legality)?;
        Ok((liveness, ranges, legality))
    }
}
pub(crate) mod custody {
    use crate::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness};

    use crate::StagedOptimizedFixedViewCopyCustodyReceipt;

    use super::StagedOptimizedSelectedReanalysisCustodyReceipt;

    pub(crate) fn selected_reanalysis_custody_receipt(
        source: StagedOptimizedFixedViewCopyCustodyReceipt,
        liveness: &ValidatedLiveness,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
    ) -> StagedOptimizedSelectedReanalysisCustodyReceipt {
        StagedOptimizedSelectedReanalysisCustodyReceipt {
            source,
            transformed_liveness: liveness.receipt().identity(),
            transformed_ranges: ranges.receipt().identity(),
            transformed_legality: legality.receipt().identity(),
            allocator_availability: legality.receipt().allocator_availability(),
        }
    }
}
pub(crate) mod invariants {
    use crate::ValidatedAllocationLegality;

    use super::OptimizedSelectedReanalysisError;

    pub(crate) fn require_no_transitions(
        legality: &ValidatedAllocationLegality,
    ) -> Result<(), OptimizedSelectedReanalysisError> {
        let count = legality.receipt().entry_transition_count();
        if count != 0 {
            return Err(OptimizedSelectedReanalysisError::RemainingTransitions { count });
        }
        Ok(())
    }
}

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

#[derive(Debug)]
pub struct StagedOptimizedSelectedReanalysis {
    transformation: StagedOptimizedFixedViewCopies,
    liveness: ValidatedLiveness,
    ranges: ValidatedLiveRanges,
    legality: ValidatedAllocationLegality,
    custody: StagedOptimizedSelectedReanalysisCustodyReceipt,
}

impl StagedOptimizedSelectedReanalysis {
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

copy_receipt! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StagedOptimizedSelectedReanalysisCustodyReceipt {
        source: StagedOptimizedFixedViewCopyCustodyReceipt,
        transformed_liveness: selected_instructions::LivenessIdentity,
        transformed_ranges: selected_instructions::LiveRangeIdentity,
        transformed_legality: register_homes::AllocationLegalityIdentity,
        allocator_availability: register_homes::AllocatorAvailabilityIdentity,
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

failed_display!(
    OptimizedSelectedReanalysisError,
    "optimized transformed-selected reanalysis failed"
);
