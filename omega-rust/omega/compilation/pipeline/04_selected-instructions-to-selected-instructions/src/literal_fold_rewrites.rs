/*
We implement the `SelectedLowering` optimization phase (`literal_folds` and
`selected_lowering`), folding incoming literal materializations into consumer
instructions admitted by the declarative pair-rule catalog and independently
replaying every committed step.
*/

#[allow(unused_imports)]
pub(crate) use crate::rewrites::*;

pub(crate) mod literal_folds {
    use crate::{ValidatedSelectedAnalysis, resolve_selected_lowering_rules};
    use optimization_core::{
        Optimization, OptimizationSelectionIdentity, OptimizationSelections,
        OptimizationWorkBudget, OptimizationWorkUsage,
        SelectedLoweringOptimizationCompletionIdentity,
    };
    use register_homes::{RecoveryClassificationPolicy, SpillChoicePolicy};
    use selected_instructions::SelectedInstructionPlanIdentity;

    use crate::StagedOptimizedAllocationLegality;

    pub(crate) mod accounting {
        use super::{
            OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
            RecoveryClassificationPolicy, SelectedLoweringOptimizationCompletionIdentity,
            SpillChoicePolicy,
        };
        use crate::{
            LiteralFoldPolicy, OptimizedLiteralFoldCustodyError, StagedOptimizedAllocationLegality,
            StagedOptimizedAllocationLegalityCustodyReceipt, StagedOptimizedLiteralFoldAttempt,
            StagedOptimizedLiteralFoldAttemptReceipt, StagedOptimizedLiteralFoldIterationReceipt,
            StagedOptimizedLiteralFoldStep, StagedSelectedLoweringOptimizationCustodyReceipt,
            ValidatedLiteralFold, ValidatedRecoveryClassifications, ValidatedSpillChoices,
        };
        fn fold_evidence(
            choices: &ValidatedSpillChoices,
            recovery: &ValidatedRecoveryClassifications,
            fold: &ValidatedLiteralFold,
        ) -> StagedOptimizedLiteralFoldAttemptReceipt {
            StagedOptimizedLiteralFoldAttemptReceipt {
                source_selected: fold.receipt().source_selected(),
                source_ranges: fold.receipt().ranges(),
                source_legality: fold.receipt().legality(),
                choices: choices.receipt().identity(),
                choice_policy: choices.receipt().policy(),
                choice_usage: choices.receipt().usage(),
                recovery: recovery.receipt().identity(),
                recovery_policy: recovery.receipt().policy(),
                recovery_usage: recovery.receipt().usage(),
                fold: fold.receipt().identity(),
                fold_policy: fold.receipt().policy(),
                fold_usage: fold.receipt().usage(),
                applied_count: fold.receipt().applied_count(),
                transformed_selected: fold.receipt().transformed_selected(),
            }
        }

