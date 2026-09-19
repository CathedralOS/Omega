use crate::RegisterAllocationError;
use crate::{
    FixedViewCopyPolicy, PressureRematerializationPolicy, RecoveryClassificationPolicy,
    SpillChoicePolicy,
};
use crate::{
    RetainedAllocation, StagedOptimizedAllocationLegality, StagedOptimizedLiveRanges,
    stage_optimized_active_resident_rematerialization_pressure,
    stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
    stage_optimized_selected_reanalysis,
};

/// The shared fixed-view preflight: fixed/precolored segment-home analysis,
/// one admitted copy policy, and the complete selected reanalysis over an
/// already-staged legality chain. Every hop replays the evidence roots the
/// copy plan binds, so the transformation and its reanalyzed
/// liveness/ranges/legality stay under the same custody as any other
/// allocation route.
fn fixed_view_reanalysis(
    legality: StagedOptimizedAllocationLegality,
    policy: FixedViewCopyPolicy,
) -> Result<crate::StagedOptimizedSelectedReanalysis, RegisterAllocationError> {
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
    stage_optimized_selected_reanalysis(copies).map_err(RegisterAllocationError::Reanalysis)
}

/// The homes-only sequence: post-copy assignment is the terminal step, so
/// residual `NoCompatibleHome` pressure keeps the direct post-copy error
/// surface for callers that consume the staged homes result.
fn fixed_view_homes(
    legality: StagedOptimizedAllocationLegality,
    policy: FixedViewCopyPolicy,
) -> Result<crate::StagedOptimizedRegisterHomesAfterFixedViewCopies, RegisterAllocationError> {
    let reanalysis = fixed_view_reanalysis(legality, policy)?;
    crate::assignment::baseline::stage_optimized_register_homes_after_fixed_view_copies(reanalysis)
        .map_err(RegisterAllocationError::FixedViewHomes)
}

/// The composing sequence: residual `NoCompatibleHome` pressure after the
/// copies does not fail the route — the reanalysis becomes runtime-spill
/// recovery's source, so a pressured program that also needed fixed-view
/// copies composes the two recoveries under one retained allocation. Any
/// other assignment failure keeps the direct post-copy error surface.
///
/// The declared shared-entry route gets one earlier arm: its segment-home
/// front-end is probed on the borrowed legality before the sequence commits,
/// and a capacity decline — placement pressure or front-end work-budget
/// exhaustion — hands the still-owned legality to runtime spill with the
/// declined policy and verdict recorded for replay-bound selection
/// evidence. Probing is confined to that declared policy: the leaf-local
/// default path is entered only because unresolved entry transitions still
/// need the copies this sequence materializes, so a decline there could
/// never reach spill recovery and keeps the staged `FixedSegments` surface.
/// Every other probe outcome falls through to the staged sequence, which
/// reproduces any non-decline failure unchanged.
fn fixed_view_allocation(
    legality: StagedOptimizedAllocationLegality,
    policy: FixedViewCopyPolicy,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    if policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 {
        let budget = legality
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target()
            .optimized()
            .budget_per_pass();
        if let Some(decline) =
            crate::probe_optimized_fixed_precolored_segment_homes(&legality, budget)
                .err()
                .and_then(|error| error.capacity_decline())
        {
            return RetainedAllocation::try_from(
                crate::assignment::runtime_spill::recover_after_declined_fixed_view_probe(
                    legality, policy, decline,
                )
                .map_err(RegisterAllocationError::RuntimeSpill)?,
            )
            .map_err(RegisterAllocationError::Replay);
        }
    }
    let reanalysis = fixed_view_reanalysis(legality, policy)?;
    match crate::assignment::baseline::assign_optimized_register_homes_after_fixed_view_copies(
        &reanalysis,
    ) {
        Ok(homes) => RetainedAllocation::try_from(
            crate::assignment::baseline::stage_register_homes_after_fixed_view_copies_with_assignment(
                reanalysis,
                homes,
            )
            .map_err(RegisterAllocationError::FixedViewHomes)?,
        )
        .map_err(RegisterAllocationError::Replay),
        Err(crate::RegisterHomeError::NoCompatibleHome { .. }) => RetainedAllocation::try_from(
            crate::assignment::runtime_spill::recover_after_fixed_view_copies(reanalysis)
                .map_err(RegisterAllocationError::RuntimeSpill)?,
        )
        .map_err(RegisterAllocationError::Replay),
        Err(error) => Err(RegisterAllocationError::FixedViewHomes(
            crate::OptimizedPostCopyRegisterHomeCustodyError::Assignment(error),
        )),
    }
}

