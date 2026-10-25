use crate::{
    RegisterAllocationError, RetainedAllocation,
    stage_optimized_active_resident_rematerialization_pressure,
};
use register_homes::{RecoveryClassificationPolicy, SpillChoicePolicy};
use selected_instructions_to_selected_instructions::{
    FixedViewCopyPolicy, PressureRematerializationPolicy, StagedOptimizedAllocationLegality,
    StagedOptimizedLiveRanges, StagedOptimizedSelectedReanalysis,
    StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationRun,
    probe_optimized_fixed_precolored_segment_homes,
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
) -> Result<StagedOptimizedSelectedReanalysis, RegisterAllocationError> {
    let budget = legality.budget_per_pass();
    let segments = stage_optimized_fixed_precolored_segment_homes(legality, budget)
        .map_err(RegisterAllocationError::FixedSegments)?;
    let copies = stage_optimized_fixed_view_copies(segments, policy, budget)
        .map_err(RegisterAllocationError::FixedViewCopies)?;
    stage_optimized_selected_reanalysis(copies).map_err(RegisterAllocationError::Reanalysis)
}

/// The composing sequence: residual runtime-recoverable pressure after the
/// copies does not fail the route — the reanalysis becomes runtime-spill
/// recovery's source, so a pressured program that also needed fixed-view
/// copies composes the two recoveries under one retained allocation. Any
/// other assignment failure keeps the direct post-copy error surface.
///
/// The declared shared-entry route gets one earlier arm: its segment-home
/// front-end is probed on the borrowed legality before the sequence commits,
/// and a capacity decline — placement pressure or front-end work-budget
/// exhaustion — hands the still-owned legality to runtime spill with the
/// declined policy recorded as selection evidence. Probing is confined to that declared policy: the
/// shared-source-exit default path is entered only because unresolved entry
/// transitions still need the copies this sequence materializes, so a
/// decline there could never reach spill recovery and keeps the staged
/// `FixedSegments` surface.
/// Every other probe outcome falls through to the staged sequence, which
/// reproduces any non-decline failure unchanged.
fn fixed_view_allocation(
    legality: StagedOptimizedAllocationLegality,
    policy: FixedViewCopyPolicy,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    if policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 {
        let budget = legality.budget_per_pass();
        if probe_optimized_fixed_precolored_segment_homes(&legality, budget)
            .err()
            .and_then(|error| error.capacity_decline())
            .is_some()
        {
            return RetainedAllocation::try_from(
                crate::assignment::runtime_spill::recover_after_declined_fixed_view_probe(
                    legality, policy,
                )
                .map_err(RegisterAllocationError::RuntimeSpill)?,
            )
            .map_err(RegisterAllocationError::Admission);
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
        .map_err(RegisterAllocationError::Admission),
        Err(error) if error.admits_runtime_recovery() => RetainedAllocation::try_from(
            crate::assignment::runtime_spill::recover_after_fixed_view_copies(reanalysis)
                .map_err(RegisterAllocationError::RuntimeSpill)?,
        )
        .map_err(RegisterAllocationError::Admission),
        Err(error) => Err(RegisterAllocationError::FixedViewHomes(
            crate::OptimizedPostCopyRegisterHomeCustodyError::Assignment(error),
        )),
    }
}

/// The declared-rule composing form over an already-staged legality chain, so
/// a pressured source exercises the same fixed-view sequence after legality
/// staging — including the runtime-spill composition runtime-recoverable
/// pressure admits.
pub(crate) fn stage_shared_entry_fixed_view_register_allocation(
    legality: StagedOptimizedAllocationLegality,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    fixed_view_allocation(
        legality,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    )
}

/// The declared active-resident route in its composing form: the sweep is
/// proven as a validated prefix — transformed program plus rebuilt
/// liveness, ranges, and legality — then assignment runs over those rebuilt
/// facts. A runtime-recoverable verdict does not fail the route or
/// discard the proven rematerialization: the same prefix becomes
/// runtime-spill recovery's source, so the recorded sweep stays first in
/// the produced manifest and the declared selection binds through replayed
/// custody evidence. Every other failure keeps the existing typed error
/// surface.
pub(crate) fn stage_active_resident_register_allocation(
    ranges: StagedOptimizedLiveRanges,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    let budget = ranges.budget_per_pass();
    let legality = stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges).map_err(RegisterAllocationError::Legality)?;
    let pressure = stage_optimized_active_resident_rematerialization_pressure(
        legality,
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        budget,
    ).map_err(RegisterAllocationError::Rematerialization)?;
    let environment = pressure.source().register_environment();
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
            crate::rewrites::complete_optimized_active_resident_rematerialization(pressure, homes)
                .map_err(RegisterAllocationError::Rematerialization)?,
        )
        .map_err(RegisterAllocationError::Admission),
        Err(error) if error.admits_runtime_recovery() => RetainedAllocation::try_from(
            crate::assignment::runtime_spill::recover_after_active_resident_rematerialization(
                pressure,
            )
            .map_err(RegisterAllocationError::RuntimeSpill)?,
        )
        .map_err(RegisterAllocationError::Admission),
        // Any other assignment failure keeps the surface the one-shot sweep
        // produced: the homes error wrapped under the rematerialization arm.
        Err(error) => Err(RegisterAllocationError::Rematerialization(
            crate::OptimizedActiveResidentRematerializationError::Homes(error),
        )),
    }
}

