//! Literal-fold execution, fixed-point iteration, and independent replay.

use super::accounting::*;
use super::*;

#[allow(clippy::too_many_arguments)]
pub fn stage_first_optimized_literal_fold(
    source: StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    recovery_policy: RecoveryClassificationPolicy,
    fold_policy: LiteralFoldPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedLiteralFolds, OptimizedLiteralFoldCustodyError> {
    let upstream = validate_source(&source)?;
    let selected = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let step = build_step(
        selected,
        source.live_range_stage().ranges(),
        source.legality(),
        &source,
        choice_policy,
        recovery_policy,
        fold_policy,
        budget,
    )?;
    let custody = custody_receipt(upstream, std::slice::from_ref(&step));
    Ok(StagedOptimizedLiteralFolds {
        source,
        steps: vec![step],
        custody,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn stage_next_optimized_literal_fold(
    mut sequence: StagedOptimizedLiteralFolds,
    choice_policy: SpillChoicePolicy,
    recovery_policy: RecoveryClassificationPolicy,
    fold_policy: LiteralFoldPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedLiteralFolds, OptimizedLiteralFoldCustodyError> {
    validate_optimized_literal_fold_custody(&sequence)?;
    let previous = sequence.final_step();
    let step = build_step(
        previous.fold(),
        previous.ranges(),
        previous.legality(),
        &sequence.source,
        choice_policy,
        recovery_policy,
        fold_policy,
        budget,
    )?;
    sequence.steps.push(step);
    let upstream = validate_source(&sequence.source)?;
    sequence.custody = custody_receipt(upstream, &sequence.steps);
    Ok(sequence)
}

pub(super) fn execute_selected_lowering_optimizations(
    source: StagedOptimizedAllocationLegality,
    selections: OptimizationSelections,
    selected_lowering_selections: OptimizationSelections,
    schedule: SelectedLoweringOptimizationSchedule,
    fold_policy: LiteralFoldPolicy,
) -> Result<StagedSelectedLoweringOptimizationRun, OptimizedLiteralFoldCustodyError> {
    let upstream = validate_source(&source)?;
    let budget = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .budget_per_pass();

    let choice_policy = SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1;
    let recovery_policy = RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1;
    let iteration_bound = source.legality().receipt().virtual_register_count();
    let selected = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let mut attempt = build_attempt(
        selected,
        source.live_range_stage().ranges(),
        source.legality(),
        &source,
        choice_policy,
        recovery_policy,
        fold_policy,
        budget,
    )?;
    let mut previous_measure = iteration_bound;
    let mut steps = Vec::new();
    let mut usage = attempt_usage(&attempt)?;
    ensure_selected_lowering_budget(usage, budget)?;
    loop {
        if attempt.fold.receipt().applied_count() == 0 {
            break;
        }
        if steps.len() >= iteration_bound {
            return Err(
                OptimizedLiteralFoldCustodyError::SelectedLoweringIterationBoundExceeded {
                    bound: iteration_bound,
                },
            );
        }
        let step = complete_attempt(attempt, &source)?;
        let current_measure = step.legality.receipt().virtual_register_count();
        let applied = step.fold.receipt().applied_count();
        if previous_measure.checked_sub(applied) != Some(current_measure) {
            return Err(
                OptimizedLiteralFoldCustodyError::SelectedLoweringMeasureMismatch {
                    previous: previous_measure,
                    applied,
                    current: current_measure,
                },
            );
        }
        previous_measure = current_measure;
        steps.push(step);
        let previous = steps.last().expect("applied selected-lowering step exists");
        attempt = build_attempt(
            previous.fold(),
            previous.ranges(),
            previous.legality(),
            &source,
            choice_policy,
            recovery_policy,
            fold_policy,
            budget,
        )?;
        usage = add_usage(usage, attempt_usage(&attempt)?)?;
        ensure_selected_lowering_budget(usage, budget)?;
    }
    if attempt.fold.receipt().source_selected() != attempt.fold.receipt().transformed_selected() {
        return Err(OptimizedLiteralFoldCustodyError::TerminalAttemptApplied);
    }

    let action_count = applied_action_count(&steps)?;
    let custody = selected_lowering_custody_receipt(
        upstream,
        &selections,
        &selected_lowering_selections,
        &source,
        &steps,
        &attempt,
        budget,
        usage,
        iteration_bound,
        action_count,
        schedule,
    );
    Ok(StagedSelectedLoweringOptimizationRun {
        source,
        selections,
        selected_lowering_selections,
        steps,
        attempt: attempt,
        custody,
    })
}

pub fn validate_selected_lowering_optimization_custody(
    run: &StagedSelectedLoweringOptimizationRun,
) -> Result<StagedSelectedLoweringOptimizationCustodyReceipt, OptimizedLiteralFoldCustodyError> {
    let upstream = validate_source(&run.source)?;
    let optimized = run
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized();
    let expected_budget = optimized.budget_per_pass();
    let (projected, fold_policy) = selected_lowering_rule_policy(&run.selections)?;
    let expected_schedule = selected_lowering_schedule(fold_policy);
    if run.selections != *optimized.selections()
        || run.custody.selections != optimized.selections().identity()
        || run.custody.budget != expected_budget
        || run.custody.schedule != expected_schedule
    {
        return Err(OptimizedLiteralFoldCustodyError::SelectionProjectionMismatch);
    }
    if projected != run.selected_lowering_selections {
        return Err(OptimizedLiteralFoldCustodyError::SelectionProjectionMismatch);
    }
    for step in &run.steps {
        validate_selected_lowering_schedule(
            step.choices(),
            step.recovery(),
            step.fold(),
            expected_budget,
            fold_policy,
        )?;
    }
    validate_selected_lowering_schedule(
        run.attempt.choices(),
        run.attempt.recovery(),
        run.attempt.fold(),
        expected_budget,
        fold_policy,
    )?;
    let selected = run
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let mut replayed = Vec::with_capacity(run.steps.len());
    if let Some(first) = run.steps.first() {
        replayed.push(replay_step(
            0,
            selected,
            run.source.live_range_stage().ranges(),
            run.source.legality(),
            &run.source,
            first,
        )?);
        for step_index in 1..run.steps.len() {
            let previous = replayed.last().expect("first replayed step exists");
            replayed.push(replay_step(
                step_index,
                previous.fold(),
                previous.ranges(),
                previous.legality(),
                &run.source,
                &run.steps[step_index],
            )?);
        }
    }
    let terminal = match replayed.last() {
        Some(previous) => build_attempt(
            previous.fold(),
            previous.ranges(),
            previous.legality(),
            &run.source,
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            fold_policy,
            expected_budget,
        )?,
        None => build_attempt(
            selected,
            run.source.live_range_stage().ranges(),
            run.source.legality(),
            &run.source,
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            fold_policy,
            expected_budget,
        )?,
    };
    if terminal.fold.receipt().applied_count() != 0 {
        return Err(OptimizedLiteralFoldCustodyError::TerminalAttemptApplied);
    }
    if terminal.fold.receipt().source_selected() != terminal.fold.receipt().transformed_selected() {
        return Err(OptimizedLiteralFoldCustodyError::TerminalAttemptApplied);
    }
    let mut previous_measure = run.source.legality().receipt().virtual_register_count();
    for step in &replayed {
        let current_measure = step.legality.receipt().virtual_register_count();
        let applied = step.fold.receipt().applied_count();
        if previous_measure.checked_sub(applied) != Some(current_measure) {
            return Err(
                OptimizedLiteralFoldCustodyError::SelectedLoweringMeasureMismatch {
                    previous: previous_measure,
                    applied,
                    current: current_measure,
                },
            );
        }
        previous_measure = current_measure;
    }
    let mut usage = OptimizationWorkUsage::default();
    for step in &replayed {
        usage = add_usage(usage, step_usage(step)?)?;
    }
    usage = add_usage(usage, attempt_usage(&terminal)?)?;
    ensure_selected_lowering_budget(usage, expected_budget)?;
    let action_count = applied_action_count(&replayed)?;
    let receipt = selected_lowering_custody_receipt(
        upstream,
        &run.selections,
        &run.selected_lowering_selections,
        &run.source,
        &replayed,
        &terminal,
        expected_budget,
        usage,
        run.source.legality().receipt().virtual_register_count(),
        action_count,
        expected_schedule,
    );
    if replayed != run.steps || terminal != run.attempt || receipt != run.custody {
        return Err(OptimizedLiteralFoldCustodyError::StepMismatch { step: 0 });
    }
    Ok(receipt)
}

pub fn validate_optimized_literal_fold_custody(
    sequence: &StagedOptimizedLiteralFolds,
) -> Result<StagedOptimizedLiteralFoldCustodyReceipt, OptimizedLiteralFoldCustodyError> {
    let upstream = validate_source(&sequence.source)?;
    let Some(first) = sequence.steps.first() else {
        return Err(OptimizedLiteralFoldCustodyError::EmptySequence);
    };
    let selected = sequence
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let mut replayed = vec![replay_step(
        0,
        selected,
        sequence.source.live_range_stage().ranges(),
        sequence.source.legality(),
        &sequence.source,
        first,
    )?];
    for step_index in 1..sequence.steps.len() {
        let previous = replayed.last().expect("first replayed step exists");
        let step = replay_step(
            step_index,
            previous.fold(),
            previous.ranges(),
            previous.legality(),
            &sequence.source,
            &sequence.steps[step_index],
        )?;
        replayed.push(step);
    }
    let receipt = custody_receipt(upstream, &replayed);
    if replayed != sequence.steps || receipt != sequence.custody {
        return Err(OptimizedLiteralFoldCustodyError::StepMismatch { step: 0 });
    }
    Ok(receipt)
}

fn validate_source(
    source: &StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedAllocationLegalityCustodyReceipt, OptimizedLiteralFoldCustodyError> {
    validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(OptimizedLiteralFoldCustodyError::UpstreamLegality)
}

#[allow(clippy::too_many_arguments)]
fn build_step<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    source: &StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    recovery_policy: RecoveryClassificationPolicy,
    fold_policy: LiteralFoldPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedLiteralFoldStep, OptimizedLiteralFoldCustodyError> {
    let attempt = build_attempt(
        selected,
        ranges,
        legality,
        source,
        choice_policy,
        recovery_policy,
        fold_policy,
        budget,
    )?;
    if attempt.fold.receipt().applied_count() == 0 {
        return Err(OptimizedLiteralFoldCustodyError::NoAppliedFold);
    }
    complete_attempt(attempt, source)
}

#[allow(clippy::too_many_arguments)]
fn build_attempt<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    source: &StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    recovery_policy: RecoveryClassificationPolicy,
    fold_policy: LiteralFoldPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedLiteralFoldAttempt, OptimizedLiteralFoldCustodyError> {
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let choices = choose_spill_victims(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        choice_policy,
        budget,
    )
    .map_err(OptimizedLiteralFoldCustodyError::SpillChoice)?;
    let recovery = classify_pressure_recovery(
        selected,
        ranges,
        legality,
        &choices,
        recovery_policy,
        budget,
    )
    .map_err(OptimizedLiteralFoldCustodyError::RecoveryClassification)?;
    let fold = fold_selected_incoming_literal(
        selected,
        ranges,
        legality,
        &choices,
        &recovery,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        fold_policy,
        budget,
    )
    .map_err(OptimizedLiteralFoldCustodyError::Fold)?;
    Ok(StagedOptimizedLiteralFoldAttempt {
        choices,
        recovery,
        fold,
    })
}

fn complete_attempt(
    attempt: StagedOptimizedLiteralFoldAttempt,
    source: &StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedLiteralFoldStep, OptimizedLiteralFoldCustodyError> {
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let StagedOptimizedLiteralFoldAttempt {
        choices,
        recovery,
        fold,
    } = attempt;
    let liveness = analyze_liveness(&fold).map_err(OptimizedLiteralFoldCustodyError::Liveness)?;
    let ranges = analyze_live_ranges(&fold, &liveness)
        .map_err(OptimizedLiteralFoldCustodyError::LiveRanges)?;
    let legality = analyze_allocation_legality(
        &ranges,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedLiteralFoldCustodyError::AllocationLegality)?;
    let transition_count = legality.receipt().entry_transition_count();
    if transition_count != 0 {
        return Err(OptimizedLiteralFoldCustodyError::RemainingTransitions {
            count: transition_count,
        });
    }
    Ok(StagedOptimizedLiteralFoldStep {
        choices,
        recovery,
        fold,
        liveness,
        ranges,
        legality,
    })
}

fn replay_step<S: ValidatedSelectedAnalysis>(
    step_index: usize,
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    source: &StagedOptimizedAllocationLegality,
    expected: &StagedOptimizedLiteralFoldStep,
) -> Result<StagedOptimizedLiteralFoldStep, OptimizedLiteralFoldCustodyError> {
    let replayed = build_step(
        selected,
        ranges,
        legality,
        source,
        expected.choices.receipt().policy(),
        expected.recovery.receipt().policy(),
        expected.fold.receipt().policy(),
        expected.fold.plan().budget,
    )?;
    if replayed != *expected {
        return Err(OptimizedLiteralFoldCustodyError::StepMismatch { step: step_index });
    }
    Ok(replayed)
}