pub fn stage_fixed_view_register_allocation(
    ranges: StagedOptimizedLiveRanges,
) -> Result<crate::StagedOptimizedRegisterHomesAfterFixedViewCopies, RegisterAllocationError> {
    let legality =
        stage_optimized_allocation_legality(ranges).map_err(RegisterAllocationError::Legality)?;
    fixed_view_homes(
        legality,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    )
}

/// The declared-rule composing form over an already-staged legality chain, so
/// a pressured source exercises the same sequence
/// `stage_fixed_view_register_allocation` runs after legality staging —
/// including the runtime-spill composition a residual `NoCompatibleHome`
/// admits.
pub fn stage_shared_entry_fixed_view_register_allocation(
    legality: StagedOptimizedAllocationLegality,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    fixed_view_allocation(
        legality,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    )
}

/// Default-path recovery for unresolved fixed-view transitions. The
/// immediate-site policy admits the boundaries authenticated by recorded
/// fixed-site transitions — one copy in the site's own block immediately
/// before the fixed use of a source-scalar register — so it needs no
/// declared recovery selection and stays rejected for every other shape.
///
/// This staged-homes form keeps post-copy assignment terminal; the production
/// route composes residual pressure through
/// `stage_leaf_local_fixed_view_register_allocation_composing` instead.
pub fn stage_leaf_local_fixed_view_register_allocation(
    legality: StagedOptimizedAllocationLegality,
) -> Result<crate::StagedOptimizedRegisterHomesAfterFixedViewCopies, RegisterAllocationError> {
    fixed_view_homes(legality, FixedViewCopyPolicy::ImmediateBeforeFixedUseV1)
}

/// The immediate-site sequence in its composing form: the route the default
/// allocation path takes on unresolved fixed-site transitions, publishing one
/// retained allocation whether post-copy homes or the runtime-spill
/// composition resolved the program.
pub fn stage_leaf_local_fixed_view_register_allocation_composing(
    legality: StagedOptimizedAllocationLegality,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    fixed_view_allocation(legality, FixedViewCopyPolicy::ImmediateBeforeFixedUseV1)
}

/// The declared active-resident route in its composing form: the sweep is
/// proven as a validated prefix — transformed program plus rebuilt
/// liveness, ranges, and legality — then assignment runs over those rebuilt
/// facts. A residual `NoCompatibleHome` verdict does not fail the route or
/// discard the proven rematerialization: the same prefix becomes
/// runtime-spill recovery's source, so the recorded sweep stays first in
/// the produced manifest and the declared selection binds through replayed
/// custody evidence. Every other failure keeps the existing typed error
/// surface.
pub fn stage_active_resident_register_allocation(
    ranges: StagedOptimizedLiveRanges,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    let budget = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let legality = stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges).map_err(RegisterAllocationError::Legality)?;
    let pressure = stage_optimized_active_resident_rematerialization_pressure(
        legality,
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        budget,
    ).map_err(RegisterAllocationError::Rematerialization)?;
    let environment = pressure
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    match crate::assign_register_homes(
        pressure.legality(),
        pressure.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    ) {
        Ok(homes) => RetainedAllocation::try_from(
            crate::complete_optimized_active_resident_rematerialization(pressure, homes)
                .map_err(RegisterAllocationError::Rematerialization)?,
        )
        .map_err(RegisterAllocationError::Replay),
        Err(crate::RegisterHomeError::NoCompatibleHome { .. }) => RetainedAllocation::try_from(
            crate::assignment::runtime_spill::recover_after_active_resident_rematerialization(
                pressure,
            )
            .map_err(RegisterAllocationError::RuntimeSpill)?,
        )
        .map_err(RegisterAllocationError::Replay),
        // Any other assignment failure keeps the surface the one-shot sweep
        // produced: the homes error wrapped under the rematerialization arm.
        Err(error) => Err(RegisterAllocationError::Rematerialization(
            crate::OptimizedActiveResidentRematerializationError::Homes(error),
        )),
    }
}
