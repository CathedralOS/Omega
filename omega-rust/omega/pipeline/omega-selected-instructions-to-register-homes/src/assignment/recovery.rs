use super::RegisterAllocationError;
use crate::{
    FixedViewCopyPolicy, PressureRematerializationPolicy, RecoveryClassificationPolicy,
    SpillChoicePolicy,
};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedLiveRanges,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    stage_optimized_active_resident_rematerialization, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
    stage_optimized_register_homes_after_fixed_view_copies, stage_optimized_selected_reanalysis,
};

pub fn stage_fixed_view_register_allocation(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedRegisterHomesAfterFixedViewCopies, RegisterAllocationError> {
    let budget = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let legality =
        stage_optimized_allocation_legality(ranges).map_err(RegisterAllocationError::Legality)?;
    let segments = stage_optimized_fixed_precolored_segment_homes(legality, budget)
        .map_err(RegisterAllocationError::FixedSegments)?;
    let copies = stage_optimized_fixed_view_copies(
        segments,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        budget,
    )
    .map_err(RegisterAllocationError::FixedViewCopies)?;
    let reanalysis =
        stage_optimized_selected_reanalysis(copies).map_err(RegisterAllocationError::Reanalysis)?;
    stage_optimized_register_homes_after_fixed_view_copies(reanalysis)
        .map_err(RegisterAllocationError::FixedViewHomes)
}

pub fn stage_active_resident_register_allocation(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedActiveResidentRematerialization, RegisterAllocationError> {
    let budget = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let legality = stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges).map_err(RegisterAllocationError::Legality)?;
    stage_optimized_active_resident_rematerialization(
        legality,
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        budget,
    ).map_err(RegisterAllocationError::Rematerialization)
}