        pub(crate) fn iteration_receipt(
            step: &StagedOptimizedLiteralFoldStep,
        ) -> StagedOptimizedLiteralFoldIterationReceipt {
            StagedOptimizedLiteralFoldIterationReceipt {
                evidence: fold_evidence(&step.choices, &step.recovery, &step.fold),
                fresh_liveness: step.liveness.receipt().identity(),
                fresh_ranges: step.ranges.receipt().identity(),
                fresh_legality: step.legality.receipt().identity(),
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn selected_lowering_custody_receipt(
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
            let (final_selected, final_liveness, final_ranges, final_legality) = match steps.last()
            {
                Some(step) => (
                    step.fold.receipt().transformed_selected(),
                    step.liveness.receipt().identity(),
                    step.ranges.receipt().identity(),
                    step.legality.receipt().identity(),
                ),
                None => (
                    source.selected().identity(),
                    source.liveness().receipt().identity(),
                    source.ranges().receipt().identity(),
                    source.legality().receipt().identity(),
                ),
            };
            let mut receipt = StagedSelectedLoweringOptimizationCustodyReceipt {
                identity: SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(
                    b"pending",
                ),
                source: source_receipt,
                selections: selections.identity(),
                selected_lowering_selections: selected_lowering_selections.identity(),
                budget,
                usage,
                iteration_bound,
                action_count,
                initial_virtual_register_count: source
                    .legality()
                    .receipt()
                    .virtual_register_count(),
                iterations: steps.iter().map(iteration_receipt).collect(),
                attempt: fold_evidence(&attempt.choices, &attempt.recovery, &attempt.fold),
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

        pub(crate) fn selected_lowering_completion_identity(
            receipt: &StagedSelectedLoweringOptimizationCustodyReceipt,
        ) -> SelectedLoweringOptimizationCompletionIdentity {
            let mut canonical = Vec::new();
            canonical.extend_from_slice(b"omega.selected-lowering-optimization-completion.v5\0");
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

        fn encode_fold_evidence(
            canonical: &mut Vec<u8>,
            evidence: StagedOptimizedLiteralFoldAttemptReceipt,
        ) {
            canonical.extend_from_slice(&evidence.source_selected().bytes());
            canonical.extend_from_slice(&evidence.source_ranges().bytes());
            canonical.extend_from_slice(&evidence.source_legality().bytes());
            canonical.extend_from_slice(&evidence.choices().bytes());
            canonical.push(spill_choice_policy_tag(evidence.choice_policy()));
            canonical.extend_from_slice(&evidence.choice_usage().encode());
            canonical.extend_from_slice(&evidence.recovery().bytes());
            canonical.push(recovery_policy_tag(evidence.recovery_policy()));
            canonical.extend_from_slice(&evidence.recovery_usage().encode());
            canonical.extend_from_slice(&evidence.fold().bytes());
            crate::codec_bytes::push_encoded(
                canonical,
                literal_fold_policy_tag(evidence.fold_policy()).to_le_bytes(),
            );
            canonical.extend_from_slice(&evidence.fold_usage().encode());
        }

        pub(crate) fn encode_iteration_receipt(
            canonical: &mut Vec<u8>,
            iteration: StagedOptimizedLiteralFoldIterationReceipt,
        ) {
            encode_fold_evidence(canonical, iteration.evidence);
            canonical.extend_from_slice(&iteration.transformed_selected().bytes());
            canonical.extend_from_slice(&iteration.fresh_liveness().bytes());
            canonical.extend_from_slice(&iteration.fresh_ranges().bytes());
            canonical.extend_from_slice(&iteration.fresh_legality().bytes());
        }

        pub(crate) fn encode_attempt_receipt(
            canonical: &mut Vec<u8>,
            attempt: StagedOptimizedLiteralFoldAttemptReceipt,
        ) {
            encode_fold_evidence(canonical, attempt);
            encode_count(canonical, attempt.applied_count());
            canonical.extend_from_slice(&attempt.transformed_selected().bytes());
        }

        pub(crate) fn spill_choice_policy_tag(policy: SpillChoicePolicy) -> u8 {
            match policy {
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 => 1,
            }
        }

        pub(crate) fn recovery_policy_tag(policy: RecoveryClassificationPolicy) -> u8 {
            match policy {
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 => 1,
            }
        }

        pub(crate) fn literal_fold_policy_tag(policy: LiteralFoldPolicy) -> u32 {
            policy.canonical_bits()
        }

        pub(crate) fn encode_count(canonical: &mut Vec<u8>, count: usize) {
            canonical.extend_from_slice(
                &u64::try_from(count)
                    .expect("selected-lowering completion count fits u64")
                    .to_le_bytes(),
            );
        }

        pub(crate) fn step_usage(
            step: &StagedOptimizedLiteralFoldStep,
        ) -> Result<OptimizationWorkUsage, OptimizedLiteralFoldCustodyError> {
            let choices_and_recovery = add_usage(
                step.choices.receipt().usage(),
                step.recovery.receipt().usage(),
            )?;
            add_usage(choices_and_recovery, step.fold.receipt().usage())
        }

        pub(crate) fn applied_action_count(
            steps: &[StagedOptimizedLiteralFoldStep],
        ) -> Result<usize, OptimizedLiteralFoldCustodyError> {
            steps.iter().try_fold(0_usize, |count, step| {
                count
                    .checked_add(step.fold.receipt().applied_count())
                    .ok_or(OptimizedLiteralFoldCustodyError::WorkOverflow)
            })
        }

        pub(crate) fn attempt_usage(
            attempt: &StagedOptimizedLiteralFoldAttempt,
        ) -> Result<OptimizationWorkUsage, OptimizedLiteralFoldCustodyError> {
            let choices_and_recovery = add_usage(
                attempt.choices.receipt().usage(),
                attempt.recovery.receipt().usage(),
            )?;
            add_usage(choices_and_recovery, attempt.fold.receipt().usage())
        }

        pub(crate) fn add_usage(
            left: OptimizationWorkUsage,
            right: OptimizationWorkUsage,
        ) -> Result<OptimizationWorkUsage, OptimizedLiteralFoldCustodyError> {
            crate::work_accounting::combine(left, right)
                .map_err(|_| OptimizedLiteralFoldCustodyError::WorkOverflow)
        }

        pub(crate) fn ensure_selected_lowering_budget(
            usage: OptimizationWorkUsage,
            budget: OptimizationWorkBudget,
        ) -> Result<(), OptimizedLiteralFoldCustodyError> {
            crate::work_accounting::admit(usage, budget, |required, budget| {
                OptimizedLiteralFoldCustodyError::SelectedLoweringBudgetExceeded {
                    required,
                    budget,
                }
            })
        }

        pub(crate) fn validate_selected_lowering_policies(
            choices: &ValidatedSpillChoices,
            recovery: &ValidatedRecoveryClassifications,
            fold: &ValidatedLiteralFold,
            budget: OptimizationWorkBudget,
            fold_policy: LiteralFoldPolicy,
        ) -> Result<(), OptimizedLiteralFoldCustodyError> {
            if choices.receipt().policy()
                != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
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
    }
    pub(crate) mod execution {
        use super::{
            OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
            RecoveryClassificationPolicy, SpillChoicePolicy, ValidatedSelectedAnalysis,
        };
        use crate::analyses::prepared_machine_effect_catalog;
        use crate::rewrites::fold_selected_incoming_literal;
        use crate::{
            LiteralFoldPolicy, OptimizedLiteralFoldCustodyError, StagedOptimizedAllocationLegality,
            StagedOptimizedLiteralFoldAttempt, StagedOptimizedLiteralFoldStep,
            StagedSelectedLoweringOptimizationCustodyReceipt,
            StagedSelectedLoweringOptimizationRun, ValidatedAllocationLegality,
            ValidatedLiveRanges,
        };
        use crate::{
            analyze_allocation_legality, analyze_live_ranges, analyze_liveness,
            choose_spill_victims, classify_pressure_recovery, resolve_selected_lowering_rules,
        };

        use super::accounting::{
            add_usage, applied_action_count, attempt_usage, ensure_selected_lowering_budget,
            selected_lowering_custody_receipt, step_usage, validate_selected_lowering_policies,
        };
        pub(crate) fn execute_selected_lowering_optimizations(
            source: StagedOptimizedAllocationLegality,
            selections: OptimizationSelections,
            selected_lowering_selections: OptimizationSelections,
            fold_policy: LiteralFoldPolicy,
        ) -> Result<StagedSelectedLoweringOptimizationRun, OptimizedLiteralFoldCustodyError>
        {
            let upstream = source.custody();
            let budget = source.budget_per_pass();

            let choice_policy = SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1;
            let recovery_policy =
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1;
            let iteration_bound = source.legality().receipt().virtual_register_count();
            let selected = source.selected();
            let mut attempt = build_attempt(
                selected,
                source.ranges(),
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
                previous_measure = charge_step_measure(previous_measure, &step)?;
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
            if attempt.fold.receipt().source_selected()
                != attempt.fold.receipt().transformed_selected()
            {
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
            );
            Ok(StagedSelectedLoweringOptimizationRun {
                source,
                selections,
                selected_lowering_selections,
                steps,
                attempt,
                custody,
            })
        }

        pub(crate) fn validate_selected_lowering_optimization_custody(
            run: &StagedSelectedLoweringOptimizationRun,
        ) -> Result<
            StagedSelectedLoweringOptimizationCustodyReceipt,
            OptimizedLiteralFoldCustodyError,
        > {
            let upstream = run.source.custody();
            let expected_budget = run.source.budget_per_pass();
            let selected_lowering = run
                .selections
                .project_phase(optimization_core::OptimizationExecutionPhase::SelectedLowering);
            let (projected, fold_policy) = resolve_selected_lowering_rules(&selected_lowering)?;
            if run.selections != *run.source.selections()
                || run.custody.selections != run.source.selections().identity()
                || run.custody.budget != expected_budget
                || projected != run.selected_lowering_selections
            {
                return Err(OptimizedLiteralFoldCustodyError::SelectionProjectionMismatch);
            }
            for step in &run.steps {
                validate_selected_lowering_policies(
                    step.choices(),
                    step.recovery(),
                    step.fold(),
                    expected_budget,
                    fold_policy,
                )?;
            }
            validate_selected_lowering_policies(
                run.attempt.choices(),
                run.attempt.recovery(),
                run.attempt.fold(),
                expected_budget,
                fold_policy,
            )?;
            let selected = run.source.selected();
            let mut replayed = Vec::with_capacity(run.steps.len());
            if let Some(first) = run.steps.first() {
                replayed.push(replay_step(
                    0,
                    selected,
                    run.source.ranges(),
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
                    run.source.ranges(),
                    run.source.legality(),
                    &run.source,
                    SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                    RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                    fold_policy,
                    expected_budget,
                )?,
            };
            if terminal.fold.receipt().applied_count() != 0
                || terminal.fold.receipt().source_selected()
                    != terminal.fold.receipt().transformed_selected()
            {
                return Err(OptimizedLiteralFoldCustodyError::TerminalAttemptApplied);
            }
            let mut previous_measure = run.source.legality().receipt().virtual_register_count();
            for step in &replayed {
                previous_measure = charge_step_measure(previous_measure, step)?;
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
            );
            if replayed != run.steps || terminal != run.attempt || receipt != run.custody {
                return Err(OptimizedLiteralFoldCustodyError::StepMismatch { step: 0 });
            }
            Ok(receipt)
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
            let environment = source.register_environment();
            let choices = choose_spill_victims(
                legality,
                ranges,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
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
            let effect_catalog =
                prepared_machine_effect_catalog(environment.target(), environment.constraints())
                    .map_err(OptimizedLiteralFoldCustodyError::MachineEffects)?;
            let fold = fold_selected_incoming_literal(
                selected,
                ranges,
                legality,
                &choices,
                &recovery,
                source.allocator_availability(),
                environment.identity(),
                environment.constraints(),
                &environment.allocation_constraint_keys(),
                &effect_catalog,
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
            let environment = source.register_environment();
            let StagedOptimizedLiteralFoldAttempt {
                choices,
                recovery,
                fold,
            } = attempt;
            let liveness =
                analyze_liveness(&fold).map_err(OptimizedLiteralFoldCustodyError::Liveness)?;
            let ranges = analyze_live_ranges(&fold, &liveness)
                .map_err(OptimizedLiteralFoldCustodyError::LiveRanges)?;
            let legality = analyze_allocation_legality(
                &ranges,
                source.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
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

        fn charge_step_measure(
            previous_measure: usize,
            step: &StagedOptimizedLiteralFoldStep,
        ) -> Result<usize, OptimizedLiteralFoldCustodyError> {
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
            Ok(current_measure)
        }
    }

    use crate::{
        AllocationLegalityError, LiteralFoldError, LiteralFoldIdentity, LiteralFoldPolicy,
        LiveRangeError, LivenessError, MachineEffectStageError, RecoveryClassificationError,
        SelectedLoweringRuleCatalogError, SpillChoiceError,
        StagedOptimizedAllocationLegalityCustodyReceipt, ValidatedAllocationLegality,
        ValidatedLiteralFold, ValidatedLiveRanges, ValidatedLiveness,
        ValidatedRecoveryClassifications, ValidatedSpillChoices,
    };
    pub(crate) use execution::validate_selected_lowering_optimization_custody;

    impl From<SelectedLoweringRuleCatalogError> for OptimizedLiteralFoldCustodyError {
        fn from(error: SelectedLoweringRuleCatalogError) -> Self {
            match error {
                SelectedLoweringRuleCatalogError::WrongPhase(_) => {
                    Self::SelectionProjectionMismatch
                }
                SelectedLoweringRuleCatalogError::MissingSelection => {
                    Self::MissingSelectedLoweringOptimization
                }
                SelectedLoweringRuleCatalogError::UnsupportedSelection(optimization) => {
                    Self::UnsupportedSelectedLoweringOptimization(optimization)
                }
            }
        }
    }

    pub(crate) fn run_selected_lowering_optimizations(
        source: StagedOptimizedAllocationLegality,
    ) -> Result<StagedSelectedLoweringOptimizationRun, OptimizedLiteralFoldCustodyError> {
        let selections = source.selections().clone();
        let selected_lowering = selections
            .project_phase(optimization_core::OptimizationExecutionPhase::SelectedLowering);
        let (selected, fold_policy) = resolve_selected_lowering_rules(&selected_lowering)?;
        execution::execute_selected_lowering_optimizations(
            source,
            selections,
            selected,
            fold_policy,
        )
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StagedOptimizedLiteralFoldStep {
        choices: ValidatedSpillChoices,
        recovery: ValidatedRecoveryClassifications,
        fold: ValidatedLiteralFold,
        liveness: ValidatedLiveness,
        ranges: ValidatedLiveRanges,
        legality: ValidatedAllocationLegality,
    }

    impl StagedOptimizedLiteralFoldStep {
        pub const fn choices(&self) -> &ValidatedSpillChoices {
            &self.choices
        }
        pub const fn recovery(&self) -> &ValidatedRecoveryClassifications {
            &self.recovery
        }
        pub const fn fold(&self) -> &ValidatedLiteralFold {
            &self.fold
        }
        validated_analysis_accessors!();
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StagedOptimizedLiteralFoldAttempt {
        choices: ValidatedSpillChoices,
        recovery: ValidatedRecoveryClassifications,
        fold: ValidatedLiteralFold,
    }

    impl StagedOptimizedLiteralFoldAttempt {
        pub const fn choices(&self) -> &ValidatedSpillChoices {
            &self.choices
        }
        pub const fn recovery(&self) -> &ValidatedRecoveryClassifications {
            &self.recovery
        }
        pub const fn fold(&self) -> &ValidatedLiteralFold {
            &self.fold
        }
    }

    #[derive(Debug)]
    pub struct StagedSelectedLoweringOptimizationRun {
        source: StagedOptimizedAllocationLegality,
        selections: OptimizationSelections,
        selected_lowering_selections: OptimizationSelections,
        steps: Vec<StagedOptimizedLiteralFoldStep>,
        attempt: StagedOptimizedLiteralFoldAttempt,
        custody: StagedSelectedLoweringOptimizationCustodyReceipt,
    }

    impl StagedSelectedLoweringOptimizationRun {
        pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
            &self.source
        }
        pub const fn selections(&self) -> &OptimizationSelections {
            &self.selections
        }
        pub const fn selected_lowering_selections(&self) -> &OptimizationSelections {
            &self.selected_lowering_selections
        }
        pub fn steps(&self) -> &[StagedOptimizedLiteralFoldStep] {
            &self.steps
        }
        pub const fn attempt(&self) -> &StagedOptimizedLiteralFoldAttempt {
            &self.attempt
        }
        pub const fn custody(&self) -> &StagedSelectedLoweringOptimizationCustodyReceipt {
            &self.custody
        }
    }

    impl StagedSelectedLoweringOptimizationRun {
        pub(crate) fn into_completed(
            self,
        ) -> selected_instruction_analyses::CompletedSelectedOptimization<
            SelectedLoweringOptimizationCompletionIdentity,
        > {
            let ledger = selected_instruction_analyses::SelectedOptimizationLedger {
                identity: self.custody.identity,
                phase_selections: self.custody.selected_lowering_selections,
                source: self.custody.source,
                transformations: self
                    .custody
                    .iterations
                    .iter()
                    .map(|iteration| {
                        register_homes::PostAllocationSelectedTransformation::LiteralFold(
                            iteration.fold(),
                        )
                    })
                    .collect(),
            };
            let source = self.source;
            let (current, liveness, ranges, legality) = match self.steps.into_iter().last() {
                Some(step) => (
                    crate::OwnedSelectedProgram::retain(&step.fold),
                    step.liveness,
                    step.ranges,
                    step.legality,
                ),
                None => (
                    crate::OwnedSelectedProgram::retain(source.selected()),
                    source.liveness().clone(),
                    source.ranges().clone(),
                    source.legality().clone(),
                ),
            };
            selected_instruction_analyses::CompletedSelectedOptimization {
                source,
                current,
                liveness,
                ranges,
                legality,
                ledger,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StagedSelectedLoweringOptimizationCustodyReceipt {
        identity: SelectedLoweringOptimizationCompletionIdentity,
        source: StagedOptimizedAllocationLegalityCustodyReceipt,
        selections: OptimizationSelectionIdentity,
        selected_lowering_selections: OptimizationSelectionIdentity,
        budget: OptimizationWorkBudget,
        usage: OptimizationWorkUsage,
        iteration_bound: usize,
        action_count: usize,
        initial_virtual_register_count: usize,
        iterations: Vec<StagedOptimizedLiteralFoldIterationReceipt>,
        attempt: StagedOptimizedLiteralFoldAttemptReceipt,
        final_selected: SelectedInstructionPlanIdentity,
        final_liveness: selected_instructions::LivenessIdentity,
        final_ranges: selected_instructions::LiveRangeIdentity,
        final_legality: register_homes::AllocationLegalityIdentity,
        final_virtual_register_count: usize,
    }

    impl StagedSelectedLoweringOptimizationCustodyReceipt {
        receipt_accessor_rows!(ref StagedSelectedLoweringOptimizationCustodyReceipt {
            identity: SelectedLoweringOptimizationCompletionIdentity,
            source: StagedOptimizedAllocationLegalityCustodyReceipt,
            selections: OptimizationSelectionIdentity,
            selected_lowering_selections: OptimizationSelectionIdentity,
            budget: OptimizationWorkBudget,
            usage: OptimizationWorkUsage,
            iteration_bound: usize,
            action_count: usize,
            initial_virtual_register_count: usize,
            attempt: StagedOptimizedLiteralFoldAttemptReceipt,
            final_selected: SelectedInstructionPlanIdentity,
            final_liveness: selected_instructions::LivenessIdentity,
            final_ranges: selected_instructions::LiveRangeIdentity,
            final_legality: register_homes::AllocationLegalityIdentity,
            final_virtual_register_count: usize,
        });
        pub fn iterations(&self) -> &[StagedOptimizedLiteralFoldIterationReceipt] {
            &self.iterations
        }
    }

    copy_receipt! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct StagedOptimizedLiteralFoldAttemptReceipt {
            source_selected: SelectedInstructionPlanIdentity,
            source_ranges: selected_instructions::LiveRangeIdentity,
            source_legality: register_homes::AllocationLegalityIdentity,
            choices: register_homes::SpillChoiceIdentity,
            choice_policy: SpillChoicePolicy,
            choice_usage: OptimizationWorkUsage,
            recovery: register_homes::RecoveryClassificationIdentity,
            recovery_policy: RecoveryClassificationPolicy,
            recovery_usage: OptimizationWorkUsage,
            fold: LiteralFoldIdentity,
            fold_policy: LiteralFoldPolicy,
            fold_usage: OptimizationWorkUsage,
            applied_count: usize,
            transformed_selected: SelectedInstructionPlanIdentity,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StagedOptimizedLiteralFoldIterationReceipt {
        evidence: StagedOptimizedLiteralFoldAttemptReceipt,
        fresh_liveness: selected_instructions::LivenessIdentity,
        fresh_ranges: selected_instructions::LiveRangeIdentity,
        fresh_legality: register_homes::AllocationLegalityIdentity,
    }

    impl StagedOptimizedLiteralFoldIterationReceipt {
        receipt_accessor_rows!(nested StagedOptimizedLiteralFoldIterationReceipt, evidence {
            source_selected: SelectedInstructionPlanIdentity,
            source_ranges: selected_instructions::LiveRangeIdentity,
            source_legality: register_homes::AllocationLegalityIdentity,
            transformed_selected: SelectedInstructionPlanIdentity,
            choices: register_homes::SpillChoiceIdentity,
            choice_policy: SpillChoicePolicy,
            choice_usage: OptimizationWorkUsage,
            recovery: register_homes::RecoveryClassificationIdentity,
            recovery_policy: RecoveryClassificationPolicy,
            recovery_usage: OptimizationWorkUsage,
            fold: LiteralFoldIdentity,
            fold_policy: LiteralFoldPolicy,
            fold_usage: OptimizationWorkUsage,
            applied_count: usize,
        });
        receipt_accessor_rows!(StagedOptimizedLiteralFoldIterationReceipt {
            fresh_liveness: selected_instructions::LivenessIdentity,
            fresh_ranges: selected_instructions::LiveRangeIdentity,
            fresh_legality: register_homes::AllocationLegalityIdentity,
        });
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedLiteralFoldCustodyError {
        SpillChoice(SpillChoiceError),
        RecoveryClassification(RecoveryClassificationError),
        MachineEffects(MachineEffectStageError),
        Fold(LiteralFoldError),
        NoAppliedFold,
        Liveness(LivenessError),
        LiveRanges(LiveRangeError),
        AllocationLegality(AllocationLegalityError),
        RemainingTransitions {
            count: usize,
        },
        EmptySequence,
        StepMismatch {
            step: usize,
        },
        MissingSelectedLoweringOptimization,
        UnsupportedSelectedLoweringOptimization(Optimization),
        SelectedLoweringMeasureMismatch {
            previous: usize,
            applied: usize,
            current: usize,
        },
        SelectedLoweringIterationBoundExceeded {
            bound: usize,
        },
        SelectionProjectionMismatch,
        TerminalAttemptApplied,
        WorkOverflow,
        SelectedLoweringBudgetExceeded {
            required: OptimizationWorkUsage,
            budget: OptimizationWorkBudget,
        },
    }

    failed_display!(
        OptimizedLiteralFoldCustodyError,
        "optimized literal-fold staging failed"
    );
}

pub(crate) mod selected_lowering {
    pub(crate) mod catalog {
        use optimization_core::{
            Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch,
        };

        use super::super::RegisterAllocationRuleTargetApplicability;
        use super::{LiteralFoldPolicy, SelectedInstructionPairRule};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability,
            policy: LiteralFoldPolicy,
            pairs: &'static [SelectedInstructionPairRule],
        }

        impl SelectedLoweringRuleCatalogPayload {
            receipt_accessor_rows!(SelectedLoweringRuleCatalogPayload {
                policy: LiteralFoldPolicy,
                pairs: &'static [SelectedInstructionPairRule],
            });
        }

        pub(crate) type SelectedLoweringRuleCatalogEntry =
            OptimizationCatalogDescriptor<SelectedLoweringRuleCatalogPayload>;

        const fn row(
            optimization: Optimization,
            policy: LiteralFoldPolicy,
            pairs: &'static [SelectedInstructionPairRule],
        ) -> SelectedLoweringRuleCatalogEntry {
            SelectedLoweringRuleCatalogEntry::new(
                optimization,
                SelectedLoweringRuleCatalogPayload {
                    target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
                    policy,
                    pairs,
                },
            )
        }

        pub(crate) const SELECTED_LOWERING_RULE_CATALOG: [SelectedLoweringRuleCatalogEntry; 23] = [
            row(
                Optimization::SelectedIncomingU12ExactAddImmediate,
                LiteralFoldPolicy::EXACT_ADD_V1,
                &[
                    SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
                    SelectedInstructionPairRule::EXACT_ADD_LEFT_IMMEDIATE_U12,
                ],
            ),
            row(
                Optimization::SelectedIncomingU12ExactSubtractImmediate,
                LiteralFoldPolicy::EXACT_SUBTRACT_V1,
                &[SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12],
            ),
            row(
                Optimization::SelectedIncomingU12CompareImmediate,
                LiteralFoldPolicy::COMPARE_V1,
                &[
                    SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12,
                    SelectedInstructionPairRule::COMPARE_LEFT_IMMEDIATE_U12,
                    SelectedInstructionPairRule::COMPARE_ZERO,
                    SelectedInstructionPairRule::COMPARE_LEFT_ZERO,
                ],
            ),
            row(
                Optimization::SelectedIncomingLiteralExtensionElimination,
                LiteralFoldPolicy::EXTENSION_V1,
                &SelectedInstructionPairRule::EXTENSION_LITERAL_FOLDS,
            ),
            row(
                Optimization::SelectedIncomingU12Load8IndexedOffset,
                LiteralFoldPolicy::LOAD8_INDEXED_V1,
                &[SelectedInstructionPairRule::LOAD8_INDEXED_U12],
            ),
            row(
                Optimization::SelectedIncomingLiteralCopyMaterialization,
                LiteralFoldPolicy::COPY_V1,
                &[SelectedInstructionPairRule::COPY_LITERAL_FOLD],
            ),
            row(
                Optimization::SelectedIncomingU12ByteViewAddressOffset,
                LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
                &[
                    SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_OFFSET_U12,
                    SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_BACKING_U12,
                ],
            ),
            row(
                Optimization::SelectedIncomingExactDivideIdentityCopy,
                LiteralFoldPolicy::EXACT_DIVIDE_V1,
                &[SelectedInstructionPairRule::EXACT_DIVIDE_ONE_COPY],
            ),
            row(
                Optimization::SelectedIncomingWrappingRemainderOneZeroMaterialization,
                LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
                &[SelectedInstructionPairRule::WRAPPING_REMAINDER_ONE_MATERIALIZE],
            ),
            row(
                Optimization::SelectedIncomingBitwiseAndZeroMaterialization,
                LiteralFoldPolicy::BITWISE_AND_ZERO_V1,
                &SelectedInstructionPairRule::BITWISE_AND_ZERO_FOLDS,
            ),
            row(
                Optimization::SelectedIncomingBitwiseXorZeroIdentityCopy,
                LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
                &SelectedInstructionPairRule::BITWISE_XOR_ZERO_COPIES,
            ),
            row(
                Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
                LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
                &SelectedInstructionPairRule::WRAPPING_ADD_ZERO_COPIES,
            ),
            row(
                Optimization::SelectedIncomingBitwiseAndOnesIdentityCopy,
                LiteralFoldPolicy::BITWISE_AND_ONES_V1,
                &SelectedInstructionPairRule::BITWISE_AND_ONES_COPIES,
            ),
            row(
                Optimization::SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization,
                LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
                &[SelectedInstructionPairRule::WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE],
            ),
            row(
                Optimization::SelectedIncomingExactDivideZeroDividendZeroMaterialization,
                LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
                &[SelectedInstructionPairRule::EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE],
            ),
            row(
                Optimization::SelectedIncomingSaturatingAddZeroIdentityCopy,
                LiteralFoldPolicy::SATURATING_ADD_ZERO_V1,
                &SelectedInstructionPairRule::SATURATING_ADD_ZERO_COPIES,
            ),
            row(
                Optimization::SelectedIncomingSaturatingSubtractZeroIdentityCopy,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
                &SelectedInstructionPairRule::SATURATING_SUBTRACT_ZERO_COPIES,
            ),
            row(
                Optimization::SelectedIncomingSaturatingDivideOneIdentityCopy,
                LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
                &SelectedInstructionPairRule::SATURATING_DIVIDE_ONE_COPIES,
            ),
            row(
                Optimization::SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization,
                LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1,
                &SelectedInstructionPairRule::SATURATING_DIVIDE_ZERO_DIVIDEND_MATERIALIZATIONS,
            ),
            row(
                Optimization::SelectedIncomingSaturatingSubtractZeroMinuendZeroMaterialization,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
                &SelectedInstructionPairRule::SATURATING_SUBTRACT_ZERO_MINUEND_MATERIALIZATIONS,
            ),
            row(
                Optimization::SelectedIncomingSaturatingAddUpperBoundMaterialization,
                LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1,
                &SelectedInstructionPairRule::SATURATING_ADD_UPPER_BOUND_MATERIALIZATIONS,
            ),
            row(
                Optimization::SelectedIncomingWrappingRemainderMinusOneZeroMaterialization,
                LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
                &[SelectedInstructionPairRule::WRAPPING_REMAINDER_MINUS_ONE_MATERIALIZE],
            ),
            row(
                Optimization::SelectedIncomingSaturatingSubtractUpperBoundSubtrahendZeroMaterialization,
                LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1,
                &SelectedInstructionPairRule::SATURATING_SUBTRACT_UPPER_BOUND_MATERIALIZATIONS,
            ),
        ];

        pub(crate) fn enabled_pair_rules(
            policy: LiteralFoldPolicy,
        ) -> impl Iterator<Item = SelectedInstructionPairRule> {
            SELECTED_LOWERING_RULE_CATALOG
                .into_iter()
                .filter(move |entry| policy.contains(entry.payload().policy()))
                .flat_map(|entry| entry.payload().pairs().iter().copied())
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum SelectedLoweringRuleCatalogError {
            WrongPhase(OptimizationPhaseMismatch),
            MissingSelection,
            UnsupportedSelection(Optimization),
        }
    }
    pub(crate) mod literal_fold {
        use crate::cursor::{Cursor, TakeError, Truncated};
        use crate::{
            ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiveRanges,
            ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
        };
        use register_model::{
            TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
            ValidatedRegisterConstraintCatalog,
        };

        pub(crate) mod compute {
            pub(crate) mod actions {
                use register_model::RegisterOperandAccess;
                use selected_instructions::{
                    SelectedBlockId, SelectedFunction, SelectedInstruction,
                    SelectedInstructionKind, VirtualRegisterId, VirtualRegisterOrigin,
                    terminator_successors,
                };
                use semantic_vocabulary::IntegerValue;

                use crate::{LiteralFoldAction, PairUnitDefRelation};

                use super::constraints::AdmittedPairs;
                use super::function_rewrite::rewritten_consumer;
                use crate::rewrites::operand_probes::effect_declaration;

                struct SoleUse {
                    consumer_index: usize,
                    operand: u16,
                }

                pub(crate) fn derive_actions(
                    function: &SelectedFunction,
                    rows: &AdmittedPairs<'_>,
                ) -> Vec<LiteralFoldAction> {
                    let mut actions = Vec::new();
                    for (block_index, block) in function.blocks.iter().enumerate() {
                        for (literal_index, literal) in block.instructions.iter().enumerate() {
                            let SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(value),
                            } = literal.kind
                            else {
                                continue;
                            };
                            let Some(victim) = producer_victim(literal) else {
                                continue;
                            };
                            let Some(literal_u64) = u64::try_from(value).ok() else {
                                continue;
                            };
                            if !producer_record_admits(literal)
                                || !register_admits(function, literal, victim, value)
                            {
                                continue;
                            }
                            let Some(site) =
                                sole_use(function, block_index, literal_index, literal, victim)
                            else {
                                continue;
                            };
                            let consumer = &block.instructions[site.consumer_index];
                            if actions.iter().any(|action: &LiteralFoldAction| {
                                action.consumer_instruction == consumer.id
                            }) {
                                continue;
                            }
                            if let Some(action) = admit_candidate(
                                function,
                                block_index,
                                block.id,
                                literal,
                                site.consumer_index,
                                consumer,
                                site.operand,
                                victim,
                                literal_u64,
                                rows,
                            )
                            .filter(|action| rewritten_consumer(0, function, action, rows).is_ok())
                            {
                                actions.push(action);
                            }
                        }
                    }
                    actions
                }

                fn producer_victim(literal: &SelectedInstruction) -> Option<VirtualRegisterId> {
                    let [operand] = literal.operands.as_slice() else {
                        return None;
                    };
                    (operand.access == RegisterOperandAccess::Def
                        && operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                        && !operand.early_clobber)
                        .then_some(operand.virtual_register)
                }

                fn producer_record_admits(literal: &SelectedInstruction) -> bool {
                    literal.implicit_uses.is_empty()
                        && literal.implicit_defs.is_empty()
                        && literal.clobbers.is_empty()
                        && literal.provenance.operations.len() == 1
                        && literal.provenance.values.len() == 1
                        && literal.provenance.edges.is_empty()
                        && literal.provenance.obligations.is_empty()
                        && !literal.provenance.work.is_empty()
                        && literal.provenance.work.iter().all(|work| {
                            work.site
                                == optimization_unit::PsiProvenance::Operation(
                                    literal.provenance.operations[0],
                                )
                        })
                }

                fn register_admits(
                    function: &SelectedFunction,
                    literal: &SelectedInstruction,
                    victim: VirtualRegisterId,
                    value: u128,
                ) -> bool {
                    let Some(register) = function.virtual_register(victim) else {
                        return false;
                    };
                    let Some(integer) = register.scalar_type.as_integer() else {
                        return false;
                    };
                    !integer.is_address()
                        && integer.admits(IntegerValue::Unsigned(value))
                        && register.entry_fixed_view.is_none()
                        && matches!(
                            register.origin,
                            VirtualRegisterOrigin::InstructionResult {
                                instruction,
                                source_value,
                            } if instruction == literal.id
                                && literal.provenance.values.as_slice() == [source_value]
                        )
                }

                fn sole_use(
                    function: &SelectedFunction,
                    block_index: usize,
                    literal_index: usize,
                    literal: &SelectedInstruction,
                    victim: VirtualRegisterId,
                ) -> Option<SoleUse> {
                    let mut site = None;
                    for (candidate_block_index, block) in function.blocks.iter().enumerate() {
                        let terminator =
                            selected_instructions::terminator_instruction(&block.terminator);
                        for (instruction_index, (instruction, in_block_body)) in block
                            .instructions
                            .iter()
                            .map(|instruction| (instruction, true))
                            .chain(std::iter::once((terminator, false)))
                            .enumerate()
                        {
                            for operand in &instruction.operands {
                                if operand.virtual_register != victim {
                                    continue;
                                }
                                match operand.access {
                                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef => {
                                        if candidate_block_index != block_index
                                            || instruction.id != literal.id
                                            || operand.access != RegisterOperandAccess::Def
                                        {
                                            return None;
                                        }
                                    }
                                    RegisterOperandAccess::Use => {
                                        if site.is_some() {
                                            return None;
                                        }
                                        site = Some((
                                            candidate_block_index,
                                            in_block_body,
                                            instruction_index,
                                            operand.operand,
                                        ));
                                    }
                                }
                            }
                        }
                        let successors = terminator_successors(&block.terminator);
                        for successor in successors {
                            let carried = successor.structural_bindings.iter().any(|binding| {
                                matches!(
                                    binding.transport,
                                    selected_instructions::SelectedStructuralTransport::WholeValue {
                                        argument,
                                        ..
                                    }
                                    | selected_instructions::SelectedStructuralTransport::ScalarReference {
                                        base: selected_instructions::SelectedAddressBase::Register(argument),
                                        ..
                                    }
                                    | selected_instructions::SelectedStructuralTransport::Address {
                                        base: selected_instructions::SelectedAddressBase::Register(argument),
                                        ..
                                    } if argument == victim
                                )
                            }) || successor.structural_case.iter().any(|case| {
                                case.payloads
                                    .iter()
                                    .any(|payload| match &payload.transport {
                                        selected_instructions::SelectedCasePayloadTransport::Unused => false,
                                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                                            parameter,
                                        } => *parameter == victim,
                                        selected_instructions::SelectedCasePayloadTransport::Registers {
                                            argument,
                                            parameter,
                                        } => *argument == victim || *parameter == victim,
                                    })
                            }) || successor
                                .bindings
                                .iter()
                                .any(|binding| match &binding.transport {
                                    selected_instructions::SelectedValueTransport::Unused => false,
                                    selected_instructions::SelectedValueTransport::Registers {
                                        argument,
                                        parameter,
                                    } => *argument == victim || *parameter == victim,
                                });
                            if carried {
                                return None;
                            }
                        }
                    }
                    let (site_block, in_block_body, instruction_index, operand) = site?;
                    (site_block == block_index
                        && in_block_body
                        && instruction_index > literal_index)
                        .then_some(SoleUse {
                            consumer_index: instruction_index,
                            operand,
                        })
                }

                #[allow(clippy::too_many_arguments)]
                fn admit_candidate(
                    function: &SelectedFunction,
                    block_index: usize,
                    block: SelectedBlockId,
                    literal: &SelectedInstruction,
                    consumer_index: usize,
                    consumer: &SelectedInstruction,
                    use_operand: u16,
                    victim_register: VirtualRegisterId,
                    literal_u64: u64,
                    rows: &AdmittedPairs<'_>,
                ) -> Option<LiteralFoldAction> {
                    let pair = rows.for_consumer(consumer.kind, use_operand, literal_u64)?;
                    let immediate = pair.rule.fold_immediate(literal_u64)?;
                    if !pair.rule.matches_producer(literal.kind)
                        || literal.operands.len() != 1
                        || literal.operands[0].virtual_register != victim_register
                        || literal.operands[0].access != RegisterOperandAccess::Def
                        || !pair.rule.unit_effects().admits_producer(literal)
                    {
                        return None;
                    }

                    let row = pair.row;
                    let shape = pair.rule.operand_shape();
                    let victim_operand = usize::from(shape.victim_operand());
                    let survivor_operand = shape.survivor_operand().map(usize::from);
                    let result_operand = shape.result_operand(pair.rule.result()).map(usize::from);
                    let mut operand_roles = victim_operand + 1;
                    for index in survivor_operand.into_iter().chain(result_operand) {
                        operand_roles = operand_roles.max(index + 1);
                    }
                    let (grammar, tail) = consumer.operands.split_at_checked(operand_roles)?;
                    if !shape.tail.admits(function, tail) {
                        return None;
                    }
                    if grammar[victim_operand].access != RegisterOperandAccess::Use
                        || grammar[victim_operand].virtual_register != victim_register
                    {
                        return None;
                    }
                    if survivor_operand
                        .is_some_and(|index| grammar[index].access != RegisterOperandAccess::Use)
                    {
                        return None;
                    }
                    let scalar_result = result_operand.map(|index| &grammar[index]);
                    if scalar_result
                        .is_some_and(|result| result.access != RegisterOperandAccess::Def)
                    {
                        return None;
                    }
                    let (binds_surviving, binds_result) = pair.rule.rewritten_row_roles()?;
                    let row_operands = pair.row.operands.as_slice();
                    let expected_row_operands =
                        usize::from(binds_surviving) + usize::from(binds_result);
                    if row_operands.len() != expected_row_operands
                        || row_operands
                            .iter()
                            .enumerate()
                            .any(|(index, operand)| usize::from(operand.operand) != index)
                    {
                        return None;
                    }
                    if binds_surviving
                        && row_operands[0].class
                            != survivor_operand.map(|index| &grammar[index])?.class
                    {
                        return None;
                    }
                    if pair.rule.machine_effects().unit_defs == PairUnitDefRelation::OperandSwapped
                        && (consumer.implicit_uses != pair.row.implicit_uses
                            || consumer.implicit_defs != pair.row.implicit_defs
                            || consumer.clobbers != pair.row.clobbers)
                    {
                        return None;
                    }
                    let result = if !binds_result {
                        None
                    } else {
                        let row_def = &row_operands[expected_row_operands - 1];
                        let result = scalar_result?;
                        if row_def.access != RegisterOperandAccess::Def
                            || row_def.class != result.class
                        {
                            return None;
                        }
                        Some(result.virtual_register)
                    };
                    if !pair.rule.unit_effects().admits_consumer(consumer) {
                        return None;
                    }
                    if !pair
                        .rule
                        .machine_effects()
                        .admits_consumer_obligation(consumer)
                    {
                        return None;
                    }
                    if !pair
                        .rule
                        .machine_effects()
                        .admits_dead_consumer_defs(consumer, function)
                    {
                        return None;
                    }
                    if !pair.rule.machine_effects().admits_swapped_condition_defs(
                        consumer,
                        function,
                        block_index,
                        consumer_index,
                    ) {
                        return None;
                    }
                    let producer_declaration =
                        effect_declaration(rows.catalog, pair.rule.producer(), literal.constraint)?;
                    let consumer_declaration = effect_declaration(
                        rows.catalog,
                        pair.rule.consumer(),
                        consumer.constraint,
                    )?;
                    let effects = pair.rule.machine_effects();
                    if !effects.admits_producer(producer_declaration)
                        || !effects.admits_consumer(consumer_declaration, pair.declaration)
                    {
                        return None;
                    }
                    let surviving = grammar
                        [usize::from(shape.survivor_operand().unwrap_or(shape.victim_operand()))]
                    .virtual_register;

                    Some(LiteralFoldAction {
                        block,
                        literal_instruction: literal.id,
                        victim: victim_register,
                        consumer_instruction: consumer.id,
                        surviving,
                        result,
                        immediate,
                        immediate_constraint: row.key,
                    })
                }
            }
            pub(crate) mod constraints {
                use register_model::{
                    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
                    TargetRegisterEnvironmentConstraintKeys, ValidatedRegisterConstraintCatalog,
                };
                use selected_instructions::{
                    MachineEffectDeclaration, PreparedMachineEffectCatalog, SelectedInstructionKind,
                };

                use crate::rewrites::enabled_pair_rules;
                use crate::rewrites::operand_probes::effect_declaration;
                use crate::{
                    LiteralFoldError, LiteralFoldPolicy, PairResultDisposition,
                    SelectedInstructionPairRule,
                };

                pub(crate) struct AdmittedPair<'a> {
                    pub(crate) rule: SelectedInstructionPairRule,
                    pub(crate) row: &'a RegisterInstructionConstraint,
                    pub(crate) declaration: &'a MachineEffectDeclaration,
                }

                pub(crate) struct AdmittedPairs<'a> {
                    pairs: Vec<AdmittedPair<'a>>,
                    pub(crate) catalog: &'a PreparedMachineEffectCatalog,
                }

                impl<'a> AdmittedPairs<'a> {
                    pub(crate) fn for_consumer(
                        &self,
                        kind: SelectedInstructionKind,
                        victim_operand: u16,
                        literal: u64,
                    ) -> Option<&AdmittedPair<'a>> {
                        let mut matches = self.pairs.iter().filter(|pair| {
                            pair.rule.matches_consumer(kind)
                                && pair.rule.victim_operand() == victim_operand
                                && pair.rule.admits_immediate(literal)
                        });
                        let pair = matches.next()?;
                        if matches.next().is_some() {
                            return None;
                        }
                        Some(pair)
                    }

                    pub(crate) fn for_position(
                        &self,
                        kind: SelectedInstructionKind,
                        victim_operand: u16,
                    ) -> Option<&AdmittedPair<'a>> {
                        self.pairs.iter().find(|pair| {
                            pair.rule.matches_consumer(kind)
                                && pair.rule.victim_operand() == victim_operand
                        })
                    }

                    pub(crate) fn for_consumer_row(
                        &self,
                        kind: SelectedInstructionKind,
                        key: RegisterConstraintKey,
                    ) -> Option<&AdmittedPair<'a>> {
                        self.pairs
                            .iter()
                            .find(|pair| pair.rule.matches_consumer(kind) && pair.row.key == key)
                    }
                }

                pub(crate) fn select_admitted_pairs<'a>(
                    constraints: &'a ValidatedRegisterConstraintCatalog,
                    keys: &TargetRegisterEnvironmentConstraintKeys,
                    policy: LiteralFoldPolicy,
                    catalog: &'a PreparedMachineEffectCatalog,
                ) -> Result<AdmittedPairs<'a>, LiteralFoldError> {
                    let find = |key| {
                        register_model::find_register_constraint(
                            &constraints.catalog().constraints,
                            key,
                        )
                        .ok_or(LiteralFoldError::ImmediateConstraintMismatch)
                    };
                    let mut pairs = Vec::new();
                    for rule in enabled_pair_rules(policy) {
                        let key = rule
                            .immediate_constraint_key(keys)
                            .ok_or(LiteralFoldError::ImmediateConstraintMismatch)?;
                        let row = find(key)?;
                        validate_immediate_row(rule, row)?;
                        let declaration = effect_declaration(catalog, rule.rewritten(), row.key)
                            .ok_or(LiteralFoldError::EffectCatalogMismatch)?;
                        if !rule.machine_effects().admits_rewritten(declaration) {
                            return Err(LiteralFoldError::EffectCatalogMismatch);
                        }
                        pairs.push(AdmittedPair {
                            rule,
                            row,
                            declaration,
                        });
                    }
                    Ok(AdmittedPairs { pairs, catalog })
                }

                fn validate_immediate_row(
                    rule: SelectedInstructionPairRule,
                    row: &RegisterInstructionConstraint,
                ) -> Result<(), LiteralFoldError> {
                    let unit_effects = rule.unit_effects();
                    let scalar_result = rule.result() == PairResultDisposition::ScalarRegister;
                    let (binds_surviving, binds_result) = rule
                        .rewritten_row_roles()
                        .ok_or(LiteralFoldError::ImmediateConstraintMismatch)?;
                    let survives = usize::from(binds_surviving);
                    let operands = row.operands.as_slice();
                    let well_formed = operands.len() == survives + usize::from(binds_result)
                        && operands.iter().enumerate().all(|(index, operand)| {
                            usize::from(operand.operand) == index
                                && operand.access
                                    == if index < survives {
                                        RegisterOperandAccess::Use
                                    } else {
                                        RegisterOperandAccess::Def
                                    }
                                && unit_effects.admits_operand(operand)
                        })
                        && row.implicit_defs.is_empty() == scalar_result
                        && operands
                            .windows(2)
                            .all(|pair| pair[0].class == pair[1].class)
                        && unit_effects.admits_row_units(row);
                    if !well_formed {
                        return Err(LiteralFoldError::ImmediateConstraintMismatch);
                    }
                    Ok(())
                }
            }
            pub(crate) mod function_rewrite {
                use register_model::RegisterOperandAccess;
                use selected_instructions::{
                    SelectedFunction, SelectedInstruction, SelectedInstructionId,
                    SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
                    terminator_successors_mut,
                };

                use crate::rewrites::selected_operands::selected_operand;
                use crate::{LiteralFoldAction, LiteralFoldError};

                use super::constraints::AdmittedPairs;

                pub(crate) fn apply_actions(
                    function_index: usize,
                    function: &mut SelectedFunction,
                    actions: &[LiteralFoldAction],
                    rows: &AdmittedPairs<'_>,
                ) -> Result<(), LiteralFoldError> {
                    let rewritten = actions
                        .iter()
                        .map(|action| rewritten_consumer(function_index, function, action, rows))
                        .collect::<Result<Vec<_>, _>>()?;
                    for (action, record) in actions.iter().zip(rewritten) {
                        let consumer = function
                            .blocks
                            .iter_mut()
                            .filter(|block| block.id == action.block)
                            .flat_map(|block| block.instructions.iter_mut())
                            .find(|instruction| instruction.id == action.consumer_instruction)
                            .ok_or(LiteralFoldError::DecisionMismatch {
                                function: function_index,
                            })?;
                        *consumer = record;
                    }
                    let mut removed_instructions = Vec::with_capacity(actions.len());
                    let mut settlement_shifts = Vec::new();
                    for block in &mut function.blocks {
                        let mut positions = Vec::new();
                        for action in actions.iter().filter(|action| action.block == block.id) {
                            let position = block
                                .instruction_index(action.literal_instruction)
                                .ok_or(LiteralFoldError::DecisionMismatch {
                                    function: function_index,
                                })?;
                            removed_instructions.push(action.literal_instruction);
                            positions.push(
                                u32::try_from(position)
                                    .map_err(|_| LiteralFoldError::WorkOverflow)?,
                            );
                        }
                        if positions.is_empty() {
                            continue;
                        }
                        positions.sort_unstable();
                        let retained: Vec<SelectedInstructionId> = actions
                            .iter()
                            .filter(|action| action.block == block.id)
                            .map(|action| action.literal_instruction)
                            .collect();
                        block
                            .instructions
                            .retain(|instruction| !retained.contains(&instruction.id));
                        settlement_shifts.push((block.id, positions));
                    }
                    for settlement in &mut function.boundary_settlements {
                        let Some((_, positions)) = settlement_shifts
                            .iter()
                            .find(|(block, _)| *block == settlement.block)
                        else {
                            continue;
                        };
                        let shift = u32::try_from(
                            positions
                                .iter()
                                .filter(|position| **position < settlement.instruction_index)
                                .count(),
                        )
                        .map_err(|_| LiteralFoldError::WorkOverflow)?;
                        settlement.instruction_index -= shift;
                    }
                    let mut removed_registers = Vec::with_capacity(actions.len());
                    for action in actions {
                        let victim_index = usize::try_from(action.victim.0).map_err(|_| {
                            LiteralFoldError::IdentifierUnderflow {
                                function: function_index,
                            }
                        })?;
                        if function
                            .virtual_registers
                            .get(victim_index)
                            .map(|register| register.id)
                            != Some(action.victim)
                        {
                            return Err(LiteralFoldError::DecisionMismatch {
                                function: function_index,
                            });
                        }
                        removed_registers.push(action.victim);
                    }
                    removed_registers.sort_unstable();
                    removed_instructions.sort_unstable();
                    function
                        .virtual_registers
                        .retain(|register| !removed_registers.contains(&register.id));
                    redensify(
                        function_index,
                        function,
                        &removed_instructions,
                        &removed_registers,
                    )
                }

                pub(crate) fn rewritten_consumer(
                    function_index: usize,
                    function: &SelectedFunction,
                    action: &LiteralFoldAction,
                    rows: &AdmittedPairs<'_>,
                ) -> Result<SelectedInstruction, LiteralFoldError> {
                    let block =
                        function
                            .block(action.block)
                            .ok_or(LiteralFoldError::DecisionMismatch {
                                function: function_index,
                            })?;
                    let literal = block.instruction(action.literal_instruction).ok_or(
                        LiteralFoldError::DecisionMismatch {
                            function: function_index,
                        },
                    )?;
                    let literal_provenance = literal.provenance.clone();
                    let mut consumer = block
                        .instruction(action.consumer_instruction)
                        .ok_or(LiteralFoldError::DecisionMismatch {
                            function: function_index,
                        })?
                        .clone();

                    let pair = rows
                        .for_consumer_row(consumer.kind, action.immediate_constraint)
                        .ok_or(LiteralFoldError::ConsumerMismatch {
                            function: function_index,
                        })?;
                    let result_scalar = action
                        .result
                        .and_then(|result| function.virtual_register(result))
                        .map(|register| register.scalar_type);
                    let rewritten_kind = pair
                        .rule
                        .rewrite_consumer(consumer.kind, action.immediate, result_scalar)
                        .ok_or(LiteralFoldError::ConsumerMismatch {
                            function: function_index,
                        })?;
                    let row = pair.row;

                    let consumer_provenance = consumer.provenance.clone();
                    let mut operations = literal_provenance.operations;
                    operations.extend(consumer_provenance.operations);
                    let mut work = literal_provenance.work;
                    work.extend(consumer_provenance.work);
                    let mut registers = Vec::with_capacity(row.operands.len());
                    for constraint in &row.operands {
                        let register = match constraint.access {
                            RegisterOperandAccess::Use => Some(action.surviving),
                            RegisterOperandAccess::Def => action.result,
                            _ => None,
                        };
                        registers.push(register.ok_or(LiteralFoldError::ConsumerMismatch {
                            function: function_index,
                        })?);
                    }
                    if !pair.rule.unit_effects().admits_consumer(&consumer) {
                        return Err(LiteralFoldError::ConsumerMismatch {
                            function: function_index,
                        });
                    }
                    consumer.kind = rewritten_kind;
                    consumer.constraint = action.immediate_constraint;
                    consumer.operands = row
                        .operands
                        .iter()
                        .zip(registers.iter())
                        .map(|(constraint, register)| selected_operand(constraint, *register))
                        .collect();
                    consumer.implicit_uses = row.implicit_uses.clone();
                    consumer.implicit_defs = row.implicit_defs.clone();
                    consumer.clobbers = row.clobbers.clone();
                    consumer.provenance = SelectedInstructionProvenance {
                        operations,
                        values: consumer_provenance.values,
                        edges: consumer_provenance.edges,
                        obligations: consumer_provenance.obligations,
                        work,
                    };
                    Ok(consumer)
                }

                fn redensify(
                    function_index: usize,
                    function: &mut SelectedFunction,
                    removed_instructions: &[SelectedInstructionId],
                    removed_registers: &[VirtualRegisterId],
                ) -> Result<(), LiteralFoldError> {
                    for call in &mut function.calls {
                        call.instruction = lower_instruction(
                            function_index,
                            call.instruction,
                            removed_instructions,
                        )?;
                    }
                    for access in &mut function.memory_accesses {
                        access.instruction = lower_instruction(
                            function_index,
                            access.instruction,
                            removed_instructions,
                        )?;
                    }
                    for register in &mut function.virtual_registers {
                        register.id =
                            lower_register(function_index, register.id, removed_registers)?;
                        match &mut register.origin {
                            VirtualRegisterOrigin::InstructionResult { instruction, .. }
                            | VirtualRegisterOrigin::SpillAddress { instruction, .. }
                            | VirtualRegisterOrigin::StructuralObservation {
                                instruction, ..
                            }
                            | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
                            | VirtualRegisterOrigin::InstructionScratch { instruction, .. }
                            | VirtualRegisterOrigin::AbiTransport { instruction, .. } => {
                                *instruction = lower_instruction(
                                    function_index,
                                    *instruction,
                                    removed_instructions,
                                )?;
                            }
                            VirtualRegisterOrigin::EntryParameter { .. }
                            | VirtualRegisterOrigin::StructuralParameter { .. }
                            | VirtualRegisterOrigin::BlockParameter { .. } => {}
                        }
                    }
                    for block in &mut function.blocks {
                        for instruction in &mut block.instructions {
                            lower_selected_instruction(
                                function_index,
                                instruction,
                                removed_instructions,
                                removed_registers,
                            )?;
                        }
                        let successors = terminator_successors_mut(&mut block.terminator);
                        for successor in successors {
                            for binding in &mut successor.structural_bindings {
                                if let selected_instructions::SelectedStructuralTransport::WholeValue {
                                    argument,
                                    ..
                                }
                                | selected_instructions::SelectedStructuralTransport::ScalarReference {
                                    base: selected_instructions::SelectedAddressBase::Register(argument),
                                    ..
                                }
                                | selected_instructions::SelectedStructuralTransport::Address {
                                    base: selected_instructions::SelectedAddressBase::Register(argument),
                                    ..
                                } = &mut binding.transport
                                {
                                    *argument = lower_register(function_index, *argument, removed_registers)?;
                                }
                            }
                            if let Some(case) = &mut successor.structural_case {
                                for payload in &mut case.payloads {
                                    match &mut payload.transport {
                                        selected_instructions::SelectedCasePayloadTransport::Unused => {}
                                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                                            parameter,
                                        } => {
                                            *parameter =
                                                lower_register(function_index, *parameter, removed_registers)?;
                                        }
                                        selected_instructions::SelectedCasePayloadTransport::Registers {
                                            argument,
                                            parameter,
                                        } => {
                                            *argument =
                                                lower_register(function_index, *argument, removed_registers)?;
                                            *parameter =
                                                lower_register(function_index, *parameter, removed_registers)?;
                                        }
                                    }
                                }
                            }
                            for binding in &mut successor.bindings {
                                if let selected_instructions::SelectedValueTransport::Registers {
                                    argument,
                                    parameter,
                                } = &mut binding.transport
                                {
                                    *argument = lower_register(
                                        function_index,
                                        *argument,
                                        removed_registers,
                                    )?;
                                    *parameter = lower_register(
                                        function_index,
                                        *parameter,
                                        removed_registers,
                                    )?;
                                }
                            }
                        }
                        lower_selected_instruction(
                            function_index,
                            selected_instructions::terminator_instruction_mut(
                                &mut block.terminator,
                            ),
                            removed_instructions,
                            removed_registers,
                        )?;
                    }
                    Ok(())
                }

                fn lower_selected_instruction(
                    function_index: usize,
                    instruction: &mut SelectedInstruction,
                    removed_instructions: &[SelectedInstructionId],
                    removed_registers: &[VirtualRegisterId],
                ) -> Result<(), LiteralFoldError> {
                    instruction.id =
                        lower_instruction(function_index, instruction.id, removed_instructions)?;
                    for operand in &mut instruction.operands {
                        operand.virtual_register = lower_register(
                            function_index,
                            operand.virtual_register,
                            removed_registers,
                        )?;
                    }
                    Ok(())
                }

                fn lower_id<Id: Copy + PartialEq>(
                    function_index: usize,
                    id: Id,
                    removed: &[Id],
                    raw: impl Fn(Id) -> u32 + Copy,
                ) -> Result<u32, LiteralFoldError> {
                    if removed.contains(&id) {
                        return Err(LiteralFoldError::IdentifierUnderflow {
                            function: function_index,
                        });
                    }
                    raw(id)
                        .checked_sub(
                            u32::try_from(
                                removed.partition_point(|removed| raw(*removed) < raw(id)),
                            )
                            .map_err(|_| {
                                LiteralFoldError::IdentifierUnderflow {
                                    function: function_index,
                                }
                            })?,
                        )
                        .ok_or(LiteralFoldError::IdentifierUnderflow {
                            function: function_index,
                        })
                }

                fn lower_instruction(
                    function_index: usize,
                    id: SelectedInstructionId,
                    removed: &[SelectedInstructionId],
                ) -> Result<SelectedInstructionId, LiteralFoldError> {
                    Ok(SelectedInstructionId(lower_id(
                        function_index,
                        id,
                        removed,
                        |id| id.0,
                    )?))
                }

                fn lower_register(
                    function_index: usize,
                    id: VirtualRegisterId,
                    removed: &[VirtualRegisterId],
                ) -> Result<VirtualRegisterId, LiteralFoldError> {
                    Ok(VirtualRegisterId(lower_id(
                        function_index,
                        id,
                        removed,
                        |id| id.0,
                    )?))
                }
            }
            pub(crate) mod functions {
                use selected_instructions::{SelectedFunction, SelectedInstructionPlan};

                use crate::{
                    FunctionLiteralFold, LiteralFoldError, ValidatedRecoveryClassifications,
                    ValidatedSelectedAnalysis,
                };

                use super::actions::derive_actions;
                use super::constraints::AdmittedPairs;
                use super::function_rewrite::apply_actions;
                use crate::rewrites::block_edges::block_instruction_ids;

                pub(crate) fn derive_function_folds(
                    selected: &impl ValidatedSelectedAnalysis,
                    recovery: &ValidatedRecoveryClassifications,
                    rows: &AdmittedPairs<'_>,
                ) -> Result<(Vec<FunctionLiteralFold>, SelectedInstructionPlan), LiteralFoldError>
                {
                    let source_plan = selected.selected_plan();
                    let mut transformed = source_plan.clone();
                    let mut functions = Vec::with_capacity(source_plan.functions.len());

                    for (function_index, source) in source_plan.functions.iter().enumerate() {
                        let recovery_function =
                            recovery.plan().functions.get(function_index).ok_or(
                                LiteralFoldError::FunctionMismatch {
                                    function: function_index,
                                },
                            )?;
                        if source.machine != recovery_function.machine {
                            return Err(LiteralFoldError::FunctionMismatch {
                                function: function_index,
                            });
                        }
                        validate_dense_identifiers(function_index, source)?;

                        let actions = derive_actions(source, rows);
                        if !actions.is_empty() {
                            apply_actions(
                                function_index,
                                &mut transformed.functions[function_index],
                                &actions,
                                rows,
                            )?;
                        }
                        functions.push(FunctionLiteralFold {
                            machine: source.machine,
                            actions,
                        });
                    }

                    Ok((functions, transformed))
                }

                fn validate_dense_identifiers(
                    function_index: usize,
                    function: &SelectedFunction,
                ) -> Result<(), LiteralFoldError> {
                    if !function.ids_are_positions() {
                        return Err(LiteralFoldError::FunctionMismatch {
                            function: function_index,
                        });
                    }
                    let mut ids = function
                        .blocks
                        .iter()
                        .flat_map(block_instruction_ids)
                        .collect::<Vec<_>>();
                    ids.sort_unstable();
                    let count =
                        u32::try_from(ids.len()).map_err(|_| LiteralFoldError::WorkOverflow)?;
                    if ids != (0..count).collect::<Vec<_>>() {
                        return Err(LiteralFoldError::FunctionMismatch {
                            function: function_index,
                        });
                    }
                    Ok(())
                }
            }
            pub(crate) mod usage {
                use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
                use register_model::RegisterOperandAccess;
                use selected_instructions::SelectedInstructionKind;

                use crate::rewrites::block_edges::terminator_successors;
                use crate::{
                    FunctionLiteralFold, LiteralFoldError, PairUnitDefRelation,
                    ValidatedSelectedAnalysis,
                };

                use super::constraints::AdmittedPairs;

                fn successor_sites(function: &selected_instructions::SelectedFunction) -> u64 {
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| {
                            selected_instructions::terminator_successors(&block.terminator)
                                .into_iter()
                        })
                        .map(|successor| {
                            successor.structural_bindings.len()
                                + successor
                                    .structural_case
                                    .iter()
                                    .map(|case| case.payloads.len())
                                    .sum::<usize>()
                                + successor.bindings.len()
                        })
                        .sum::<usize>() as u64
                }

                pub(crate) fn fold_usage(
                    selected: &impl ValidatedSelectedAnalysis,
                    functions: &[FunctionLiteralFold],
                    rows: &AdmittedPairs<'_>,
                ) -> Result<OptimizationWorkUsage, LiteralFoldError> {
                    let plan = selected.selected_plan();
                    let function_count = u64::try_from(plan.functions.len())
                        .map_err(|_| LiteralFoldError::WorkOverflow)?;
                    let mut validation_steps = plan
                        .functions
                        .iter()
                        .try_fold(0_u64, |total, function| {
                            let instructions =
                                function.blocks.iter().try_fold(0_u64, |count, block| {
                                    count.checked_add(
                                        u64::try_from(block.instructions.len())
                                            .ok()?
                                            .checked_add(1)?,
                                    )
                                })?;
                            total
                                .checked_add(u64::try_from(function.virtual_registers.len()).ok()?)?
                                .checked_add(instructions)
                        })
                        .ok_or(LiteralFoldError::WorkOverflow)?;
                    for function in &plan.functions {
                        let producers = function
                            .instructions()
                            .filter(|instruction| {
                                matches!(
                                    instruction.kind,
                                    SelectedInstructionKind::MaterializeI64 { .. }
                                )
                            })
                            .count();
                        let per_producer = function
                            .blocks
                            .iter()
                            .try_fold(0_u64, |count, block| {
                                count.checked_add(u64::try_from(block.instructions.len() + 1).ok()?)
                            })
                            .and_then(|sites| sites.checked_add(successor_sites(function)))
                            .ok_or(LiteralFoldError::WorkOverflow)?;
                        validation_steps = validation_steps
                            .checked_add(
                                u64::try_from(producers)
                                    .map_err(|_| LiteralFoldError::WorkOverflow)?
                                    .checked_mul(per_producer)
                                    .ok_or(LiteralFoldError::WorkOverflow)?,
                            )
                            .ok_or(LiteralFoldError::WorkOverflow)?;
                    }
                    let mut applied = 0_u64;
                    for (function, fold) in plan.functions.iter().zip(functions.iter()) {
                        for action in &fold.actions {
                            applied = applied
                                .checked_add(1)
                                .ok_or(LiteralFoldError::WorkOverflow)?;
                            let Some(consumer) = function
                                .block(action.block)
                                .and_then(|block| block.instruction(action.consumer_instruction))
                            else {
                                continue;
                            };
                            let Some(victim_position) = consumer
                                .operands
                                .iter()
                                .position(|operand| {
                                    operand.access == RegisterOperandAccess::Use
                                        && operand.virtual_register == action.victim
                                })
                                .and_then(|position| u16::try_from(position).ok())
                            else {
                                continue;
                            };
                            let audited = rows
                                .for_position(consumer.kind, victim_position)
                                .is_some_and(|pair| {
                                    pair.rule.machine_effects().unit_defs
                                        == PairUnitDefRelation::OperandSwapped
                                });
                            if !audited {
                                continue;
                            }
                            let scan = function
                                .blocks
                                .iter()
                                .try_fold(0_u64, |count, block| {
                                    count.checked_add(
                                        u64::try_from(block.instructions.len() + 1).ok()?,
                                    )
                                })
                                .ok_or(LiteralFoldError::WorkOverflow)?;
                            let edges = function
                                .blocks
                                .iter()
                                .try_fold(0_u64, |count, block| {
                                    count.checked_add(
                                        u64::try_from(
                                            terminator_successors(&block.terminator).len(),
                                        )
                                        .ok()?,
                                    )
                                })
                                .ok_or(LiteralFoldError::WorkOverflow)?;
                            validation_steps = validation_steps
                                .checked_add(
                                    u64::try_from(consumer.implicit_defs.len())
                                        .map_err(|_| LiteralFoldError::WorkOverflow)?
                                        .checked_mul(
                                            scan.checked_add(edges)
                                                .and_then(|per_visit| per_visit.checked_mul(2))
                                                .ok_or(LiteralFoldError::WorkOverflow)?,
                                        )
                                        .ok_or(LiteralFoldError::WorkOverflow)?,
                                )
                                .ok_or(LiteralFoldError::WorkOverflow)?;
                        }
                    }
                    Ok(OptimizationWorkUsage {
                        rule_evaluations: function_count,
                        candidates: applied,
                        validation_steps,
                        commits: applied,
                        iterations: 1,
                    })
                }

                pub(crate) fn ensure_budget(
                    usage: OptimizationWorkUsage,
                    budget: OptimizationWorkBudget,
                ) -> Result<(), LiteralFoldError> {
                    crate::work_accounting::admit(usage, budget, |required, budget| {
                        LiteralFoldError::BudgetExceeded { required, budget }
                    })
                }
            }

            use optimization_core::OptimizationWorkBudget;
            use register_model::{
                TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
                ValidatedRegisterConstraintCatalog,
            };
            use selected_instructions::{PreparedMachineEffectCatalog, SelectedInstructionPlan};

            use crate::{
                LiteralFoldError, LiteralFoldPlan, LiteralFoldPolicy, ValidatedAllocationLegality,
                ValidatedAllocatorAvailability, ValidatedLiveRanges,
                ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
            };

            use constraints::select_admitted_pairs;
            use functions::derive_function_folds;
            use usage::{ensure_budget, fold_usage};

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn compute_terminal_literal_fold<S: ValidatedSelectedAnalysis>(
                selected: &S,
                ranges: &ValidatedLiveRanges,
                legality: &ValidatedAllocationLegality,
                spill_choices: &ValidatedSpillChoices,
                recovery: &ValidatedRecoveryClassifications,
                availability: &ValidatedAllocatorAvailability,
                register_environment: TargetRegisterEnvironmentIdentity,
                constraints: &ValidatedRegisterConstraintCatalog,
                selected_keys: &TargetRegisterEnvironmentConstraintKeys,
                effect_catalog: &PreparedMachineEffectCatalog,
                policy: LiteralFoldPolicy,
                budget: OptimizationWorkBudget,
            ) -> Result<(LiteralFoldPlan, SelectedInstructionPlan), LiteralFoldError> {
                let rows =
                    select_admitted_pairs(constraints, selected_keys, policy, effect_catalog)?;
                let (functions, transformed) = derive_function_folds(selected, recovery, &rows)?;
                let usage = fold_usage(selected, &functions, &rows)?;
                ensure_budget(usage, budget)?;

                let transformed_selected = selected_instruction_plan_identity(&transformed);
                let plan = LiteralFoldPlan {
                    source_selected: selected.selected_identity(),
                    spill_choices: spill_choices.receipt().identity(),
                    recovery_classifications: recovery.receipt().identity(),
                    ranges: ranges.receipt().identity(),
                    legality: legality.receipt().identity(),
                    register_environment,
                    allocator_availability: availability.receipt().identity(),
                    machine_effect_catalog: effect_catalog.identity(),
                    optimization_unit: selected.optimization_unit_identity(),
                    work_schedule: selected.work_schedule_identity(),
                    policy,
                    budget,
                    usage,
                    functions,
                    transformed_selected,
                };
                Ok((plan, transformed))
            }
            use selected_instructions::selected_instruction_plan_identity;
        }
        pub(crate) mod identity {
            use selected_instructions::LiteralFoldIdentity;

            use crate::LiteralFoldPlan;
            use crate::codec_bytes::push_encoded;

            pub(crate) fn literal_fold_identity(plan: &LiteralFoldPlan) -> LiteralFoldIdentity {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(b"omega.terminal-literal-fold.v8\0");
                bytes.extend_from_slice(&encode_terminal_literal_fold_content(plan));
                LiteralFoldIdentity::from_canonical_bytes(&bytes)
            }

            pub(crate) fn encode_terminal_literal_fold_content(plan: &LiteralFoldPlan) -> Vec<u8> {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&plan.source_selected.bytes());
                bytes.extend_from_slice(&plan.spill_choices.bytes());
                bytes.extend_from_slice(&plan.recovery_classifications.bytes());
                bytes.extend_from_slice(&plan.ranges.bytes());
                bytes.extend_from_slice(&plan.legality.bytes());
                bytes.extend_from_slice(&plan.register_environment.bytes());
                bytes.extend_from_slice(&plan.allocator_availability.bytes());
                bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());
                bytes.extend_from_slice(&plan.optimization_unit.bytes());
                push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
                push_encoded(&mut bytes, plan.policy.canonical_bits().to_le_bytes());
                bytes.extend_from_slice(&plan.budget.encode());
                bytes.extend_from_slice(&plan.usage.encode());
                length(&mut bytes, plan.functions.len());
                for function in &plan.functions {
                    push_encoded(&mut bytes, function.machine.get().to_le_bytes());
                    length(&mut bytes, function.actions.len());
                    for action in &function.actions {
                        push_encoded(&mut bytes, action.block.0.to_le_bytes());
                        push_encoded(&mut bytes, action.literal_instruction.0.to_le_bytes());
                        push_encoded(&mut bytes, action.victim.0.to_le_bytes());
                        push_encoded(&mut bytes, action.consumer_instruction.0.to_le_bytes());
                        push_encoded(&mut bytes, action.surviving.0.to_le_bytes());
                        match action.result {
                            None => bytes.push(0),
                            Some(result) => {
                                bytes.push(1);
                                push_encoded(&mut bytes, result.0.to_le_bytes());
                            }
                        }
                        push_encoded(&mut bytes, action.immediate.to_le_bytes());
                        register_model::encode_constraint_key(
                            &mut bytes,
                            action.immediate_constraint,
                        );
                    }
                }
                bytes.extend_from_slice(&plan.transformed_selected.bytes());
                bytes
            }

            fn length(bytes: &mut Vec<u8>, value: usize) {
                bytes.extend_from_slice(
                    &u64::try_from(value)
                        .expect("literal-fold identity length fits u64")
                        .to_le_bytes(),
                );
            }
        }
        pub(crate) mod pair_rule {
            use register_model::{
                RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
                RegisterOperandConstraint, RegisterUnitId, TargetRegisterEnvironmentConstraintKeys,
            };
            use selected_instructions::{
                MachineAlternative, MachineEffectDeclaration, MachineEncodedControlEffect,
                MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
                MachineMemoryEffect, MachineSemanticKind, SaturatingCarrier, SelectedFunction,
                SelectedInstruction, SelectedInstructionKind, SelectedOperand, VirtualRegisterId,
            };
            use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

            use crate::analyses::machine_effects::machine_semantic_kind;
            use crate::rewrites::block_edges::{block_instructions, terminator_successors};
            use crate::rewrites::operand_probes::isolated_declaration;

            pub(crate) mod predicates {
                use super::*;

                pub(crate) fn indexed_pointer_read_matches(
                    consumer: &MachineAlternative,
                    rewritten: &MachineAlternative,
                    index_operand: u16,
                ) -> bool {
                    let (
                        MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                            pointer_operand,
                            index_operand: index,
                            byte_count,
                        },
                        MachineEncodedMemoryEffect::ReadPointerV1 {
                            pointer_operand: rewritten_pointer,
                            byte_count: rewritten_bytes,
                        },
                    ) = (consumer.encoded.memory, rewritten.encoded.memory)
                    else {
                        return false;
                    };
                    index == index_operand
                        && pointer_operand == rewritten_pointer
                        && byte_count == rewritten_bytes
                        && consumer.encoded.stack == rewritten.encoded.stack
                        && consumer.encoded.trap == rewritten.encoded.trap
                        && consumer.encoded.control == rewritten.encoded.control
                        && consumer.encoded.implicit_unit_uses.is_empty()
                        && consumer
                            .encoded
                            .implicit_unit_defs
                            .iter()
                            .all(|unit| rewritten.encoded.implicit_unit_defs.contains(unit))
                        && rewritten.encoded.implicit_unit_uses.is_empty()
                        && rewritten.encoded.implicit_unit_clobbers.is_empty()
                }

                pub(crate) fn implicit_defs_covered(
                    consumer: &MachineAlternative,
                    rewritten: &MachineEffectDeclaration,
                ) -> bool {
                    rewritten.alternatives.iter().all(|alternative| {
                        consumer
                            .encoded
                            .implicit_unit_defs
                            .iter()
                            .all(|unit| alternative.encoded.implicit_unit_defs.contains(unit))
                    })
                }

                pub(crate) fn isolated_alternative(alternative: &MachineAlternative) -> bool {
                    let encoded = &alternative.encoded;
                    encoded.memory == MachineEncodedMemoryEffect::NoneV1
                        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
                        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
                        && encoded.control == MachineEncodedControlEffect::FallThroughV1
                }

                pub(crate) fn fault_discharged_alternative(
                    alternative: &MachineAlternative,
                ) -> bool {
                    let encoded = &alternative.encoded;
                    encoded.memory == MachineEncodedMemoryEffect::NoneV1
                        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
                        && matches!(
                            encoded.trap,
                            MachineEncodedTrapBehavior::NeverV1
                                | MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                        )
                        && encoded.control == MachineEncodedControlEffect::FallThroughV1
                }

                pub(crate) fn implicit_unit_used(
                    function: &SelectedFunction,
                    unit: RegisterUnitId,
                ) -> bool {
                    function.blocks.iter().any(|block| {
                        block_instructions(block)
                            .any(|instruction| instruction.implicit_uses.contains(&unit))
                    })
                }

                pub(crate) fn equality_sensing_reader(kind: SelectedInstructionKind) -> bool {
                    matches!(
                        kind,
                        SelectedInstructionKind::MaterializeBooleanEqual
                            | SelectedInstructionKind::ConditionalBranchNonZero
                    )
                }

                pub(crate) fn swapped_unit_readers_equality_only(
                    function: &SelectedFunction,
                    block_index: usize,
                    consumer_index: usize,
                    unit: RegisterUnitId,
                ) -> bool {
                    let mut visited = std::collections::BTreeSet::new();
                    let mut frontier = vec![(block_index, consumer_index + 1)];
                    while let Some((current, start)) = frontier.pop() {
                        if !visited.insert((current, start)) {
                            continue;
                        }
                        let block = &function.blocks[current];
                        let mut killed = false;
                        for instruction in block_instructions(block).skip(start) {
                            if instruction.implicit_uses.contains(&unit)
                                && !equality_sensing_reader(instruction.kind)
                            {
                                return false;
                            }
                            if instruction.implicit_defs.contains(&unit)
                                || instruction.clobbers.contains(&unit)
                            {
                                killed = true;
                                break;
                            }
                        }
                        if killed {
                            continue;
                        }
                        for successor in terminator_successors(&block.terminator) {
                            let Some(target) = function.block_index(successor.block) else {
                                return false;
                            };
                            frontier.push((target, 0));
                        }
                    }
                    true
                }

                pub(crate) fn dead_scratch(
                    function: &SelectedFunction,
                    operands: &[SelectedOperand],
                ) -> bool {
                    operands.iter().all(|operand| {
                        operand.access == RegisterOperandAccess::Def
                            && dropped_def_is_dead(function, operand.virtual_register)
                    })
                }

                pub(crate) fn auxiliary_uses(
                    function: &SelectedFunction,
                    operands: &[SelectedOperand],
                ) -> bool {
                    operands.iter().all(|operand| {
                        operand.access == RegisterOperandAccess::Use
                            && auxiliary_zero_defined(function, operand.virtual_register)
                    })
                }

                pub(crate) fn droppable_tail(
                    function: &SelectedFunction,
                    operands: &[SelectedOperand],
                ) -> bool {
                    operands.iter().all(|operand| match operand.access {
                        RegisterOperandAccess::Use => {
                            auxiliary_zero_defined(function, operand.virtual_register)
                        }
                        RegisterOperandAccess::Def => {
                            dropped_def_is_dead(function, operand.virtual_register)
                        }
                        _ => false,
                    })
                }

                fn auxiliary_zero_defined(
                    function: &SelectedFunction,
                    register: VirtualRegisterId,
                ) -> bool {
                    let mut definitions = function.instructions().filter(|instruction| {
                        instruction.operands.iter().any(|operand| {
                            operand.access == RegisterOperandAccess::Def
                                && operand.virtual_register == register
                        })
                    });
                    let mut saw_definition = false;
                    definitions.all(|instruction| {
                        saw_definition = true;
                        matches!(
                            instruction.kind,
                            SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(0)
                            }
                        )
                    }) && saw_definition
                }

                fn dropped_def_is_dead(
                    function: &SelectedFunction,
                    register: VirtualRegisterId,
                ) -> bool {
                    crate::rewrites::register_occurs_once(function, register)
                }
            }
            pub(crate) mod rules {
                use super::*;

                impl SelectedInstructionPairRule {
                    pub(crate) const EXACT_ADD_IMMEDIATE_U12: Self = Self {
                        producer: MachineSemanticKind::MaterializeI64,
                        consumer: MachineSemanticKind::ExactAddI64,
                        rewritten: MachineSemanticKind::ExactAddI64Immediate,
                        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                        immediate_bound: PairImmediateBound::Encoding(4095),
                        result: PairResultDisposition::ScalarRegister,
                        unit_effects: PairUnitEffects::ISOLATED,
                        machine_effects: PairMachineEffects::ISOLATED,
                    };
                    pub(crate) const EXACT_ADD_LEFT_IMMEDIATE_U12: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
                        ..Self::EXACT_ADD_IMMEDIATE_U12
                    };
                    pub(crate) const EXACT_SUBTRACT_IMMEDIATE_U12: Self = Self {
                        producer: MachineSemanticKind::MaterializeI64,
                        consumer: MachineSemanticKind::ExactSubtractI64,
                        rewritten: MachineSemanticKind::ExactSubtractI64Immediate,
                        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                        immediate_bound: PairImmediateBound::Encoding(4095),
                        result: PairResultDisposition::ScalarRegister,
                        unit_effects: PairUnitEffects::ISOLATED,
                        machine_effects: PairMachineEffects::ISOLATED,
                    };
                    pub(crate) const COMPARE_IMMEDIATE_U12: Self = Self {
                        producer: MachineSemanticKind::MaterializeI64,
                        consumer: MachineSemanticKind::CompareI64,
                        rewritten: MachineSemanticKind::CompareI64Immediate,
                        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                        immediate_bound: PairImmediateBound::EncodingNonZero(4095),
                        result: PairResultDisposition::ImplicitUnits,
                        unit_effects: PairUnitEffects::ISOLATED,
                        machine_effects: PairMachineEffects::ISOLATED,
                    };
                    pub(crate) const COMPARE_LEFT_IMMEDIATE_U12: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_OPERAND_SWAP,
                        machine_effects: PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS,
                        ..Self::COMPARE_IMMEDIATE_U12
                    };
                    pub(crate) const COMPARE_ZERO: Self = Self {
                        producer: MachineSemanticKind::MaterializeI64,
                        consumer: MachineSemanticKind::CompareI64,
                        rewritten: MachineSemanticKind::CompareI64Zero,
                        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                        immediate_bound: PairImmediateBound::Exactly(0),
                        result: PairResultDisposition::ImplicitUnits,
                        unit_effects: PairUnitEffects::ISOLATED,
                        machine_effects: PairMachineEffects::ISOLATED,
                    };
                    pub(crate) const COMPARE_LEFT_ZERO: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_OPERAND_SWAP,
                        machine_effects: PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS,
                        ..Self::COMPARE_ZERO
                    };

                    const UNARY_MATERIALIZE_FOLD: Self = Self {
                        producer: MachineSemanticKind::MaterializeI64,
                        consumer: MachineSemanticKind::MaterializeI64,
                        rewritten: MachineSemanticKind::MaterializeI64,
                        operand_shape: PairOperandShape::UNARY_LITERAL,
                        immediate_bound: PairImmediateBound::Encoding(u64::MAX),
                        result: PairResultDisposition::ScalarRegister,
                        unit_effects: PairUnitEffects::ISOLATED,
                        machine_effects: PairMachineEffects::ISOLATED,
                    };
                    pub(crate) const ZERO_EXTEND_U8_LITERAL_FOLD: Self = Self {
                        consumer: MachineSemanticKind::ZeroExtendU8,
                        ..Self::UNARY_MATERIALIZE_FOLD
                    };
                    pub(crate) const ZERO_EXTEND_U16_LITERAL_FOLD: Self = Self {
                        consumer: MachineSemanticKind::ZeroExtendU16,
                        ..Self::UNARY_MATERIALIZE_FOLD
                    };
                    pub(crate) const ZERO_EXTEND_U32_LITERAL_FOLD: Self = Self {
                        consumer: MachineSemanticKind::ZeroExtendU32,
                        ..Self::UNARY_MATERIALIZE_FOLD
                    };
                    pub(crate) const SIGN_EXTEND_I8_LITERAL_FOLD: Self = Self {
                        consumer: MachineSemanticKind::SignExtendI8,
                        ..Self::UNARY_MATERIALIZE_FOLD
                    };
                    pub(crate) const SIGN_EXTEND_I16_LITERAL_FOLD: Self = Self {
                        consumer: MachineSemanticKind::SignExtendI16,
                        ..Self::UNARY_MATERIALIZE_FOLD
                    };
                    pub(crate) const SIGN_EXTEND_I32_LITERAL_FOLD: Self = Self {
                        consumer: MachineSemanticKind::SignExtendI32,
                        ..Self::UNARY_MATERIALIZE_FOLD
                    };
                    pub(crate) const EXTENSION_LITERAL_FOLDS: [Self; 6] = [
                        Self::ZERO_EXTEND_U8_LITERAL_FOLD,
                        Self::ZERO_EXTEND_U16_LITERAL_FOLD,
                        Self::ZERO_EXTEND_U32_LITERAL_FOLD,
                        Self::SIGN_EXTEND_I8_LITERAL_FOLD,
                        Self::SIGN_EXTEND_I16_LITERAL_FOLD,
                        Self::SIGN_EXTEND_I32_LITERAL_FOLD,
                    ];

                    pub(crate) const COPY_LITERAL_FOLD: Self = Self {
                        consumer: MachineSemanticKind::CopyI64,
                        ..Self::UNARY_MATERIALIZE_FOLD
                    };

                    pub(crate) const LOAD8_INDEXED_U12: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::Load8Indexed,
                            rewritten: MachineSemanticKind::Load8,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                            immediate_bound: PairImmediateBound::Encoding(4095),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::ISOLATED,
                            machine_effects: PairMachineEffects::indexed_pointer_read_fold(1),
                        };
                        assert!(
                            matches!(
                                rule.machine_effects.non_unit,
                                PairNonUnitSurface::IndexedPointerRead { index_operand }
                                    if index_operand == rule.victim_operand()
                            ),
                            "the folded operand is the indexed read's index operand"
                        );
                        rule
                    };

                    pub(crate) const BYTE_VIEW_ADDRESS_OFFSET_U12: Self = Self {
                        producer: MachineSemanticKind::MaterializeI64,
                        consumer: MachineSemanticKind::ByteViewAddress,
                        rewritten: MachineSemanticKind::AddressOffset,
                        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                        immediate_bound: PairImmediateBound::Encoding(4095),
                        result: PairResultDisposition::ScalarRegister,
                        unit_effects: PairUnitEffects::ISOLATED,
                        machine_effects: PairMachineEffects::ISOLATED,
                    };
                    pub(crate) const BYTE_VIEW_ADDRESS_BACKING_U12: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
                        ..Self::BYTE_VIEW_ADDRESS_OFFSET_U12
                    };

