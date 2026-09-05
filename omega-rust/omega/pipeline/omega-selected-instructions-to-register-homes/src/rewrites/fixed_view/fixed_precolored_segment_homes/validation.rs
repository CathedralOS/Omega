use crate::{
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
    ValidatedFixedPrecoloredSplitRequirements, validate_fixed_precolored_intervals,
    validate_fixed_precolored_segment_homes, validate_fixed_precolored_split_requirements,
};

use crate::{
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    validate_optimized_allocation_legality_custody,
};

use super::{
    OptimizedFixedPrecoloredSegmentHomeCustodyError,
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt, custody,
};

pub fn validate_optimized_fixed_precolored_segment_home_custody(
    source: &StagedOptimizedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
) -> Result<
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    OptimizedFixedPrecoloredSegmentHomeCustodyError,
> {
    let upstream = validate_source(source)?;
    let ranges = source.live_range_stage().ranges();
    let replayed_fixed =
        validate_fixed_precolored_intervals(ranges, source.legality(), fixed.plan().clone())
            .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::FixedIntervals)?;
    let replayed_requirements = validate_fixed_precolored_split_requirements(
        ranges,
        source.legality(),
        &replayed_fixed,
        requirements.plan().clone(),
    )
    .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::SplitRequirements)?;
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed_homes = validate_fixed_precolored_segment_homes(
        ranges,
        source.legality(),
        &replayed_fixed,
        &replayed_requirements,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        homes.plan().clone(),
    )
    .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::SegmentHomes)?;
    if replayed_fixed.receipt() != fixed.receipt()
        || replayed_requirements.receipt() != requirements.receipt()
        || replayed_homes.receipt() != homes.receipt()
    {
        return Err(OptimizedFixedPrecoloredSegmentHomeCustodyError::ReceiptMismatch);
    }
    Ok(custody::seal(
        upstream,
        replayed_fixed.receipt(),
        replayed_requirements.receipt(),
        replayed_homes.receipt(),
    ))
}

pub(super) fn validate_source(
    source: &StagedOptimizedAllocationLegality,
) -> Result<
    StagedOptimizedAllocationLegalityCustodyReceipt,
    OptimizedFixedPrecoloredSegmentHomeCustodyError,
> {
    validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(OptimizedFixedPrecoloredSegmentHomeCustodyError::UpstreamLegality)
}
