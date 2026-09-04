use omega_optimization_core::OptimizationWorkBudget;
use omega_regalloc::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSegmentHomePolicy,
    FixedPrecoloredSplitRequirementPolicy, ValidatedFixedPrecoloredIntervals,
    ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
    analyze_fixed_precolored_intervals, analyze_fixed_precolored_split_requirements,
    assign_fixed_precolored_segment_homes,
};

use omega_live_ranges_to_allocation_legality::StagedOptimizedAllocationLegality;

use super::OptimizedFixedPrecoloredSegmentHomeCustodyError;

pub(super) fn derive(
    source: &StagedOptimizedAllocationLegality,
    budget: OptimizationWorkBudget,
) -> Result<
    (
        ValidatedFixedPrecoloredIntervals,
        ValidatedFixedPrecoloredSplitRequirements,
        ValidatedFixedPrecoloredSegmentHomes,
    ),
    OptimizedFixedPrecoloredSegmentHomeCustodyError,
> {
    let ranges = source.live_range_stage().ranges();
    let fixed = analyze_fixed_precolored_intervals(
        ranges,
        source.legality(),
        FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
        budget,
    )
    .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::FixedIntervals)?;
    let requirements = analyze_fixed_precolored_split_requirements(
        ranges,
        source.legality(),
        &fixed,
        FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
        budget,
    )
    .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::SplitRequirements)?;
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let homes = assign_fixed_precolored_segment_homes(
        ranges,
        source.legality(),
        &fixed,
        &requirements,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1,
        budget,
    )
    .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::SegmentHomes)?;
    Ok((fixed, requirements, homes))
}
