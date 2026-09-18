use super::model::RuntimeSpillStepRewrite;
use super::recovery::{analyze, assign, candidates, overlaps_pressure, transformations};
use super::{RuntimeSpillAllocation, RuntimeSpillAllocationError};

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
    let (upstream_manifest, prefix) = source.upstream_manifest()?;
    let environment = source.register_environment();
    let mut failure = require_pressure(assign(environment, source.ranges(), source.legality()))?;
    let budget = source.optimized_target().optimized().budget_per_pass();
    let roster = candidates(source.base().plan());
    let mut used = Vec::new();
    let mut current_ranges = source.ranges().clone();
    let mut current_liveness = source.liveness().clone();
    let mut prior: Option<RuntimeSpillStepRewrite> = None;
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
                || source.base(),
                |source_index| staged.steps[source_index].rewrite.selected(),
            );
            let facts = analyze(
                environment,
                source.allocator_availability(),
                &previous_source,
                &current_liveness,
                &current_ranges,
                &previous.selected(),
            )?;
            failure = require_pressure(assign(environment, &facts.ranges, &facts.legality))?;
            current_ranges = facts.ranges;
            current_liveness = facts.liveness;
        }
        if !overlaps_pressure(&failure, &current_ranges, step.function, step.register) {
            return Err(RuntimeSpillAllocationError::CandidateMismatch);
        }
        let selected = prior
            .as_ref()
            .map_or_else(|| source.base(), |previous| previous.selected());
        let replayed = match &step.rewrite {
            RuntimeSpillStepRewrite::Spill(rewrite) => {
                // The producer's decision is replayed, not trusted: private
                // storage may stand only while rematerialization remains
                // inadmissible for the same pressured value.
                if crate::rematerialize_selected_runtime_value(
                    &selected,
                    step.function,
                    step.register,
                    environment,
                    budget,
                )
                .is_ok()
                {
                    return Err(RuntimeSpillAllocationError::CandidateMismatch);
                }
                // The producer attempted the call-surviving shape first and
                // committed it — ending recovery — exactly when its produced
                // plan assigned. Replay re-derives that decision on the same
                // inputs: a crossing step must be the last one and validate
                // under the crossing policy, while every other spill is the
                // bounded fallback.
                let expects_crossing = match crate::spill_selected_runtime_value_with_span_policy(
                    &selected,
                    step.function,
                    step.register,
                    environment,
                    budget,
                    crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
                ) {
                    Ok(crossing) => {
                        let probe = analyze(
                            environment,
                            source.allocator_availability(),
                            &selected,
                            &current_liveness,
                            &current_ranges,
                            &crate::SelectedProgramRef::new(&crossing),
                        )?;
                        match assign(environment, &probe.ranges, &probe.legality) {
                            Ok(_) => true,
                            Err(crate::RegisterHomeError::NoCompatibleHome { .. }) => false,
                            Err(error) => {
                                return Err(RuntimeSpillAllocationError::Homes(error));
                            }
                        }
                    }
                    Err(_) => false,
                };
                if expects_crossing {
                    if step_index + 1 != staged.steps.len() {
                        return Err(RuntimeSpillAllocationError::CandidateMismatch);
                    }
                    let replayed = crate::validate_runtime_spill_with_span_policy(
                        &selected,
                        step.function,
                        step.register,
                        environment,
                        budget,
                        rewrite.transformed().clone(),
                        crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
                    )
                    .map_err(RuntimeSpillAllocationError::Rewrite)?;
                    if replayed.receipt() != rewrite.receipt() {
                        return Err(RuntimeSpillAllocationError::ReceiptMismatch);
                    }
                    RuntimeSpillStepRewrite::Spill(replayed)
                } else {
                    let replayed = crate::validate_runtime_spill(
                        &selected,
                        step.function,
                        step.register,
                        environment,
                        budget,
                        rewrite.transformed().clone(),
                    )
                    .map_err(RuntimeSpillAllocationError::Rewrite)?;
                    if replayed.receipt() != rewrite.receipt() {
                        return Err(RuntimeSpillAllocationError::ReceiptMismatch);
                    }
                    RuntimeSpillStepRewrite::Spill(replayed)
                }
            }
            RuntimeSpillStepRewrite::Rematerialization(rewrite) => {
                let replayed = crate::validate_runtime_rematerialization(
                    &selected,
                    step.function,
                    step.register,
                    environment,
                    budget,
                    rewrite.transformed().clone(),
                )
                .map_err(RuntimeSpillAllocationError::Rematerialization)?;
                if replayed.receipt() != rewrite.receipt() {
                    return Err(RuntimeSpillAllocationError::ReceiptMismatch);
                }
                RuntimeSpillStepRewrite::Rematerialization(replayed)
            }
        };
        prior = Some(replayed);
        used.push(position);
    }
    let final_rewrite = prior.ok_or(RuntimeSpillAllocationError::CandidateMismatch)?;
    let selected = final_rewrite.selected();
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
        upstream_manifest,
        &transformations(&prefix, &staged.steps),
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