                    pub(crate) const EXACT_DIVIDE_ONE_COPY: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::ExactDivideU64,
                            rewritten: MachineSemanticKind::CopyI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES,
                            immediate_bound: PairImmediateBound::Exactly(1),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
                        };
                        assert!(
                            matches!(
                                rule.machine_effects,
                                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
                            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
                            "the fault discharge holds only for the divisor literal one"
                        );
                        rule
                    };

                    pub(crate) const EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::ExactDivideU64,
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape:
                                PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION,
                        };
                        assert!(
                            matches!(
                                rule.machine_effects,
                                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION
                            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the obligation discharge holds only for the dividend literal zero"
                        );
                        rule
                    };

                    pub(crate) const WRAPPING_REMAINDER_ONE_MATERIALIZE: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::WrappingRemainderI64,
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
                            immediate_bound: PairImmediateBound::Exactly(1),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
                        };
                        assert!(
                            matches!(
                                rule.machine_effects,
                                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
                            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
                            "the fault discharge holds only for the divisor literal one"
                        );
                        rule
                    };

                    pub(crate) const WRAPPING_REMAINDER_MINUS_ONE_MATERIALIZE: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::WrappingRemainderI64,
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
                            immediate_bound: PairImmediateBound::Exactly(u64::MAX),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
                        };
                        assert!(
                            matches!(
                                rule.machine_effects,
                                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
                            ) && matches!(
                                rule.immediate_bound,
                                PairImmediateBound::Exactly(u64::MAX)
                            ),
                            "the fault discharge holds only for the divisor literal minus one"
                        );
                        rule
                    };

                    pub(crate) const WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::WrappingRemainderI64,
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION,
                        };
                        assert!(
                            matches!(
                                rule.machine_effects,
                                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION
                            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the obligation discharge holds only for the dividend literal zero"
                        );
                        rule
                    };

                    pub(crate) const BITWISE_AND_ZERO_MATERIALIZE: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::BitwiseAndI64,
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::ISOLATED,
                            machine_effects: PairMachineEffects::ISOLATED,
                        };
                        assert!(
                            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the annihilator fold holds only for the literal zero"
                        );
                        rule
                    };

                    pub(crate) const BITWISE_AND_ZERO_LEFT_MATERIALIZE: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
                        ..Self::BITWISE_AND_ZERO_MATERIALIZE
                    };

                    pub(crate) const BITWISE_AND_ZERO_FOLDS: [Self; 2] = [
                        Self::BITWISE_AND_ZERO_MATERIALIZE,
                        Self::BITWISE_AND_ZERO_LEFT_MATERIALIZE,
                    ];

                    const ISOLATED_ZERO_IDENTITY_COPY: Self = Self {
                        producer: MachineSemanticKind::MaterializeI64,
                        consumer: MachineSemanticKind::MaterializeI64,
                        rewritten: MachineSemanticKind::CopyI64,
                        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                        immediate_bound: PairImmediateBound::Exactly(0),
                        result: PairResultDisposition::ScalarRegister,
                        unit_effects: PairUnitEffects::ISOLATED,
                        machine_effects: PairMachineEffects::ISOLATED,
                    };

                    pub(crate) const BITWISE_XOR_ZERO_COPY: Self = Self {
                        consumer: MachineSemanticKind::BitwiseXorI64,
                        ..Self::ISOLATED_ZERO_IDENTITY_COPY
                    };

                    pub(crate) const BITWISE_XOR_ZERO_LEFT_COPY: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
                        ..Self::BITWISE_XOR_ZERO_COPY
                    };

                    pub(crate) const BITWISE_XOR_ZERO_COPIES: [Self; 2] = [
                        Self::BITWISE_XOR_ZERO_COPY,
                        Self::BITWISE_XOR_ZERO_LEFT_COPY,
                    ];

                    pub(crate) const WRAPPING_ADD_ZERO_COPY: Self = Self {
                        consumer: MachineSemanticKind::WrappingAddI64,
                        ..Self::ISOLATED_ZERO_IDENTITY_COPY
                    };

                    pub(crate) const WRAPPING_ADD_ZERO_LEFT_COPY: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
                        ..Self::WRAPPING_ADD_ZERO_COPY
                    };

                    pub(crate) const WRAPPING_ADD_ZERO_COPIES: [Self; 2] = [
                        Self::WRAPPING_ADD_ZERO_COPY,
                        Self::WRAPPING_ADD_ZERO_LEFT_COPY,
                    ];

                    pub(crate) const BITWISE_AND_ONES_COPY: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::BitwiseAndI64,
                            rewritten: MachineSemanticKind::CopyI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                            immediate_bound: PairImmediateBound::Exactly(u64::MAX),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::ISOLATED,
                            machine_effects: PairMachineEffects::ISOLATED,
                        };
                        assert!(
                            matches!(rule.immediate_bound, PairImmediateBound::Exactly(u64::MAX)),
                            "the identity fold holds only for the all-ones literal"
                        );
                        rule
                    };

                    pub(crate) const BITWISE_AND_ONES_LEFT_COPY: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
                        ..Self::BITWISE_AND_ONES_COPY
                    };

                    pub(crate) const BITWISE_AND_ONES_COPIES: [Self; 2] = [
                        Self::BITWISE_AND_ONES_COPY,
                        Self::BITWISE_AND_ONES_LEFT_COPY,
                    ];

                    pub(crate) const SATURATING_ADD_ZERO_COPY: Self = {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingAdd(SaturatingCarrier::U64),
                            rewritten: MachineSemanticKind::CopyI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
                        };
                        assert!(
                            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the identity fold holds only for the literal zero"
                        );
                        rule
                    };

                    pub(crate) const SATURATING_ADD_ZERO_LEFT_COPY: Self = Self {
                        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
                        ..Self::SATURATING_ADD_ZERO_COPY
                    };

                    const fn saturating_add_zero_clamped(carrier: SaturatingCarrier) -> Self {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingAdd(carrier),
                            rewritten: MachineSemanticKind::CopyI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
                        };
                        assert!(
                            !matches!(carrier, SaturatingCarrier::U64)
                                && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the clamped identity fold holds only for a non-u64 carrier's zero literal"
                        );
                        rule
                    }

                    const fn saturating_add_zero_clamped_left(carrier: SaturatingCarrier) -> Self {
                        Self {
                            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_SCRATCH_DEFS,
                            ..Self::saturating_add_zero_clamped(carrier)
                        }
                    }

                    pub(crate) const SATURATING_ADD_ZERO_COPIES: [Self; 16] = [
                        Self::SATURATING_ADD_ZERO_COPY,
                        Self::SATURATING_ADD_ZERO_LEFT_COPY,
                        Self::saturating_add_zero_clamped(SaturatingCarrier::I8),
                        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I8),
                        Self::saturating_add_zero_clamped(SaturatingCarrier::I16),
                        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I16),
                        Self::saturating_add_zero_clamped(SaturatingCarrier::I32),
                        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I32),
                        Self::saturating_add_zero_clamped(SaturatingCarrier::I64),
                        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I64),
                        Self::saturating_add_zero_clamped(SaturatingCarrier::U8),
                        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U8),
                        Self::saturating_add_zero_clamped(SaturatingCarrier::U16),
                        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U16),
                        Self::saturating_add_zero_clamped(SaturatingCarrier::U32),
                        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U32),
                    ];

                    const fn saturating_add_upper_bound(carrier: SaturatingCarrier) -> Self {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingAdd(carrier),
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
                            immediate_bound: PairImmediateBound::Exactly(carrier.maximum_bits()),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
                        };
                        assert!(
                            !carrier.is_signed()
                                && matches!(
                                    rule.immediate_bound,
                                    PairImmediateBound::Exactly(bound) if bound == carrier.maximum_bits()
                                ),
                            "the upper-bound fold holds only for an unsigned carrier's maximum literal"
                        );
                        rule
                    }

                    const fn saturating_add_upper_bound_left(carrier: SaturatingCarrier) -> Self {
                        Self {
                            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
                            ..Self::saturating_add_upper_bound(carrier)
                        }
                    }

                    pub(crate) const SATURATING_ADD_UPPER_BOUND_MATERIALIZATIONS: [Self; 8] = [
                        Self::saturating_add_upper_bound(SaturatingCarrier::U8),
                        Self::saturating_add_upper_bound_left(SaturatingCarrier::U8),
                        Self::saturating_add_upper_bound(SaturatingCarrier::U16),
                        Self::saturating_add_upper_bound_left(SaturatingCarrier::U16),
                        Self::saturating_add_upper_bound(SaturatingCarrier::U32),
                        Self::saturating_add_upper_bound_left(SaturatingCarrier::U32),
                        Self::saturating_add_upper_bound(SaturatingCarrier::U64),
                        Self::saturating_add_upper_bound_left(SaturatingCarrier::U64),
                    ];

                    const fn saturating_subtract_zero_unsigned(carrier: SaturatingCarrier) -> Self {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
                            rewritten: MachineSemanticKind::CopyI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
                        };
                        assert!(
                            !carrier.is_signed()
                                && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the unsigned identity fold holds only for an unsigned carrier's zero literal"
                        );
                        rule
                    }

                    const fn saturating_subtract_zero_clamped(carrier: SaturatingCarrier) -> Self {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
                            rewritten: MachineSemanticKind::CopyI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
                        };
                        assert!(
                            carrier.is_signed()
                                && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the clamped identity fold holds only for a signed carrier's zero literal"
                        );
                        rule
                    }

                    pub(crate) const SATURATING_SUBTRACT_ZERO_COPIES: [Self; 8] = [
                        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U8),
                        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U16),
                        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U32),
                        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U64),
                        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I8),
                        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I16),
                        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I32),
                        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I64),
                    ];

                    const fn saturating_subtract_zero_minuend(carrier: SaturatingCarrier) -> Self {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
                        };
                        assert!(
                            !carrier.is_signed()
                                && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the zero-minuend fold holds only for an unsigned carrier's zero literal"
                        );
                        rule
                    }

                    pub(crate) const SATURATING_SUBTRACT_ZERO_MINUEND_MATERIALIZATIONS: [Self; 4] = [
                        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U8),
                        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U16),
                        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U32),
                        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U64),
                    ];

                    const fn saturating_subtract_upper_bound(carrier: SaturatingCarrier) -> Self {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
                            immediate_bound: PairImmediateBound::Exactly(carrier.maximum_bits()),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
                        };
                        assert!(
                            !carrier.is_signed()
                                && matches!(
                                    rule.immediate_bound,
                                    PairImmediateBound::Exactly(bound) if bound == carrier.maximum_bits()
                                ),
                            "the upper-bound subtrahend fold holds only for an unsigned carrier's maximum literal"
                        );
                        rule
                    }

                    pub(crate) const SATURATING_SUBTRACT_UPPER_BOUND_MATERIALIZATIONS: [Self; 4] = [
                        Self::saturating_subtract_upper_bound(SaturatingCarrier::U8),
                        Self::saturating_subtract_upper_bound(SaturatingCarrier::U16),
                        Self::saturating_subtract_upper_bound(SaturatingCarrier::U32),
                        Self::saturating_subtract_upper_bound(SaturatingCarrier::U64),
                    ];

                    const fn saturating_divide_one(carrier: SaturatingCarrier) -> Self {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingDivide(carrier),
                            rewritten: MachineSemanticKind::CopyI64,
                            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS,
                            immediate_bound: PairImmediateBound::Exactly(1),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS,
                        };
                        assert!(
                            matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
                            "the fault discharge holds only for the divisor literal one"
                        );
                        rule
                    }

                    pub(crate) const SATURATING_DIVIDE_ONE_COPIES: [Self; 8] = [
                        Self::saturating_divide_one(SaturatingCarrier::U8),
                        Self::saturating_divide_one(SaturatingCarrier::U16),
                        Self::saturating_divide_one(SaturatingCarrier::U32),
                        Self::saturating_divide_one(SaturatingCarrier::U64),
                        Self::saturating_divide_one(SaturatingCarrier::I8),
                        Self::saturating_divide_one(SaturatingCarrier::I16),
                        Self::saturating_divide_one(SaturatingCarrier::I32),
                        Self::saturating_divide_one(SaturatingCarrier::I64),
                    ];

                    const fn saturating_divide_zero_dividend(carrier: SaturatingCarrier) -> Self {
                        let rule = Self {
                            producer: MachineSemanticKind::MaterializeI64,
                            consumer: MachineSemanticKind::SaturatingDivide(carrier),
                            rewritten: MachineSemanticKind::MaterializeI64,
                            operand_shape:
                                PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS,
                            immediate_bound: PairImmediateBound::Exactly(0),
                            result: PairResultDisposition::ScalarRegister,
                            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
                            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS,
                        };
                        assert!(
                            matches!(
                                rule.machine_effects,
                                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS
                            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
                            "the obligation discharge holds only for the dividend literal zero"
                        );
                        rule
                    }

                    pub(crate) const SATURATING_DIVIDE_ZERO_DIVIDEND_MATERIALIZATIONS: [Self; 8] = [
                        Self::saturating_divide_zero_dividend(SaturatingCarrier::U8),
                        Self::saturating_divide_zero_dividend(SaturatingCarrier::U16),
                        Self::saturating_divide_zero_dividend(SaturatingCarrier::U32),
                        Self::saturating_divide_zero_dividend(SaturatingCarrier::U64),
                        Self::saturating_divide_zero_dividend(SaturatingCarrier::I8),
                        Self::saturating_divide_zero_dividend(SaturatingCarrier::I16),
                        Self::saturating_divide_zero_dividend(SaturatingCarrier::I32),
                        Self::saturating_divide_zero_dividend(SaturatingCarrier::I64),
                    ];
                }
            }

            use predicates::{
                auxiliary_uses, dead_scratch, droppable_tail, fault_discharged_alternative,
                implicit_defs_covered, implicit_unit_used, indexed_pointer_read_matches,
                isolated_alternative, swapped_unit_readers_equality_only,
            };

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairResultDisposition {
                ScalarRegister,
                ImplicitUnits,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct PairUnitEffects {
                pub consumer_fixed_view: PairConsumerBindingAdmission,
                pub consumer_early_clobber: PairConsumerBindingAdmission,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairConsumerBindingAdmission {
                Rejected,
                DroppedWithOperandList,
            }

            impl PairUnitEffects {
                pub(crate) const ISOLATED: Self = Self {
                    consumer_fixed_view: PairConsumerBindingAdmission::Rejected,
                    consumer_early_clobber: PairConsumerBindingAdmission::Rejected,
                };

                pub(crate) const BOUND_CONSUMER_OPERANDS: Self = Self {
                    consumer_fixed_view: PairConsumerBindingAdmission::DroppedWithOperandList,
                    ..Self::ISOLATED
                };

                pub(crate) const BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS: Self = Self {
                    consumer_early_clobber: PairConsumerBindingAdmission::DroppedWithOperandList,
                    ..Self::BOUND_CONSUMER_OPERANDS
                };

                pub(crate) fn admits_row_units(self, row: &RegisterInstructionConstraint) -> bool {
                    row.implicit_uses.is_empty() && row.clobbers.is_empty()
                }

                pub(crate) fn admits_operand(self, operand: &RegisterOperandConstraint) -> bool {
                    operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                        && !operand.early_clobber
                }

                pub(crate) fn admits_consumer(self, consumer: &SelectedInstruction) -> bool {
                    let admits_fixed_view = matches!(
                        self.consumer_fixed_view,
                        PairConsumerBindingAdmission::DroppedWithOperandList
                    );
                    let admits_early_clobber = matches!(
                        self.consumer_early_clobber,
                        PairConsumerBindingAdmission::DroppedWithOperandList
                    );
                    consumer.operands.iter().all(|operand| {
                        operand.tied_to.is_none()
                            && (admits_fixed_view || operand.fixed_view.is_none())
                            && (admits_early_clobber || !operand.early_clobber)
                    })
                }

                pub(crate) fn admits_producer(self, producer: &SelectedInstruction) -> bool {
                    producer.implicit_uses.is_empty()
                        && producer.implicit_defs.is_empty()
                        && producer.clobbers.is_empty()
                        && producer.operands.iter().all(|operand| {
                            operand.fixed_view.is_none()
                                && operand.tied_to.is_none()
                                && !operand.early_clobber
                        })
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct PairMachineEffects {
                pub non_unit: PairNonUnitSurface,
                pub fault: PairFaultDischarge,
                pub unit_defs: PairUnitDefRelation,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairNonUnitSurface {
                Isolated,
                IndexedPointerRead { index_operand: u16 },
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairFaultDischarge {
                Isolated,
                DischargedByLiteral,
                DischargedByObligation,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairUnitDefRelation {
                Covered,
                RetiredWhenDead,
                OperandSwapped,
            }

            impl PairMachineEffects {
                pub(crate) const ISOLATED: Self = Self {
                    non_unit: PairNonUnitSurface::Isolated,
                    fault: PairFaultDischarge::Isolated,
                    unit_defs: PairUnitDefRelation::Covered,
                };

                pub const fn indexed_pointer_read_fold(index_operand: u16) -> Self {
                    Self {
                        non_unit: PairNonUnitSurface::IndexedPointerRead { index_operand },
                        fault: PairFaultDischarge::Isolated,
                        unit_defs: PairUnitDefRelation::Covered,
                    }
                }

                pub(crate) const FAULT_DISCHARGED_BY_LITERAL: Self = Self {
                    fault: PairFaultDischarge::DischargedByLiteral,
                    ..Self::ISOLATED
                };

                pub(crate) const FAULT_DISCHARGED_BY_OBLIGATION: Self = Self {
                    fault: PairFaultDischarge::DischargedByObligation,
                    ..Self::ISOLATED
                };

                pub(crate) const DEAD_CONSUMER_UNIT_DEFS: Self = Self {
                    unit_defs: PairUnitDefRelation::RetiredWhenDead,
                    ..Self::ISOLATED
                };

                pub(crate) const FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS: Self = Self {
                    fault: PairFaultDischarge::DischargedByLiteral,
                    unit_defs: PairUnitDefRelation::RetiredWhenDead,
                    ..Self::ISOLATED
                };

                pub(crate) const FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS: Self = Self {
                    fault: PairFaultDischarge::DischargedByObligation,
                    unit_defs: PairUnitDefRelation::RetiredWhenDead,
                    ..Self::ISOLATED
                };

                pub(crate) const OPERAND_SWAPPED_UNIT_DEFS: Self = Self {
                    unit_defs: PairUnitDefRelation::OperandSwapped,
                    ..Self::ISOLATED
                };

                pub(crate) fn admits_producer(
                    self,
                    declaration: &MachineEffectDeclaration,
                ) -> bool {
                    isolated_declaration(declaration)
                        && declaration.alternatives.iter().all(|alternative| {
                            isolated_alternative(alternative)
                                && alternative.encoded.implicit_unit_uses.is_empty()
                                && alternative.encoded.implicit_unit_defs.is_empty()
                                && alternative.encoded.implicit_unit_clobbers.is_empty()
                        })
                }

                pub(crate) fn admits_consumer(
                    self,
                    declaration: &MachineEffectDeclaration,
                    rewritten: &MachineEffectDeclaration,
                ) -> bool {
                    match self.non_unit {
                        PairNonUnitSurface::Isolated => {
                            isolated_declaration(declaration)
                                && declaration.alternatives.iter().all(|alternative| {
                                    self.fault.consumer_alternative_gate(alternative)
                                        && alternative.encoded.implicit_unit_uses.is_empty()
                                        && self.unit_defs.defs_gate(alternative, rewritten)
                                })
                        }
                        PairNonUnitSurface::IndexedPointerRead { index_operand } => {
                            declaration.memory == MachineMemoryEffect::ReadPointerV1
                                && declaration.memory == rewritten.memory
                                && declaration.trap == rewritten.trap
                                && declaration.barrier == rewritten.barrier
                                && declaration.call == rewritten.call
                                && declaration.cleanup == rewritten.cleanup
                                && declaration.alternatives.iter().all(|alternative| {
                                    rewritten.alternatives.iter().all(|rewritten_alternative| {
                                        indexed_pointer_read_matches(
                                            alternative,
                                            rewritten_alternative,
                                            index_operand,
                                        )
                                    })
                                })
                        }
                    }
                }

                pub(crate) fn admits_consumer_obligation(
                    self,
                    consumer: &SelectedInstruction,
                ) -> bool {
                    if self.fault != PairFaultDischarge::DischargedByObligation {
                        return true;
                    }
                    let obligation = match consumer.kind {
                        SelectedInstructionKind::WrappingRemainderI64 { obligation, .. }
                        | SelectedInstructionKind::ExactDivideU64 { obligation, .. }
                        | SelectedInstructionKind::SaturatingDivide { obligation, .. } => {
                            obligation
                        }
                        _ => return false,
                    };
                    consumer.provenance.obligations.contains(&obligation)
                }

                pub(crate) fn admits_dead_consumer_defs(
                    self,
                    consumer: &SelectedInstruction,
                    function: &SelectedFunction,
                ) -> bool {
                    match self.unit_defs {
                        PairUnitDefRelation::RetiredWhenDead => {
                            consumer.implicit_uses.is_empty()
                                && consumer
                                    .implicit_defs
                                    .iter()
                                    .all(|unit| !implicit_unit_used(function, *unit))
                        }
                        PairUnitDefRelation::Covered | PairUnitDefRelation::OperandSwapped => true,
                    }
                }

                pub(crate) fn admits_swapped_condition_defs(
                    self,
                    consumer: &SelectedInstruction,
                    function: &SelectedFunction,
                    block_index: usize,
                    consumer_index: usize,
                ) -> bool {
                    match self.unit_defs {
                        PairUnitDefRelation::OperandSwapped => {
                            let [left, right, ..] = consumer.operands.as_slice() else {
                                return false;
                            };
                            consumer.implicit_uses.is_empty()
                                && (left.virtual_register == right.virtual_register
                                    || consumer.implicit_defs.iter().all(|unit| {
                                        swapped_unit_readers_equality_only(
                                            function,
                                            block_index,
                                            consumer_index,
                                            *unit,
                                        )
                                    }))
                        }
                        PairUnitDefRelation::Covered | PairUnitDefRelation::RetiredWhenDead => true,
                    }
                }

                pub(crate) fn admits_rewritten(
                    self,
                    declaration: &MachineEffectDeclaration,
                ) -> bool {
                    match self.non_unit {
                        PairNonUnitSurface::Isolated => {
                            isolated_declaration(declaration)
                                && declaration.alternatives.iter().all(|alternative| {
                                    isolated_alternative(alternative)
                                        && alternative.encoded.implicit_unit_uses.is_empty()
                                        && alternative.encoded.implicit_unit_clobbers.is_empty()
                                })
                        }
                        PairNonUnitSurface::IndexedPointerRead { .. } => {
                            declaration.memory == MachineMemoryEffect::ReadPointerV1
                                && declaration.alternatives.iter().all(|alternative| {
                                    matches!(
                                        alternative.encoded.memory,
                                        MachineEncodedMemoryEffect::ReadPointerV1 { .. }
                                    ) && alternative.encoded.stack
                                        == MachineEncodedStackEffect::UnchangedV1
                                        && alternative.encoded.control
                                            == MachineEncodedControlEffect::FallThroughV1
                                        && alternative.encoded.implicit_unit_uses.is_empty()
                                        && alternative.encoded.implicit_unit_clobbers.is_empty()
                                })
                        }
                    }
                }
            }

            impl PairFaultDischarge {
                fn consumer_alternative_gate(self, alternative: &MachineAlternative) -> bool {
                    match self {
                        Self::Isolated => isolated_alternative(alternative),
                        Self::DischargedByLiteral | Self::DischargedByObligation => {
                            fault_discharged_alternative(alternative)
                        }
                    }
                }
            }

            impl PairUnitDefRelation {
                fn defs_gate(
                    self,
                    consumer: &MachineAlternative,
                    rewritten: &MachineEffectDeclaration,
                ) -> bool {
                    match self {
                        Self::Covered | Self::OperandSwapped => {
                            implicit_defs_covered(consumer, rewritten)
                        }
                        Self::RetiredWhenDead => true,
                    }
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct PairOperandShape {
                pub position: PairLiteralPosition,
                pub result_kind: PairOperandResult,
                pub tail: PairTailCustody,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairLiteralPosition {
                Right,
                Left,
                Sole,
            }

            impl PairLiteralPosition {
                pub(crate) const fn victim_operand(self) -> u16 {
                    match self {
                        Self::Right => 1,
                        Self::Left | Self::Sole => 0,
                    }
                }

                pub(crate) const fn survivor_operand(self) -> Option<u16> {
                    match self {
                        Self::Right => Some(0),
                        Self::Left => Some(1),
                        Self::Sole => None,
                    }
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairOperandResult {
                SurvivingOperand,
                ConstantOfLiteral,
                SwappedOperand,
                LiteralRecompute,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairTailCustody {
                Bare,
                AuxiliaryUses,
                ScratchDefs,
                AuxiliaryUsesOrScratchDefs,
            }

            impl PairTailCustody {
                pub(crate) fn admits(
                    self,
                    function: &SelectedFunction,
                    operands: &[SelectedOperand],
                ) -> bool {
                    match self {
                        Self::Bare => operands.is_empty(),
                        Self::AuxiliaryUses => auxiliary_uses(function, operands),
                        Self::ScratchDefs => dead_scratch(function, operands),
                        Self::AuxiliaryUsesOrScratchDefs => droppable_tail(function, operands),
                    }
                }
            }

            impl PairOperandShape {
                pub(crate) const BINARY_RIGHT_LITERAL: Self = Self {
                    position: PairLiteralPosition::Right,
                    result_kind: PairOperandResult::SurvivingOperand,
                    tail: PairTailCustody::Bare,
                };
                pub(crate) const BINARY_LEFT_LITERAL: Self = Self {
                    position: PairLiteralPosition::Left,
                    result_kind: PairOperandResult::SurvivingOperand,
                    tail: PairTailCustody::Bare,
                };
                pub(crate) const BINARY_LEFT_LITERAL_OPERAND_SWAP: Self = Self {
                    position: PairLiteralPosition::Left,
                    result_kind: PairOperandResult::SwappedOperand,
                    tail: PairTailCustody::Bare,
                };
                pub(crate) const UNARY_LITERAL: Self = Self {
                    position: PairLiteralPosition::Sole,
                    result_kind: PairOperandResult::LiteralRecompute,
                    tail: PairTailCustody::Bare,
                };
                pub(crate) const BINARY_RIGHT_LITERAL_AUXILIARY_USES: Self = Self {
                    position: PairLiteralPosition::Right,
                    result_kind: PairOperandResult::SurvivingOperand,
                    tail: PairTailCustody::AuxiliaryUses,
                };
                pub(crate) const BINARY_RIGHT_LITERAL_CONSTANT_RESULT: Self = Self {
                    position: PairLiteralPosition::Right,
                    result_kind: PairOperandResult::ConstantOfLiteral,
                    tail: PairTailCustody::ScratchDefs,
                };
                pub(crate) const BINARY_LEFT_LITERAL_CONSTANT_RESULT: Self = Self {
                    position: PairLiteralPosition::Left,
                    result_kind: PairOperandResult::ConstantOfLiteral,
                    tail: PairTailCustody::ScratchDefs,
                };
                pub(crate) const BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES: Self = Self {
                    position: PairLiteralPosition::Left,
                    result_kind: PairOperandResult::ConstantOfLiteral,
                    tail: PairTailCustody::AuxiliaryUses,
                };
                pub(crate) const BINARY_RIGHT_LITERAL_SCRATCH_DEFS: Self = Self {
                    position: PairLiteralPosition::Right,
                    result_kind: PairOperandResult::SurvivingOperand,
                    tail: PairTailCustody::ScratchDefs,
                };
                pub(crate) const BINARY_LEFT_LITERAL_SCRATCH_DEFS: Self = Self {
                    position: PairLiteralPosition::Left,
                    result_kind: PairOperandResult::SurvivingOperand,
                    tail: PairTailCustody::ScratchDefs,
                };
                pub(crate) const BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS: Self = Self {
                    position: PairLiteralPosition::Right,
                    result_kind: PairOperandResult::SurvivingOperand,
                    tail: PairTailCustody::AuxiliaryUsesOrScratchDefs,
                };
                pub(crate) const BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS: Self =
                    Self {
                        position: PairLiteralPosition::Left,
                        result_kind: PairOperandResult::ConstantOfLiteral,
                        tail: PairTailCustody::AuxiliaryUsesOrScratchDefs,
                    };

                pub(crate) const fn victim_operand(self) -> u16 {
                    self.position.victim_operand()
                }

                pub(crate) const fn survivor_operand(self) -> Option<u16> {
                    self.position.survivor_operand()
                }

                pub(crate) const fn result_operand(
                    self,
                    result: PairResultDisposition,
                ) -> Option<u16> {
                    match (self.position, result) {
                        (_, PairResultDisposition::ImplicitUnits) => None,
                        (PairLiteralPosition::Sole, PairResultDisposition::ScalarRegister) => {
                            Some(1)
                        }
                        (
                            PairLiteralPosition::Right | PairLiteralPosition::Left,
                            PairResultDisposition::ScalarRegister,
                        ) => Some(2),
                    }
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PairImmediateBound {
                Encoding(u64),
                EncodingNonZero(u64),
                Exactly(u64),
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct SelectedInstructionPairRule {
                producer: MachineSemanticKind,
                consumer: MachineSemanticKind,
                rewritten: MachineSemanticKind,
                operand_shape: PairOperandShape,
                immediate_bound: PairImmediateBound,
                result: PairResultDisposition,
                unit_effects: PairUnitEffects,
                machine_effects: PairMachineEffects,
            }

            impl SelectedInstructionPairRule {
                receipt_accessor_rows!(SelectedInstructionPairRule {
                    producer: MachineSemanticKind,
                    consumer: MachineSemanticKind,
                    rewritten: MachineSemanticKind,
                    operand_shape: PairOperandShape,
                    result: PairResultDisposition,
                    unit_effects: PairUnitEffects,
                    machine_effects: PairMachineEffects,
                });

                pub const fn victim_operand(self) -> u16 {
                    self.operand_shape.victim_operand()
                }

                pub(crate) const fn rewritten_row_roles(self) -> Option<(bool, bool)> {
                    match (self.operand_shape.result_kind, self.result) {
                        (
                            PairOperandResult::SurvivingOperand,
                            PairResultDisposition::ScalarRegister,
                        ) => Some((true, true)),
                        (
                            PairOperandResult::SurvivingOperand,
                            PairResultDisposition::ImplicitUnits,
                        ) => Some((true, false)),
                        (
                            PairOperandResult::SwappedOperand,
                            PairResultDisposition::ImplicitUnits,
                        ) => Some((true, false)),
                        (
                            PairOperandResult::ConstantOfLiteral
                            | PairOperandResult::LiteralRecompute,
                            PairResultDisposition::ScalarRegister,
                        ) => Some((false, true)),
                        _ => None,
                    }
                }
                pub const fn admits_immediate(self, value: u64) -> bool {
                    match self.immediate_bound {
                        PairImmediateBound::Encoding(limit) => value <= limit,
                        PairImmediateBound::EncodingNonZero(limit) => value != 0 && value <= limit,
                        PairImmediateBound::Exactly(exact) => value == exact,
                    }
                }

                pub(crate) fn fold_immediate(self, literal: u64) -> Option<u64> {
                    Some(match self.operand_shape.result_kind {
                        PairOperandResult::SurvivingOperand | PairOperandResult::SwappedOperand => {
                            literal
                        }
                        PairOperandResult::ConstantOfLiteral => match self.consumer {
                            MachineSemanticKind::SaturatingAdd(carrier) => carrier.maximum_bits(),
                            _ => 0,
                        },
                        PairOperandResult::LiteralRecompute => match self.consumer {
                            MachineSemanticKind::CopyI64 => literal,
                            MachineSemanticKind::ZeroExtendU8 => literal & 0xFF,
                            MachineSemanticKind::ZeroExtendU16 => literal & 0xFFFF,
                            MachineSemanticKind::ZeroExtendU32 => literal & 0xFFFF_FFFF,
                            MachineSemanticKind::SignExtendI8 => {
                                (literal & 0xFF) as u8 as i8 as i64 as u64
                            }
                            MachineSemanticKind::SignExtendI16 => {
                                (literal & 0xFFFF) as u16 as i16 as i64 as u64
                            }
                            MachineSemanticKind::SignExtendI32 => {
                                (literal & 0xFFFF_FFFF) as u32 as i32 as i64 as u64
                            }
                            _ => return None,
                        },
                    })
                }

                pub(crate) fn matches_producer(self, kind: SelectedInstructionKind) -> bool {
                    machine_semantic_kind(kind) == self.producer
                }

                pub(crate) fn matches_consumer(self, kind: SelectedInstructionKind) -> bool {
                    machine_semantic_kind(kind) == self.consumer
                }

                pub(crate) fn immediate_constraint_key(
                    self,
                    keys: &TargetRegisterEnvironmentConstraintKeys,
                ) -> Option<RegisterConstraintKey> {
                    match self.rewritten {
                        MachineSemanticKind::ExactAddI64Immediate => Some(keys.add_i64_immediate),
                        MachineSemanticKind::ExactSubtractI64Immediate => {
                            Some(keys.subtract_i64_immediate)
                        }
                        MachineSemanticKind::CompareI64Immediate => {
                            Some(keys.compare_i64_immediate)
                        }
                        MachineSemanticKind::CompareI64Zero => Some(keys.compare_i64_zero),
                        MachineSemanticKind::MaterializeI64 => Some(keys.materialize_i64),
                        MachineSemanticKind::Load8 => keys.load8,
                        MachineSemanticKind::AddressOffset => keys.address_offset,
                        MachineSemanticKind::CopyI64 => Some(keys.copy_i64),
                        _ => None,
                    }
                }

                pub(crate) fn rewrite_consumer(
                    self,
                    kind: SelectedInstructionKind,
                    immediate: u64,
                    result_scalar: Option<ScalarType>,
                ) -> Option<SelectedInstructionKind> {
                    let literal = IntegerValue::Unsigned(u128::from(immediate));
                    match (self.rewritten, kind) {
                        (
                            MachineSemanticKind::ExactAddI64Immediate,
                            SelectedInstructionKind::ExactAddI64 {
                                obligation,
                                accepted_fact,
                            },
                        ) => Some(SelectedInstructionKind::ExactAddI64Immediate {
                            immediate: literal,
                            obligation,
                            accepted_fact,
                        }),
                        (
                            MachineSemanticKind::ExactSubtractI64Immediate,
                            SelectedInstructionKind::ExactSubtractI64 {
                                obligation,
                                accepted_fact,
                            },
                        ) => Some(SelectedInstructionKind::ExactSubtractI64Immediate {
                            immediate: literal,
                            obligation,
                            accepted_fact,
                        }),
                        (
                            MachineSemanticKind::CompareI64Immediate,
                            SelectedInstructionKind::CompareI64,
                        ) => Some(SelectedInstructionKind::CompareI64Immediate {
                            immediate: literal,
                        }),
                        (
                            MachineSemanticKind::CompareI64Zero,
                            SelectedInstructionKind::CompareI64,
                        ) => Some(SelectedInstructionKind::CompareI64Zero),
                        (
                            MachineSemanticKind::MaterializeI64,
                            kind @ (SelectedInstructionKind::ZeroExtendU8
                            | SelectedInstructionKind::ZeroExtendU16
                            | SelectedInstructionKind::ZeroExtendU32
                            | SelectedInstructionKind::SignExtendI8
                            | SelectedInstructionKind::SignExtendI16
                            | SelectedInstructionKind::SignExtendI32
                            | SelectedInstructionKind::CopyI64),
                        ) if machine_semantic_kind(kind) == self.consumer => {
                            scalar_materialize_value(immediate, result_scalar?)
                                .map(|value| SelectedInstructionKind::MaterializeI64 { value })
                        }
                        (MachineSemanticKind::Load8, SelectedInstructionKind::Load8Indexed) => {
                            u32::try_from(immediate)
                                .ok()
                                .map(|byte_offset| SelectedInstructionKind::Load8 { byte_offset })
                        }
                        (
                            MachineSemanticKind::AddressOffset,
                            SelectedInstructionKind::ByteViewAddress,
                        ) => u32::try_from(immediate).ok().map(|byte_offset| {
                            SelectedInstructionKind::AddressOffset { byte_offset }
                        }),
                        (
                            MachineSemanticKind::CopyI64,
                            SelectedInstructionKind::ExactDivideU64 { .. },
                        ) => Some(SelectedInstructionKind::CopyI64),
                        (
                            MachineSemanticKind::CopyI64,
                            kind @ (SelectedInstructionKind::BitwiseXorI64
                            | SelectedInstructionKind::WrappingAddI64
                            | SelectedInstructionKind::BitwiseAndI64
                            | selected_instructions::saturating_add_or_subtract_pattern!()
                            | SelectedInstructionKind::SaturatingDivide { .. }),
                        ) if machine_semantic_kind(kind) == self.consumer => {
                            Some(SelectedInstructionKind::CopyI64)
                        }
                        (
                            MachineSemanticKind::MaterializeI64,
                            kind @ (SelectedInstructionKind::WrappingRemainderI64 { .. }
                            | SelectedInstructionKind::ExactDivideU64 { .. }
                            | SelectedInstructionKind::SaturatingDivide { .. }
                            | SelectedInstructionKind::SaturatingSubtract { .. }
                            | SelectedInstructionKind::SaturatingAdd { .. }
                            | SelectedInstructionKind::BitwiseAndI64),
                        ) if machine_semantic_kind(kind) == self.consumer => {
                            scalar_materialize_value(immediate, result_scalar?)
                                .map(|value| SelectedInstructionKind::MaterializeI64 { value })
                        }
                        _ => None,
                    }
                }
            }

            fn scalar_materialize_value(bits: u64, scalar: ScalarType) -> Option<IntegerValue> {
                let integer = scalar.as_integer()?;
                if integer.is_address() || integer.bits() > 64 {
                    return None;
                }
                let value = match integer.sign() {
                    IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(bits)),
                    IntegerSign::Signed => IntegerValue::Signed(i128::from(bits as i64)),
                };
                integer.admits(value).then_some(value)
            }
        }
        pub(crate) mod receipt {
            use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};

            use crate::rewrites::literal_fold_identity;
            use crate::{LiteralFoldPlan, LiteralFoldValidationReceipt, ValidatedLiteralFold};

            pub(crate) fn admit_literal_fold(
                plan: LiteralFoldPlan,
                transformed: SelectedInstructionPlan,
                transformed_selected: SelectedInstructionPlanIdentity,
                applied_count: usize,
            ) -> ValidatedLiteralFold {
                let receipt = LiteralFoldValidationReceipt {
                    identity: literal_fold_identity(&plan),
                    source_selected: plan.source_selected,
                    spill_choices: plan.spill_choices,
                    recovery_classifications: plan.recovery_classifications,
                    ranges: plan.ranges,
                    legality: plan.legality,
                    register_environment: plan.register_environment,
                    allocator_availability: plan.allocator_availability,
                    machine_effect_catalog: plan.machine_effect_catalog,
                    optimization_unit: plan.optimization_unit,
                    work_schedule: plan.work_schedule,
                    transformed_selected,
                    policy: plan.policy,
                    usage: plan.usage,
                    function_count: transformed.functions.len(),
                    applied_count,
                };
                ValidatedLiteralFold {
                    plan,
                    transformed: transformed.into(),
                    receipt,
                }
            }
        }

        use identity::encode_terminal_literal_fold_content;
        pub(crate) use identity::literal_fold_identity;
        use optimization_core::{
            OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
        };
        pub(crate) use pair_rule::{
            PairResultDisposition, PairUnitDefRelation, SelectedInstructionPairRule,
        };
        use register_homes::{
            AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
            RecoveryClassificationIdentity, SpillChoiceIdentity,
        };
        use register_model::RegisterConstraintKey;
        pub use selected_instructions::LiteralFoldIdentity;
        use selected_instructions::{
            LiveRangeIdentity, MachineEffectCatalogIdentity, SelectedBlockId,
            SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
            VirtualRegisterId,
        };
        use semantic_vocabulary::{MachineId, WorkScheduleIdentity};

        pub(crate) fn fold_selected_incoming_literal<S: ValidatedSelectedAnalysis>(
            selected: &S,
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
            spill_choices: &ValidatedSpillChoices,
            recovery: &ValidatedRecoveryClassifications,
            availability: &ValidatedAllocatorAvailability,
            register_environment: TargetRegisterEnvironmentIdentity,
            constraints: &ValidatedRegisterConstraintCatalog,
            selected_keys: &TargetRegisterEnvironmentConstraintKeys,
            effect_catalog: &selected_instructions::PreparedMachineEffectCatalog,
            policy: LiteralFoldPolicy,
            budget: optimization_core::OptimizationWorkBudget,
        ) -> Result<ValidatedLiteralFold, LiteralFoldError> {
            let (plan, transformed) = compute::compute_terminal_literal_fold(
                selected,
                ranges,
                legality,
                spill_choices,
                recovery,
                availability,
                register_environment,
                constraints,
                selected_keys,
                effect_catalog,
                policy,
                budget,
            )?;
            let applied_count = plan
                .functions
                .iter()
                .map(|function| function.actions.len())
                .sum();
            let transformed_selected = plan.transformed_selected;
            Ok(receipt::admit_literal_fold(
                plan,
                transformed,
                transformed_selected,
                applied_count,
            ))
        }

        const LITERAL_FOLD_MAGIC: &[u8; 8] = b"OMGLFD\0\0";
        const LITERAL_FOLD_VERSION: u32 = 13;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct LiteralFoldPolicy {
            enabled_rules: u32,
        }

        impl LiteralFoldPolicy {
            const EXACT_ADD_BIT: u32 = 1 << 0;
            const EXACT_SUBTRACT_BIT: u32 = 1 << 1;
            const COMPARE_BIT: u32 = 1 << 2;
            const EXTENSION_BIT: u32 = 1 << 3;
            const LOAD8_INDEXED_BIT: u32 = 1 << 4;
            const COPY_BIT: u32 = 1 << 5;
            const BYTE_VIEW_ADDRESS_BIT: u32 = 1 << 6;
            const EXACT_DIVIDE_BIT: u32 = 1 << 7;
            const WRAPPING_REMAINDER_BIT: u32 = 1 << 8;
            const BITWISE_AND_ZERO_BIT: u32 = 1 << 9;
            const BITWISE_XOR_ZERO_BIT: u32 = 1 << 10;
            const WRAPPING_ADD_ZERO_BIT: u32 = 1 << 11;
            const BITWISE_AND_ONES_BIT: u32 = 1 << 12;
            const WRAPPING_REMAINDER_ZERO_BIT: u32 = 1 << 13;
            const EXACT_DIVIDE_ZERO_BIT: u32 = 1 << 14;
            const SATURATING_ADD_ZERO_BIT: u32 = 1 << 15;
            const SATURATING_SUBTRACT_ZERO_BIT: u32 = 1 << 16;
            const SATURATING_DIVIDE_ONE_BIT: u32 = 1 << 17;
            const SATURATING_DIVIDE_ZERO_BIT: u32 = 1 << 18;
            const SATURATING_SUBTRACT_ZERO_MINUEND_BIT: u32 = 1 << 19;
            const SATURATING_ADD_UPPER_BOUND_BIT: u32 = 1 << 20;
            const WRAPPING_REMAINDER_MINUS_ONE_BIT: u32 = 1 << 21;
            const SATURATING_SUBTRACT_UPPER_BOUND_BIT: u32 = 1 << 22;
            const KNOWN_BITS: u32 = Self::EXACT_ADD_BIT
                | Self::EXACT_SUBTRACT_BIT
                | Self::COMPARE_BIT
                | Self::EXTENSION_BIT
                | Self::LOAD8_INDEXED_BIT
                | Self::COPY_BIT
                | Self::BYTE_VIEW_ADDRESS_BIT
                | Self::EXACT_DIVIDE_BIT
                | Self::WRAPPING_REMAINDER_BIT
                | Self::BITWISE_AND_ZERO_BIT
                | Self::BITWISE_XOR_ZERO_BIT
                | Self::WRAPPING_ADD_ZERO_BIT
                | Self::BITWISE_AND_ONES_BIT
                | Self::WRAPPING_REMAINDER_ZERO_BIT
                | Self::EXACT_DIVIDE_ZERO_BIT
                | Self::SATURATING_ADD_ZERO_BIT
                | Self::SATURATING_SUBTRACT_ZERO_BIT
                | Self::SATURATING_DIVIDE_ONE_BIT
                | Self::SATURATING_DIVIDE_ZERO_BIT
                | Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT
                | Self::SATURATING_ADD_UPPER_BOUND_BIT
                | Self::WRAPPING_REMAINDER_MINUS_ONE_BIT
                | Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT;

            pub(crate) const EXACT_ADD_V1: Self = Self {
                enabled_rules: Self::EXACT_ADD_BIT,
            };
            pub(crate) const EXACT_SUBTRACT_V1: Self = Self {
                enabled_rules: Self::EXACT_SUBTRACT_BIT,
            };
            pub(crate) const COMPARE_V1: Self = Self {
                enabled_rules: Self::COMPARE_BIT,
            };
            pub(crate) const EXTENSION_V1: Self = Self {
                enabled_rules: Self::EXTENSION_BIT,
            };
            pub(crate) const LOAD8_INDEXED_V1: Self = Self {
                enabled_rules: Self::LOAD8_INDEXED_BIT,
            };
            pub(crate) const COPY_V1: Self = Self {
                enabled_rules: Self::COPY_BIT,
            };
            pub(crate) const BYTE_VIEW_ADDRESS_V1: Self = Self {
                enabled_rules: Self::BYTE_VIEW_ADDRESS_BIT,
            };
            pub(crate) const EXACT_DIVIDE_V1: Self = Self {
                enabled_rules: Self::EXACT_DIVIDE_BIT,
            };
            pub(crate) const WRAPPING_REMAINDER_V1: Self = Self {
                enabled_rules: Self::WRAPPING_REMAINDER_BIT,
            };
            pub(crate) const BITWISE_AND_ZERO_V1: Self = Self {
                enabled_rules: Self::BITWISE_AND_ZERO_BIT,
            };
            pub(crate) const BITWISE_XOR_ZERO_V1: Self = Self {
                enabled_rules: Self::BITWISE_XOR_ZERO_BIT,
            };
            pub(crate) const WRAPPING_ADD_ZERO_V1: Self = Self {
                enabled_rules: Self::WRAPPING_ADD_ZERO_BIT,
            };
            pub(crate) const BITWISE_AND_ONES_V1: Self = Self {
                enabled_rules: Self::BITWISE_AND_ONES_BIT,
            };
            pub(crate) const WRAPPING_REMAINDER_ZERO_V1: Self = Self {
                enabled_rules: Self::WRAPPING_REMAINDER_ZERO_BIT,
            };
            pub(crate) const EXACT_DIVIDE_ZERO_V1: Self = Self {
                enabled_rules: Self::EXACT_DIVIDE_ZERO_BIT,
            };
            pub(crate) const SATURATING_ADD_ZERO_V1: Self = Self {
                enabled_rules: Self::SATURATING_ADD_ZERO_BIT,
            };
            pub(crate) const SATURATING_SUBTRACT_ZERO_V1: Self = Self {
                enabled_rules: Self::SATURATING_SUBTRACT_ZERO_BIT,
            };
            pub(crate) const SATURATING_DIVIDE_ONE_V1: Self = Self {
                enabled_rules: Self::SATURATING_DIVIDE_ONE_BIT,
            };
            pub(crate) const SATURATING_DIVIDE_ZERO_V1: Self = Self {
                enabled_rules: Self::SATURATING_DIVIDE_ZERO_BIT,
            };
            pub(crate) const SATURATING_SUBTRACT_ZERO_MINUEND_V1: Self = Self {
                enabled_rules: Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT,
            };
            pub(crate) const SATURATING_ADD_UPPER_BOUND_V1: Self = Self {
                enabled_rules: Self::SATURATING_ADD_UPPER_BOUND_BIT,
            };
            pub(crate) const WRAPPING_REMAINDER_MINUS_ONE_V1: Self = Self {
                enabled_rules: Self::WRAPPING_REMAINDER_MINUS_ONE_BIT,
            };
            pub(crate) const SATURATING_SUBTRACT_UPPER_BOUND_V1: Self = Self {
                enabled_rules: Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT,
            };

            pub(crate) const fn empty() -> Self {
                Self { enabled_rules: 0 }
            }

            pub const fn contains(self, other: Self) -> bool {
                other.enabled_rules != 0
                    && self.enabled_rules & other.enabled_rules == other.enabled_rules
            }

            pub const fn union(self, other: Self) -> Self {
                Self {
                    enabled_rules: self.enabled_rules | other.enabled_rules,
                }
            }

            pub const fn canonical_bits(self) -> u32 {
                self.enabled_rules
            }

            pub const fn from_canonical_bits(bits: u32) -> Option<Self> {
                if bits == 0 || bits & !Self::KNOWN_BITS != 0 {
                    None
                } else {
                    Some(Self {
                        enabled_rules: bits,
                    })
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct LiteralFoldPlan {
            pub source_selected: SelectedInstructionPlanIdentity,
            pub spill_choices: SpillChoiceIdentity,
            pub recovery_classifications: RecoveryClassificationIdentity,
            pub ranges: LiveRangeIdentity,
            pub legality: AllocationLegalityIdentity,
            pub register_environment: TargetRegisterEnvironmentIdentity,
            pub allocator_availability: AllocatorAvailabilityIdentity,
            pub machine_effect_catalog: MachineEffectCatalogIdentity,
            pub optimization_unit: OptimizationUnitIdentity,
            pub work_schedule: WorkScheduleIdentity,
            pub policy: LiteralFoldPolicy,
            pub budget: OptimizationWorkBudget,
            pub usage: OptimizationWorkUsage,
            pub functions: Vec<FunctionLiteralFold>,
            pub transformed_selected: SelectedInstructionPlanIdentity,
        }

        impl LiteralFoldPlan {
            pub fn encode(&self) -> Vec<u8> {
                let content = encode_terminal_literal_fold_content(self);
                let identity = literal_fold_identity(self);
                let mut encoded = Vec::with_capacity(44 + content.len());
                crate::codec_bytes::encode_header(
                    &mut encoded,
                    LITERAL_FOLD_MAGIC,
                    LITERAL_FOLD_VERSION,
                );
                encoded.extend_from_slice(&identity.bytes());
                encoded.extend_from_slice(&content);
                encoded
            }

            pub fn decode(encoded: &[u8]) -> Result<Self, LiteralFoldDecodeError> {
                let mut cursor = Cursor::new(encoded);
                crate::codec_bytes::decode_header(
                    &mut cursor,
                    LITERAL_FOLD_MAGIC,
                    LITERAL_FOLD_VERSION,
                    || LiteralFoldDecodeError::WrongMagic,
                    LiteralFoldDecodeError::UnsupportedVersion,
                )?;
                let identity = LiteralFoldIdentity::from_bytes(cursor.array()?);
                let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
                let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
                let recovery_classifications =
                    RecoveryClassificationIdentity::from_bytes(cursor.array()?);
                let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
                let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
                let register_environment =
                    TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
                let allocator_availability =
                    AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
                let machine_effect_catalog =
                    MachineEffectCatalogIdentity::from_bytes(cursor.array()?);
                let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
                let raw_work = cursor.u32()?;
                let work_schedule = WorkScheduleIdentity::new(raw_work)
                    .ok_or(LiteralFoldDecodeError::InvalidWorkSchedule(raw_work))?;
                let policy_bits = cursor.u32()?;
                let policy = LiteralFoldPolicy::from_canonical_bits(policy_bits)
                    .ok_or(LiteralFoldDecodeError::UnknownPolicy(policy_bits))?;
                let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
                    .map_err(|_| LiteralFoldDecodeError::InvalidBudget)?;
                let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
                    .map_err(|_| LiteralFoldDecodeError::InvalidUsage)?;
                let function_count = cursor.length()?;
                let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
                for _ in 0..function_count {
                    let raw_machine = cursor.u64()?;
                    let machine = MachineId::new(raw_machine)
                        .ok_or(LiteralFoldDecodeError::InvalidMachineId(raw_machine))?;
                    let action_count = cursor.length()?;
                    let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
                    for _ in 0..action_count {
                        actions.push(LiteralFoldAction {
                            block: SelectedBlockId(cursor.u32()?),
                            literal_instruction: SelectedInstructionId(cursor.u32()?),
                            victim: VirtualRegisterId(cursor.u32()?),
                            consumer_instruction: SelectedInstructionId(u32::from_le_bytes(
                                cursor.array()?,
                            )),
                            surviving: VirtualRegisterId(cursor.u32()?),
                            result: match cursor.byte()? {
                                0 => None,
                                1 => Some(VirtualRegisterId(cursor.u32()?)),
                                tag => return Err(LiteralFoldDecodeError::UnknownOption(tag)),
                            },
                            immediate: cursor.u64()?,
                            immediate_constraint: decode_constraint_key(&mut cursor)?,
                        });
                    }
                    functions.push(FunctionLiteralFold { machine, actions });
                }
                let transformed_selected =
                    SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
                if cursor.remaining() != 0 {
                    return Err(LiteralFoldDecodeError::TrailingBytes);
                }
                let plan = Self {
                    source_selected,
                    spill_choices,
                    recovery_classifications,
                    ranges,
                    legality,
                    register_environment,
                    allocator_availability,
                    machine_effect_catalog,
                    optimization_unit,
                    work_schedule,
                    policy,
                    budget,
                    usage,
                    functions,
                    transformed_selected,
                };
                if literal_fold_identity(&plan) != identity {
                    return Err(LiteralFoldDecodeError::IdentityMismatch);
                }
                Ok(plan)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct FunctionLiteralFold {
            pub machine: MachineId,
            pub actions: Vec<LiteralFoldAction>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct LiteralFoldAction {
            pub block: SelectedBlockId,
            pub literal_instruction: SelectedInstructionId,
            pub victim: VirtualRegisterId,
            pub consumer_instruction: SelectedInstructionId,
            pub surviving: VirtualRegisterId,
            pub result: Option<VirtualRegisterId>,
            pub immediate: u64,
            pub immediate_constraint: RegisterConstraintKey,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct LiteralFoldValidationReceipt {
            pub(crate) identity: LiteralFoldIdentity,
            pub(crate) source_selected: SelectedInstructionPlanIdentity,
            pub(crate) spill_choices: SpillChoiceIdentity,
            pub(crate) recovery_classifications: RecoveryClassificationIdentity,
            pub(crate) ranges: LiveRangeIdentity,
            pub(crate) legality: AllocationLegalityIdentity,
            pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
            pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
            pub(crate) machine_effect_catalog: MachineEffectCatalogIdentity,
            pub(crate) optimization_unit: OptimizationUnitIdentity,
            pub(crate) work_schedule: WorkScheduleIdentity,
            pub(crate) transformed_selected: SelectedInstructionPlanIdentity,
            pub(crate) policy: LiteralFoldPolicy,
            pub(crate) usage: OptimizationWorkUsage,
            pub(crate) function_count: usize,
            pub(crate) applied_count: usize,
        }

        impl LiteralFoldValidationReceipt {
            receipt_accessor_rows!(LiteralFoldValidationReceipt {
                identity: LiteralFoldIdentity,
                spill_choices: SpillChoiceIdentity,
                recovery_classifications: RecoveryClassificationIdentity,
                machine_effect_catalog: MachineEffectCatalogIdentity,
                policy: LiteralFoldPolicy,
                usage: OptimizationWorkUsage,
                function_count: usize,
                applied_count: usize,
                source_selected: SelectedInstructionPlanIdentity,
                transformed_selected: SelectedInstructionPlanIdentity,
            });
            shared_analysis_identity_accessors!(pub);
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ValidatedLiteralFold {
            pub(crate) plan: LiteralFoldPlan,
            pub(crate) transformed: std::sync::Arc<SelectedInstructionPlan>,
            pub(crate) receipt: LiteralFoldValidationReceipt,
        }

        impl ValidatedLiteralFold {
            pub const fn plan(&self) -> &LiteralFoldPlan {
                &self.plan
            }
            pub(crate) fn shared_transformed(&self) -> std::sync::Arc<SelectedInstructionPlan> {
                std::sync::Arc::clone(&self.transformed)
            }

            pub fn transformed(&self) -> &SelectedInstructionPlan {
                &self.transformed
            }
            receipt_accessor_rows!(ref ValidatedLiteralFold {
                receipt: LiteralFoldValidationReceipt,
            });
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum LiteralFoldError {
            RootMismatch,
            WorkOverflow,
            BudgetExceeded {
                required: OptimizationWorkUsage,
                budget: OptimizationWorkBudget,
            },
            FunctionMismatch {
                function: usize,
            },
            ConsumerMismatch {
                function: usize,
            },
            EffectCatalogMismatch,
            ImmediateConstraintMismatch,
            IdentifierUnderflow {
                function: usize,
            },
            DecisionMismatch {
                function: usize,
            },
        }

        failed_display!(LiteralFoldError, "terminal literal fold failed");

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum LiteralFoldDecodeError {
            Truncated,
            WrongMagic,
            UnsupportedVersion(u32),
            UnknownPolicy(u32),
            UnknownOption(u8),
            UnknownConstraintFamily(u8),
            InvalidWorkSchedule(u32),
            InvalidMachineId(u64),
            InvalidBudget,
            InvalidUsage,
            LengthOverflow,
            TrailingBytes,
            IdentityMismatch,
        }

        failed_display!(LiteralFoldDecodeError, "invalid terminal literal fold");

        fn decode_constraint_key(
            cursor: &mut Cursor<'_>,
        ) -> Result<RegisterConstraintKey, LiteralFoldDecodeError> {
            let family = match cursor.byte()? {
                0 => register_model::RegisterConstraintFamily::Call,
                1 => register_model::RegisterConstraintFamily::Return,
                2 => register_model::RegisterConstraintFamily::SystemCall,
                3 => register_model::RegisterConstraintFamily::InlineAssembly,
                4 => register_model::RegisterConstraintFamily::Instruction,
                tag => return Err(LiteralFoldDecodeError::UnknownConstraintFamily(tag)),
            };
            Ok(RegisterConstraintKey {
                family,
                variant: cursor.u32()?,
            })
        }

        impl From<Truncated> for LiteralFoldDecodeError {
            fn from(_: Truncated) -> Self {
                Self::Truncated
            }
        }

        impl From<TakeError> for LiteralFoldDecodeError {
            fn from(error: TakeError) -> Self {
                match error {
                    TakeError::Truncated => Self::Truncated,
                    TakeError::LengthOverflow => Self::LengthOverflow,
                }
            }
        }
    }

    use optimization_core::{
        OptimizationExecutionPhase, OptimizationPhaseSelections, OptimizationSelections,
    };

    pub(crate) use catalog::{
        SELECTED_LOWERING_RULE_CATALOG, SelectedLoweringRuleCatalogEntry,
        SelectedLoweringRuleCatalogError, enabled_pair_rules,
    };
    pub(crate) use literal_fold::{
        FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan,
        LiteralFoldPolicy, LiteralFoldValidationReceipt, PairResultDisposition,
        PairUnitDefRelation, SelectedInstructionPairRule,
    };
    pub use literal_fold::{LiteralFoldIdentity, ValidatedLiteralFold};
    pub(crate) use literal_fold::{fold_selected_incoming_literal, literal_fold_identity};

    pub(crate) fn resolve_selected_lowering_rules(
        selections: &OptimizationPhaseSelections,
    ) -> Result<(OptimizationSelections, LiteralFoldPolicy), SelectedLoweringRuleCatalogError> {
        let phase = super::required_stage_phase(
            selections,
            OptimizationExecutionPhase::SelectedLowering,
            SelectedLoweringRuleCatalogError::WrongPhase,
            || SelectedLoweringRuleCatalogError::MissingSelection,
            SelectedLoweringRuleCatalogError::UnsupportedSelection,
        )?;
        let policy = SELECTED_LOWERING_RULE_CATALOG
            .iter()
            .filter(|entry| phase.contains(entry.optimization()))
            .fold(LiteralFoldPolicy::empty(), |policy, entry| {
                policy.union(entry.payload().policy())
            });
        Ok((phase, policy))
    }
}
