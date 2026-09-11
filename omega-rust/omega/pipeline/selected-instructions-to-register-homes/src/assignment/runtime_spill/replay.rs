use super::recovery::{
    analyze, assign, assign_source, candidates, overlaps_pressure, transformations,
};
use super::{RuntimeSpillAllocation, RuntimeSpillAllocationError};
use crate::SelectedProgramRef;

pub(super) fn inadmissible(error: &crate::RuntimeSpillError) -> bool {
    matches!(
        error,
        crate::RuntimeSpillError::UnsupportedValue
            | crate::RuntimeSpillError::UnsupportedUse
            | crate::RuntimeSpillError::UnsupportedControlFlow
    )
}

pub(crate) fn validate(staged: &RuntimeSpillAllocation) -> Result<(), RuntimeSpillAllocationError> {
    let source = &staged.source;
    let upstream = crate::validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(RuntimeSpillAllocationError::Upstream)?;
    let mut failure = require_pressure(assign_source(source))?;
    let selected_stage = source.live_range_stage().liveness_stage().selected_stage();
    let environment = selected_stage.register_environment();
    let budget = selected_stage
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let roster = candidates(source);
    let mut used = Vec::new();
    let mut current_ranges = source.live_range_stage().ranges().clone();
    let mut current_liveness = source
        .live_range_stage()
        .liveness_stage()
        .liveness()
        .clone();
    let mut prior: Option<crate::ValidatedRuntimeSpill> = None;
    for (step_index, step) in staged.steps.iter().enumerate() {
        let position = roster
            .iter()
            .position(|candidate| *candidate == (step.function, step.register))
            .ok_or(RuntimeSpillAllocationError::CandidateMismatch)?;
        if used.contains(&position) {
            return Err(RuntimeSpillAllocationError::CandidateMismatch);
        }
        if let Some(previous) = &prior {
            // Earlier steps have already been independently replayed. Their
            // source is only an equality prerequisite for candidate fact reuse.
            let previous_source = step_index.checked_sub(2).map_or_else(
                || SelectedProgramRef::new(selected_stage.selected()),
                |source_index| SelectedProgramRef::new(&staged.steps[source_index].rewrite),
            );
            let facts = analyze(
                source,
                &previous_source,
                &current_liveness,
                &current_ranges,
                previous,
            )?;
            failure = require_pressure(assign(source, &facts.ranges, &facts.legality))?;
            current_ranges = facts.ranges;
            current_liveness = facts.liveness;
        }
        if !overlaps_pressure(&failure, &current_ranges, step.function, step.register) {
            return Err(RuntimeSpillAllocationError::CandidateMismatch);
        }
        let selected = prior.as_ref().map_or_else(
            || SelectedProgramRef::new(selected_stage.selected()),
            SelectedProgramRef::new,
        );
        let replayed = crate::validate_runtime_spill(
            &selected,
            step.function,
            step.register,
            environment,
            budget,
            step.rewrite.transformed().clone(),
        )
        .map_err(RuntimeSpillAllocationError::Rewrite)?;
        if replayed.receipt() != step.rewrite.receipt() {
            return Err(RuntimeSpillAllocationError::ReceiptMismatch);
        }
        prior = Some(replayed);
        used.push(position);
    }
    let selected = prior.ok_or(RuntimeSpillAllocationError::CandidateMismatch)?;
    let liveness = crate::validate_liveness(&selected, staged.facts.liveness.plan().clone())
        .map_err(RuntimeSpillAllocationError::Liveness)?;
    let ranges =
        crate::validate_live_ranges(&selected, &liveness, staged.facts.ranges.plan().clone())
            .map_err(RuntimeSpillAllocationError::Ranges)?;
    let legality = crate::validate_allocation_legality(
        &ranges,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        staged.facts.legality.plan().clone(),
    )
    .map_err(RuntimeSpillAllocationError::Legality)?;
    let homes = crate::validate_register_homes(
        &legality,
        &ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        staged.homes.plan().clone(),
    )
    .map_err(RuntimeSpillAllocationError::Homes)?;
    let manifest = crate::validate_post_allocation_optimization_manifest(
        staged.manifest.record(),
        upstream.manifest(),
        &transformations(&staged.steps),
        &ranges,
        &legality,
        &homes,
    )
    .map_err(RuntimeSpillAllocationError::Manifest)?;
    if liveness != staged.facts.liveness
        || ranges != staged.facts.ranges
        || legality != staged.facts.legality
        || homes != staged.homes
        || manifest != staged.manifest
    {
        return Err(RuntimeSpillAllocationError::ReceiptMismatch);
    }
    Ok(())
}

fn require_pressure(
    result: Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError>,
) -> Result<crate::RegisterHomeError, RuntimeSpillAllocationError> {
    match result {
        Err(error @ crate::RegisterHomeError::NoCompatibleHome { .. }) => Ok(error),
        Err(error) => Err(RuntimeSpillAllocationError::Homes(error)),
        Ok(_) => Err(RuntimeSpillAllocationError::RecoveryNotRequired),
    }
}
