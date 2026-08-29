//! Receipt identity, work accounting, and schedule validation.

use super::*;

pub(super) fn custody_receipt(
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    steps: &[StagedOptimizedLiteralFoldStep],
) -> StagedOptimizedLiteralFoldCustodyReceipt {
    let final_step = steps.last().expect("literal-fold custody is nonempty");
    StagedOptimizedLiteralFoldCustodyReceipt {
        source,
        iterations: steps.iter().map(iteration_receipt).collect(),
        transformations: steps
            .iter()
            .map(|step| step.fold.receipt().identity())
            .collect(),
        final_selected: final_step.fold.receipt().transformed_selected(),
        final_liveness: final_step.liveness.receipt().identity(),
        final_ranges: final_step.ranges.receipt().identity(),
        final_legality: final_step.legality.receipt().identity(),
        final_virtual_register_count: final_step.legality.receipt().virtual_register_count(),
        final_entry_transition_count: final_step.legality.receipt().entry_transition_count(),
    }
}

pub(super) fn iteration_receipt(
    step: &StagedOptimizedLiteralFoldStep,
) -> StagedOptimizedLiteralFoldIterationReceipt {
    StagedOptimizedLiteralFoldIterationReceipt {
        source_selected: step.fold.plan().source_selected,
        source_ranges: step.fold.plan().ranges,
        source_legality: step.fold.plan().legality,
        choices: step.choices.receipt().identity(),
        choice_policy: step.choices.receipt().policy(),
        choice_usage: step.choices.receipt().usage(),
        recovery: step.recovery.receipt().identity(),
        recovery_policy: step.recovery.receipt().policy(),
        recovery_usage: step.recovery.receipt().usage(),
        fold: step.fold.receipt().identity(),
        fold_policy: step.fold.receipt().policy(),
        fold_usage: step.fold.receipt().usage(),
        transformed_selected: step.fold.receipt().transformed_selected(),
        fresh_liveness: step.liveness.receipt().identity(),
        fresh_ranges: step.ranges.receipt().identity(),
        fresh_legality: step.legality.receipt().identity(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn selected_lowering_custody_receipt(
    source_receipt: StagedOptimizedAllocationLegalityCustodyReceipt,
    selections: &OptimizationSelections,
    selected_lowering_selections: &OptimizationSelections,
    source: &StagedOptimizedAllocationLegality,
    steps: &[StagedOptimizedLiteralFoldStep],
    attempt: &StagedOptimizedLiteralFoldAttempt,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    iteration_bound: usize,
    action_count: usize,
) -> StagedSelectedLoweringOptimizationCustodyReceipt {
    let (final_selected, final_liveness, final_ranges, final_legality) = match steps.last() {
        Some(step) => (
            step.fold.receipt().transformed_selected(),
            step.liveness.receipt().identity(),
            step.ranges.receipt().identity(),
            step.legality.receipt().identity(),
        ),
        None => (
            source
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .receipt()
                .identity(),
            source
                .live_range_stage()
                .liveness_stage()
                .liveness()
                .receipt()
                .identity(),
            source.live_range_stage().ranges().receipt().identity(),
            source.legality().receipt().identity(),
        ),
    };
    let mut receipt = StagedSelectedLoweringOptimizationCustodyReceipt {
        identity: SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"pending"),
        source: source_receipt,
        selections: selections.identity(),
        selected_lowering_selections: selected_lowering_selections.identity(),
        budget,
        usage,
        iteration_bound,
        action_count,
        initial_virtual_register_count: source.legality().receipt().virtual_register_count(),
        iterations: steps.iter().map(iteration_receipt).collect(),
        attempt: attempt_receipt(attempt),
        final_selected,
        final_liveness,
        final_ranges,
        final_legality,
        final_virtual_register_count: steps
            .last()
            .map(|step| step.legality.receipt().virtual_register_count())
            .unwrap_or_else(|| source.legality().receipt().virtual_register_count()),
    };
    receipt.identity = selected_lowering_completion_identity(&receipt);
    receipt
}

pub(super) fn selected_lowering_completion_identity(
    receipt: &StagedSelectedLoweringOptimizationCustodyReceipt,
) -> SelectedLoweringOptimizationCompletionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.selected-lowering-optimization-completion.v3\0");
    let source = receipt.source;
    for identity in [
        source.optimization().bytes(),
        source.manifest().bytes(),
        source.register_environment().bytes(),
        source.allocator_availability().bytes(),
        source.selected().bytes(),
        source.liveness().bytes(),
        source.ranges().bytes(),
        source.legality().bytes(),
        receipt.selections.bytes(),
        receipt.selected_lowering_selections.bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    canonical.extend_from_slice(&receipt.budget.encode());
    canonical.extend_from_slice(&receipt.usage.encode());
    for count in [
        receipt.iteration_bound,
        receipt.action_count,
        receipt.initial_virtual_register_count,
        receipt.iterations.len(),
    ] {
        encode_count(&mut canonical, count);
    }
    for iteration in &receipt.iterations {
        encode_iteration_receipt(&mut canonical, *iteration);
    }
    encode_attempt_receipt(&mut canonical, receipt.attempt);
    canonical.extend_from_slice(&receipt.final_selected.bytes());
    canonical.extend_from_slice(&receipt.final_liveness.bytes());
    canonical.extend_from_slice(&receipt.final_ranges.bytes());
    canonical.extend_from_slice(&receipt.final_legality.bytes());
    encode_count(&mut canonical, receipt.final_virtual_register_count);
    SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(&canonical)
}

pub(super) fn encode_iteration_receipt(
    canonical: &mut Vec<u8>,
    iteration: StagedOptimizedLiteralFoldIterationReceipt,
) {
    canonical.extend_from_slice(&iteration.source_selected().bytes());
    canonical.extend_from_slice(&iteration.source_ranges().bytes());
    canonical.extend_from_slice(&iteration.source_legality().bytes());
    canonical.extend_from_slice(&iteration.choices().bytes());
    canonical.push(spill_choice_policy_tag(iteration.choice_policy()));
    canonical.extend_from_slice(&iteration.choice_usage().encode());
    canonical.extend_from_slice(&iteration.recovery().bytes());
    canonical.push(recovery_policy_tag(iteration.recovery_policy()));
    canonical.extend_from_slice(&iteration.recovery_usage().encode());
    canonical.extend_from_slice(&iteration.fold().bytes());
    canonical.push(literal_fold_policy_tag(iteration.fold_policy()));
    canonical.extend_from_slice(&iteration.fold_usage().encode());
    canonical.extend_from_slice(&iteration.transformed_selected().bytes());
    canonical.extend_from_slice(&iteration.fresh_liveness().bytes());
    canonical.extend_from_slice(&iteration.fresh_ranges().bytes());
    canonical.extend_from_slice(&iteration.fresh_legality().bytes());
}

pub(super) fn encode_attempt_receipt(
    canonical: &mut Vec<u8>,
    attempt: StagedOptimizedLiteralFoldAttemptReceipt,
) {
    canonical.extend_from_slice(&attempt.source_selected().bytes());
    canonical.extend_from_slice(&attempt.source_ranges().bytes());
    canonical.extend_from_slice(&attempt.source_legality().bytes());
    canonical.extend_from_slice(&attempt.choices().bytes());
    canonical.push(spill_choice_policy_tag(attempt.choice_policy()));
    canonical.extend_from_slice(&attempt.choice_usage().encode());
    canonical.extend_from_slice(&attempt.recovery().bytes());
    canonical.push(recovery_policy_tag(attempt.recovery_policy()));
    canonical.extend_from_slice(&attempt.recovery_usage().encode());
    canonical.extend_from_slice(&attempt.fold().bytes());
    canonical.push(literal_fold_policy_tag(attempt.fold_policy()));
    canonical.extend_from_slice(&attempt.fold_usage().encode());
    encode_count(canonical, attempt.applied_count());
    canonical.extend_from_slice(&attempt.transformed_selected().bytes());
}

pub(super) fn spill_choice_policy_tag(policy: SpillChoicePolicy) -> u8 {
    match policy {
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 => 1,
    }
}

pub(super) fn recovery_policy_tag(policy: RecoveryClassificationPolicy) -> u8 {
    match policy {
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 => 1,
    }
}

pub(super) fn literal_fold_policy_tag(policy: LiteralFoldPolicy) -> u8 {
    policy.canonical_bits()
}

pub(super) fn encode_count(canonical: &mut Vec<u8>, count: usize) {
    canonical.extend_from_slice(
        &u64::try_from(count)
            .expect("selected-lowering completion count fits u64")
            .to_le_bytes(),
    );
}

pub(super) fn attempt_receipt(
    attempt: &StagedOptimizedLiteralFoldAttempt,
) -> StagedOptimizedLiteralFoldAttemptReceipt {
    StagedOptimizedLiteralFoldAttemptReceipt {
        source_selected: attempt.fold.receipt().source_selected(),
        source_ranges: attempt.fold.receipt().ranges(),
        source_legality: attempt.fold.receipt().legality(),
        choices: attempt.choices.receipt().identity(),
        choice_policy: attempt.choices.receipt().policy(),
        choice_usage: attempt.choices.receipt().usage(),
        recovery: attempt.recovery.receipt().identity(),
        recovery_policy: attempt.recovery.receipt().policy(),
        recovery_usage: attempt.recovery.receipt().usage(),
        fold: attempt.fold.receipt().identity(),
        fold_policy: attempt.fold.receipt().policy(),
        fold_usage: attempt.fold.receipt().usage(),
        applied_count: attempt.fold.receipt().applied_count(),
        transformed_selected: attempt.fold.receipt().transformed_selected(),
    }
}

pub(super) fn step_usage(
    step: &StagedOptimizedLiteralFoldStep,
) -> Result<OptimizationWorkUsage, OptimizedLiteralFoldCustodyError> {
    let choices_and_recovery = add_usage(
        step.choices.receipt().usage(),
        step.recovery.receipt().usage(),
    )?;
    add_usage(choices_and_recovery, step.fold.receipt().usage())
}

pub(super) fn applied_action_count(
    steps: &[StagedOptimizedLiteralFoldStep],
) -> Result<usize, OptimizedLiteralFoldCustodyError> {
    steps.iter().try_fold(0_usize, |count, step| {
        count
            .checked_add(step.fold.receipt().applied_count())
            .ok_or(OptimizedLiteralFoldCustodyError::WorkOverflow)
    })
}

pub(super) fn attempt_usage(
    attempt: &StagedOptimizedLiteralFoldAttempt,
) -> Result<OptimizationWorkUsage, OptimizedLiteralFoldCustodyError> {
    let choices_and_recovery = add_usage(
        attempt.choices.receipt().usage(),
        attempt.recovery.receipt().usage(),
    )?;
    add_usage(choices_and_recovery, attempt.fold.receipt().usage())
}

pub(super) fn add_usage(
    left: OptimizationWorkUsage,
    right: OptimizationWorkUsage,
) -> Result<OptimizationWorkUsage, OptimizedLiteralFoldCustodyError> {
    Ok(OptimizationWorkUsage {
        rule_evaluations: left
            .rule_evaluations
            .checked_add(right.rule_evaluations)
            .ok_or(OptimizedLiteralFoldCustodyError::WorkOverflow)?,
        candidates: left
            .candidates
            .checked_add(right.candidates)
            .ok_or(OptimizedLiteralFoldCustodyError::WorkOverflow)?,
        validation_steps: left
            .validation_steps
            .checked_add(right.validation_steps)
            .ok_or(OptimizedLiteralFoldCustodyError::WorkOverflow)?,
        commits: left
            .commits
            .checked_add(right.commits)
            .ok_or(OptimizedLiteralFoldCustodyError::WorkOverflow)?,
        iterations: left
            .iterations
            .checked_add(right.iterations)
            .ok_or(OptimizedLiteralFoldCustodyError::WorkOverflow)?,
    })
}

pub(super) fn ensure_selected_lowering_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), OptimizedLiteralFoldCustodyError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(
            OptimizedLiteralFoldCustodyError::SelectedLoweringBudgetExceeded {
                required: usage,
                budget,
            },
        )
    }
}

pub(super) fn validate_selected_lowering_policies(
    choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    fold: &ValidatedLiteralFold,
    budget: OptimizationWorkBudget,
    fold_policy: LiteralFoldPolicy,
) -> Result<(), OptimizedLiteralFoldCustodyError> {
    if choices.receipt().policy() != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
        || recovery.receipt().policy()
            != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1
        || fold.receipt().policy() != fold_policy
        || choices.plan().budget != budget
        || recovery.plan().budget != budget
        || fold.plan().budget != budget
    {
        return Err(OptimizedLiteralFoldCustodyError::SelectionProjectionMismatch);
    }
    Ok(())
}
