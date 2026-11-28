//! Move completed analysis records across the stage boundary without walking plans.
//! Allocation computes legality when absent or when required register availability differs.
use crate::{
    AllocatorAvailabilityRequirement, OptimizedAllocationLegalityCustodyError,
    SelectedOptimizationInput, StagedOptimizedAllocationLegality, StagedOptimizedLiveRanges,
    StagedOptimizedLiveness, ValidatedAllocationLegality, ValidatedAllocatorAvailability,
    ValidatedLiveRanges, ValidatedLiveness, stage_optimized_allocation_legality,
};
use selected_analysis_facts::{
    AllocationLegalityFacts, LiveRangeFacts, LivenessFacts, SelectedFacts, SelectedFactsParts,
};

pub fn publish_live_ranges(ranges: StagedOptimizedLiveRanges) -> SelectedFactsParts {
    let (liveness, ranges) = ranges.into_parts();
    let (selected, liveness) = liveness.into_parts();
    SelectedFactsParts {
        selected: selected.program,
        custody: selected.custody,
        environment: selected.register_environment,
        selections: selected.selections,
        budget: selected.budget_per_pass,
        history: selected.record,
        liveness: LivenessFacts {
            plan: liveness.plan,
            receipt: liveness.receipt,
        },
        ranges: LiveRangeFacts {
            plan: ranges.plan,
            receipt: ranges.receipt,
            function_counts: ranges.function_counts,
        },
        legality: None,
    }
}

pub fn publish_legality(legality: StagedOptimizedAllocationLegality) -> SelectedFactsParts {
    let (ranges, availability, legality) = legality.into_parts();
    let mut parts = publish_live_ranges(ranges);
    parts.legality = Some(AllocationLegalityFacts {
        plan: legality.plan,
        receipt: legality.receipt,
        function_counts: legality.function_counts,
        availability: availability.plan,
        availability_receipt: availability.receipt,
    });
    parts
}

/// Allocation chooses the availability policy. Reuse issued legality only when
/// it names that policy; otherwise derive it from the issued live ranges.
pub fn prepare_allocation_legality(
    facts: SelectedFacts,
    required: Option<AllocatorAvailabilityRequirement>,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let parts = facts.into_parts();
    let selected = SelectedOptimizationInput {
        program: parts.selected,
        custody: parts.custody,
        register_environment: parts.environment,
        selections: parts.selections,
        budget_per_pass: parts.budget,
        record: parts.history,
    };
    let liveness = StagedOptimizedLiveness::from_published(
        selected,
        ValidatedLiveness {
            plan: parts.liveness.plan,
            receipt: parts.liveness.receipt,
        },
    );
    let ranges = StagedOptimizedLiveRanges::from_published(
        liveness,
        ValidatedLiveRanges {
            plan: parts.ranges.plan,
            receipt: parts.ranges.receipt,
            function_counts: parts.ranges.function_counts,
        },
    );
    let required =
        required.unwrap_or(AllocatorAvailabilityRequirement::AllEnvironmentAllocatableViews);
    match parts.legality {
        None => stage_optimized_allocation_legality(ranges, required),
        Some(legality) => StagedOptimizedAllocationLegality::from_published(
            ranges,
            ValidatedAllocatorAvailability {
                plan: legality.availability,
                receipt: legality.availability_receipt,
            },
            ValidatedAllocationLegality {
                plan: legality.plan,
                receipt: legality.receipt,
                function_counts: legality.function_counts,
            },
        )
        .for_availability(required),
    }
}
