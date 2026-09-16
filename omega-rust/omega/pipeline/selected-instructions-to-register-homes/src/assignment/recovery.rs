use crate::RegisterAllocationError;
use crate::{
    FixedViewCopyPolicy, PressureRematerializationPolicy, RecoveryClassificationPolicy,
    SpillChoicePolicy,
};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedAllocationLegality,
    StagedOptimizedLiveRanges, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    stage_optimized_active_resident_rematerialization, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
    stage_optimized_register_homes_after_fixed_view_copies, stage_optimized_selected_reanalysis,
};

/// Sequence the fixed/precolored segment-home analysis, one admitted
/// fixed-view copy policy, complete selected reanalysis, and post-copy
/// assignment over an already-staged legality chain. Every hop replays the
/// evidence roots the copy plan binds, so the transformation, its reanalyzed
/// liveness/ranges/legality, and the post-allocation manifest all stay under
/// the same custody as any other allocation route.
fn fixed_view_allocation(
    legality: StagedOptimizedAllocationLegality,
    policy: FixedViewCopyPolicy,
) -> Result<StagedOptimizedRegisterHomesAfterFixedViewCopies, RegisterAllocationError> {
    let budget = legality
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let segments = stage_optimized_fixed_precolored_segment_homes(legality, budget)
        .map_err(RegisterAllocationError::FixedSegments)?;
    let copies = stage_optimized_fixed_view_copies(segments, policy, budget)
        .map_err(RegisterAllocationError::FixedViewCopies)?;
    let reanalysis =
        stage_optimized_selected_reanalysis(copies).map_err(RegisterAllocationError::Reanalysis)?;
    stage_optimized_register_homes_after_fixed_view_copies(reanalysis)
        .map_err(RegisterAllocationError::FixedViewHomes)
}

pub fn stage_fixed_view_register_allocation(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedRegisterHomesAfterFixedViewCopies, RegisterAllocationError> {
    let legality =
        stage_optimized_allocation_legality(ranges).map_err(RegisterAllocationError::Legality)?;
    fixed_view_allocation(
        legality,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    )
}

/// Default-path recovery for unresolved entry-fixed-view transitions. The
/// leaf-local policy admits exactly the boundaries authenticated by recorded
/// entry transitions — one copy in the leaf block immediately before each
/// fixed leaf use of an entry-pinned parameter — so it needs no declared
/// recovery selection and stays rejected for every other shape.
pub fn stage_leaf_local_fixed_view_register_allocation(
    legality: StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedRegisterHomesAfterFixedViewCopies, RegisterAllocationError> {
    fixed_view_allocation(legality, FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1)
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
