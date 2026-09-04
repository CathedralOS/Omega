use omega_regalloc::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    analyze_allocation_legality, analyze_live_ranges, analyze_liveness,
};

use omega_allocation_legality_to_fixed_view_copies::StagedOptimizedFixedViewCopies;

use super::invariants::require_no_transitions;
use super::model::OptimizedSelectedReanalysisError;

pub(super) fn compute_selected_reanalysis(
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
    let liveness = analyze_liveness(copies).map_err(OptimizedSelectedReanalysisError::Liveness)?;
    let ranges = analyze_live_ranges(copies, &liveness)
        .map_err(OptimizedSelectedReanalysisError::LiveRanges)?;
    let environment = transformation
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let availability = transformation
        .source_legality_stage()
        .allocator_availability();
    let legality = analyze_allocation_legality(
        &ranges,
        availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedSelectedReanalysisError::AllocationLegality)?;
    require_no_transitions(&legality)?;
    Ok((liveness, ranges, legality))
}