/// A completed selected-lowering run in its composing form: the suite stays
/// the proven prefix — its fold ledger and completion identity record ahead
/// of every recovery step — while residual runtime-recoverable pressure over
/// the run's final facts hands custody to runtime spill instead of failing
/// the route. Any other assignment failure keeps the post-lowering homes
/// error surface.
pub(crate) fn stage_selected_lowering_register_allocation(
    run: StagedSelectedLoweringOptimizationRun,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    match crate::assignment::transformed::assign_optimized_register_homes_after_selected_lowering(
        &run,
    ) {
        Ok(homes) => RetainedAllocation::try_from(
            crate::assignment::transformed::stage_optimized_register_homes_after_selected_lowering_with_assignment(run, homes)
                .map_err(RegisterAllocationError::TransformedHomes)?,
        )
        .map_err(RegisterAllocationError::Admission),
        Err(error) if error.admits_runtime_recovery() => RetainedAllocation::try_from(
            crate::assignment::runtime_spill::recover_after_selected_lowering(run)
                .map_err(RegisterAllocationError::RuntimeSpill)?,
        )
        .map_err(RegisterAllocationError::Admission),
        Err(error) => Err(RegisterAllocationError::TransformedHomes(
            crate::OptimizedPostSelectedLoweringHomeCustodyError::Assignment(error),
        )),
    }
}

/// A completed pre-allocation run in its composing form: the suite stays
/// the proven prefix — its committed transformation ledger and completion
/// identity record ahead of every recovery step — while residual
/// runtime-recoverable pressure over the run's final facts hands custody to
/// runtime spill instead of failing the route. Any other assignment
/// failure keeps the post-run homes error surface.
pub(crate) fn stage_pre_allocation_register_allocation(
    run: StagedPreAllocationOptimizationRun,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    match crate::assignment::transformed::assign_optimized_register_homes_after_pre_allocation(&run)
    {
        Ok(homes) => RetainedAllocation::try_from(
            crate::assignment::transformed::stage_optimized_register_homes_after_pre_allocation_with_assignment(run, homes)
                .map_err(RegisterAllocationError::PreAllocationHomes)?,
        )
        .map_err(RegisterAllocationError::Admission),
        Err(error) if error.admits_runtime_recovery() => RetainedAllocation::try_from(
            crate::assignment::runtime_spill::recover_after_pre_allocation(run)
                .map_err(RegisterAllocationError::RuntimeSpill)?,
        )
        .map_err(RegisterAllocationError::Admission),
        Err(error) => Err(RegisterAllocationError::PreAllocationHomes(
            crate::OptimizedPostPreAllocationHomeCustodyError::Assignment(error),
        )),
    }
}
