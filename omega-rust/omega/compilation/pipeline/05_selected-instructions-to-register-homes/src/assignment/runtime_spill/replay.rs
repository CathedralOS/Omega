use super::recovery::{
    admissible_split, analyze, assign, candidates, overlaps_pressure, retire_minted,
    transformations,
};
use super::{RuntimeSpillAllocation, RuntimeSpillAllocationError, RuntimeSpillStepRewrite};
use selected_instructions::VirtualRegisterId;
use selected_instructions_to_selected_instructions::{
    RuntimeSpillError, RuntimeSpillSpanPolicy, SelectedProgramRef,
    rematerialize_selected_runtime_value, spill_selected_runtime_value_with_span_policy,
    validate_allocation_legality, validate_live_ranges, validate_liveness,
    validate_runtime_rematerialization, validate_runtime_spill,
    validate_runtime_spill_with_span_policy, validate_runtime_split,
};

pub(super) fn inadmissible(error: &RuntimeSpillError) -> bool {
    matches!(
        error,
        RuntimeSpillError::UnsupportedValue
            | RuntimeSpillError::UnsupportedUse
            | RuntimeSpillError::UnsupportedControlFlow
    )
}

pub(crate) fn validate(staged: &RuntimeSpillAllocation) -> Result<(), RuntimeSpillAllocationError> {
    let source = &staged.source;
    let base = source.upstream_manifest()?;
    let environment = source.register_environment();
    let mut failure = require_pressure(assign(environment, source.ranges(), source.legality()))?;
    let budget = source.budget_per_pass();
    let mut used: std::collections::BTreeSet<(usize, VirtualRegisterId)> =
        std::collections::BTreeSet::new();
    let mut current_ranges = source.ranges().clone();
    let mut current_liveness = source.liveness().clone();
    let mut current_legality = source.legality().clone();
    let mut prior: Option<RuntimeSpillStepRewrite> = None;
    for (step_index, step) in staged.steps.iter().enumerate() {
        if used.contains(&(step.function, step.register)) {
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
                &current_legality,
                &previous.selected(),
            )?;
            failure = require_pressure(assign(environment, &facts.ranges, &facts.legality))?;
            current_ranges = facts.ranges;
            current_legality = facts.legality;
            current_liveness = facts.liveness;
        }
        if !overlaps_pressure(&failure, &current_ranges, step.function, step.register) {
            return Err(RuntimeSpillAllocationError::CandidateMismatch);
        }
        let selected = prior
            .as_ref()
            .map_or_else(|| source.base(), |previous| previous.selected());
        // The candidate pool rebuilds against the current plan exactly the
        // way the producer's does: a split's fresh register joins it,
        // consumed victims and spill or rematerialization reloads leave it,
        // and the recorded victim must name a still-live member.
        let roster: Vec<(usize, VirtualRegisterId)> = candidates(selected.plan())
            .into_iter()
            .filter(|entry| !used.contains(entry))
            .collect();
        if !roster
            .iter()
            .any(|candidate| *candidate == (step.function, step.register))
        {
            return Err(RuntimeSpillAllocationError::CandidateMismatch);
        }
        // The cost order is replayed, not trusted: the producer commits the
        // first admissible split over the still-available candidates before
        // it considers any other decision, so a recorded split must be that
        // exact candidate and every other step requires that none existed.
        let expected_split = admissible_split(
            &failure,
            roster.iter(),
            &current_ranges,
            &selected,
            environment,
            budget,
        )?
        .map(|(candidate, _)| candidate);
        if !matches!(step.rewrite, RuntimeSpillStepRewrite::Split(_)) && expected_split.is_some() {
            return Err(RuntimeSpillAllocationError::CandidateMismatch);
        }
        let replayed = match &step.rewrite {
            RuntimeSpillStepRewrite::Split(rewrite) => {
                if expected_split != Some((step.function, step.register)) {
                    return Err(RuntimeSpillAllocationError::CandidateMismatch);
                }
                let replayed = validate_runtime_split(
                    &selected,
                    step.function,
                    step.register,
                    environment,
                    budget,
                    rewrite.transformed().clone(),
                )
                .map_err(RuntimeSpillAllocationError::Split)?;
                if replayed.receipt() != rewrite.receipt() {
                    return Err(RuntimeSpillAllocationError::ReceiptMismatch);
                }
                RuntimeSpillStepRewrite::Split(replayed)
            }
            RuntimeSpillStepRewrite::Spill(rewrite) => {
                // The producer's decision is replayed, not trusted: private
                // storage may stand only while rematerialization remains
                // inadmissible for the same pressured value.
                if rematerialize_selected_runtime_value(
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
                let expects_crossing = match spill_selected_runtime_value_with_span_policy(
                    &selected,
                    step.function,
                    step.register,
                    environment,
                    budget,
                    RuntimeSpillSpanPolicy::UnitWriteCrossing,
                ) {
                    Ok(crossing) => {
                        let probe = analyze(
                            environment,
                            source.allocator_availability(),
                            &selected,
                            &current_liveness,
                            &current_ranges,
                            &current_legality,
                            &SelectedProgramRef::new(&crossing),
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
                    let replayed = validate_runtime_spill_with_span_policy(
                        &selected,
                        step.function,
                        step.register,
                        environment,
                        budget,
                        rewrite.transformed().clone(),
                        RuntimeSpillSpanPolicy::UnitWriteCrossing,
                    )
                    .map_err(RuntimeSpillAllocationError::Rewrite)?;
                    if replayed.receipt() != rewrite.receipt() {
                        return Err(RuntimeSpillAllocationError::ReceiptMismatch);
                    }
                    RuntimeSpillStepRewrite::Spill(replayed)
                } else {
                    let replayed = validate_runtime_spill(
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
                let replayed = validate_runtime_rematerialization(
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
        if !matches!(replayed, RuntimeSpillStepRewrite::Split(_)) {
            retire_minted(&mut used, selected.plan(), replayed.selected().plan());
        }
        prior = Some(replayed);
        used.insert((step.function, step.register));
    }
    let final_rewrite = prior.ok_or(RuntimeSpillAllocationError::CandidateMismatch)?;
    let selected = final_rewrite.selected();
    let liveness = validate_liveness(&selected, staged.facts.liveness.plan().clone())
        .map_err(RuntimeSpillAllocationError::Liveness)?;
    let ranges = validate_live_ranges(&selected, &liveness, staged.facts.ranges.plan().clone())
        .map_err(RuntimeSpillAllocationError::Ranges)?;
    let legality = validate_allocation_legality(
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
    let manifest = crate::assignment::post_allocation_manifest::validate_post_allocation_optimization_manifest_with_suite_completions(
        staged.manifest.record(),
        base.pre_physical,
        base.selected_lowering_completion,
        base.pre_allocation_completion,
        &transformations(&base.prefix, &staged.steps),
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
