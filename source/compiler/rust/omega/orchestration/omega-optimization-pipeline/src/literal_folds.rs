use omega_optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    SelectedLoweringOptimizationCompletionIdentity,
};
use omega_regalloc::{
    TerminalAllocationLegalityError, TerminalLiteralFoldError, TerminalLiteralFoldIdentity,
    TerminalLiteralFoldPolicy, TerminalLiveRangeError, TerminalLivenessError,
    TerminalRecoveryClassificationError, TerminalRecoveryClassificationPolicy,
    TerminalSpillChoiceError, TerminalSpillChoicePolicy, ValidatedTerminalAllocationLegality,
    ValidatedTerminalLiteralFold, ValidatedTerminalLiveRanges, ValidatedTerminalLiveness,
    ValidatedTerminalRecoveryClassifications, ValidatedTerminalSelectedAnalysis,
    ValidatedTerminalSpillChoices, analyze_terminal_allocation_legality,
    analyze_terminal_live_ranges, analyze_terminal_liveness, choose_terminal_spill_victims,
    classify_terminal_pressure_recovery, fold_terminal_selected_incoming_literal,
};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt,
    validate_optimized_allocation_legality_custody,
};

/// One explicitly requested pressure decision, semantic classification,
/// literal fold, and complete analysis reconstruction. No source analysis fact
/// crosses the transformed selected-CFG boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedLiteralFoldStep {
    choices: ValidatedTerminalSpillChoices,
    recovery: ValidatedTerminalRecoveryClassifications,
    fold: ValidatedTerminalLiteralFold,
    liveness: ValidatedTerminalLiveness,
    ranges: ValidatedTerminalLiveRanges,
    legality: ValidatedTerminalAllocationLegality,
}

impl StagedOptimizedLiteralFoldStep {
    pub const fn choices(&self) -> &ValidatedTerminalSpillChoices {
        &self.choices
    }
    pub const fn recovery(&self) -> &ValidatedTerminalRecoveryClassifications {
        &self.recovery
    }
    pub const fn fold(&self) -> &ValidatedTerminalLiteralFold {
        &self.fold
    }
    pub const fn liveness(&self) -> &ValidatedTerminalLiveness {
        &self.liveness
    }
    pub const fn ranges(&self) -> &ValidatedTerminalLiveRanges {
        &self.ranges
    }
    pub const fn legality(&self) -> &ValidatedTerminalAllocationLegality {
        &self.legality
    }
}

/// One independently validated selected-lowering attempt before deciding
/// whether another transformed-CFG analysis cycle is required. A terminal
/// attempt has `applied_count() == 0` and is positive fixed-point evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedLiteralFoldAttempt {
    choices: ValidatedTerminalSpillChoices,
    recovery: ValidatedTerminalRecoveryClassifications,
    fold: ValidatedTerminalLiteralFold,
}

impl StagedOptimizedLiteralFoldAttempt {
    pub const fn choices(&self) -> &ValidatedTerminalSpillChoices {
        &self.choices
    }
    pub const fn recovery(&self) -> &ValidatedTerminalRecoveryClassifications {
        &self.recovery
    }
    pub const fn fold(&self) -> &ValidatedTerminalLiteralFold {
        &self.fold
    }
}

/// Ordered custody for separately invoked literal folds. Extending this value
/// requires another explicit API call; construction never iterates to a fixed
/// point and ordinary optimized staging never calls it implicitly.
#[derive(Debug)]
pub struct StagedOptimizedLiteralFolds {
    source: StagedOptimizedAllocationLegality,
    steps: Vec<StagedOptimizedLiteralFoldStep>,
    custody: StagedOptimizedLiteralFoldCustodyReceipt,
}

impl StagedOptimizedLiteralFolds {
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.source
    }
    pub fn steps(&self) -> &[StagedOptimizedLiteralFoldStep] {
        &self.steps
    }
    pub fn final_step(&self) -> &StagedOptimizedLiteralFoldStep {
        self.steps
            .last()
            .expect("validated literal-fold sequence is nonempty")
    }
    pub const fn custody(&self) -> &StagedOptimizedLiteralFoldCustodyReceipt {
        &self.custody
    }
}

/// Completed execution of the selected-lowering projection of one exact
/// source-visible suite. Applied steps are followed by one validated no-change
/// attempt, so an empty `steps` vector is still an evidenced successful run.
#[derive(Debug)]
pub struct StagedSelectedLoweringOptimizationRun {
    source: StagedOptimizedAllocationLegality,
    selections: OptimizationSelections,
    selected_lowering_selections: OptimizationSelections,
    steps: Vec<StagedOptimizedLiteralFoldStep>,
    terminal_attempt: StagedOptimizedLiteralFoldAttempt,
    custody: StagedSelectedLoweringOptimizationCustodyReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedLoweringOptimizationSchedule {
    SelectedIncomingU12ExactAddImmediateToNoChangeV1,
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
    pub const fn terminal_attempt(&self) -> &StagedOptimizedLiteralFoldAttempt {
        &self.terminal_attempt
    }
    pub const fn custody(&self) -> &StagedSelectedLoweringOptimizationCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSelectedLoweringOptimizationCustodyReceipt {
    identity: SelectedLoweringOptimizationCompletionIdentity,
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    selections: OptimizationSelectionIdentity,
    selected_lowering_selections: OptimizationSelectionIdentity,
    schedule: SelectedLoweringOptimizationSchedule,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    iteration_bound: usize,
    action_count: usize,
    initial_virtual_register_count: usize,
    iterations: Vec<StagedOptimizedLiteralFoldIterationReceipt>,
    terminal_attempt: StagedOptimizedLiteralFoldAttemptReceipt,
    final_selected: TerminalSelectedInstructionPlanIdentity,
    final_liveness: omega_regalloc::TerminalLivenessIdentity,
    final_ranges: omega_regalloc::TerminalLiveRangeIdentity,
    final_legality: omega_regalloc::TerminalAllocationLegalityIdentity,
    final_virtual_register_count: usize,
}

impl StagedSelectedLoweringOptimizationCustodyReceipt {
    pub const fn identity(&self) -> SelectedLoweringOptimizationCompletionIdentity {
        self.identity
    }
    pub const fn source(&self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.source
    }
    pub const fn selections(&self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn selected_lowering_selections(&self) -> OptimizationSelectionIdentity {
        self.selected_lowering_selections
    }
    pub const fn schedule(&self) -> SelectedLoweringOptimizationSchedule {
        self.schedule
    }
    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }
    pub const fn usage(&self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn iteration_bound(&self) -> usize {
        self.iteration_bound
    }
    pub const fn action_count(&self) -> usize {
        self.action_count
    }
    pub const fn initial_virtual_register_count(&self) -> usize {
        self.initial_virtual_register_count
    }
    pub fn iterations(&self) -> &[StagedOptimizedLiteralFoldIterationReceipt] {
        &self.iterations
    }
    pub const fn terminal_attempt(&self) -> StagedOptimizedLiteralFoldAttemptReceipt {
        self.terminal_attempt
    }
    pub const fn final_selected(&self) -> TerminalSelectedInstructionPlanIdentity {
        self.final_selected
    }
    pub const fn final_liveness(&self) -> omega_regalloc::TerminalLivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> omega_regalloc::TerminalAllocationLegalityIdentity {
        self.final_legality
    }
    pub const fn final_virtual_register_count(&self) -> usize {
        self.final_virtual_register_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLiteralFoldAttemptReceipt {
    source_selected: TerminalSelectedInstructionPlanIdentity,
    source_ranges: omega_regalloc::TerminalLiveRangeIdentity,
    source_legality: omega_regalloc::TerminalAllocationLegalityIdentity,
    choices: omega_regalloc::TerminalSpillChoiceIdentity,
    choice_policy: TerminalSpillChoicePolicy,
    choice_usage: OptimizationWorkUsage,
    recovery: omega_regalloc::TerminalRecoveryClassificationIdentity,
    recovery_policy: TerminalRecoveryClassificationPolicy,
    recovery_usage: OptimizationWorkUsage,
    fold: TerminalLiteralFoldIdentity,
    fold_policy: TerminalLiteralFoldPolicy,
    fold_usage: OptimizationWorkUsage,
    applied_count: usize,
    transformed_selected: TerminalSelectedInstructionPlanIdentity,
}

impl StagedOptimizedLiteralFoldAttemptReceipt {
    pub const fn source_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> omega_regalloc::TerminalAllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn choices(self) -> omega_regalloc::TerminalSpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> TerminalSpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn recovery(self) -> omega_regalloc::TerminalRecoveryClassificationIdentity {
        self.recovery
    }
    pub const fn recovery_policy(self) -> TerminalRecoveryClassificationPolicy {
        self.recovery_policy
    }
    pub const fn recovery_usage(self) -> OptimizationWorkUsage {
        self.recovery_usage
    }
    pub const fn fold(self) -> TerminalLiteralFoldIdentity {
        self.fold
    }
    pub const fn fold_policy(self) -> TerminalLiteralFoldPolicy {
        self.fold_policy
    }
    pub const fn fold_usage(self) -> OptimizationWorkUsage {
        self.fold_usage
    }
    pub const fn applied_count(self) -> usize {
        self.applied_count
    }
    pub const fn transformed_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.transformed_selected
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedLiteralFoldCustodyReceipt {
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    iterations: Vec<StagedOptimizedLiteralFoldIterationReceipt>,
    transformations: Vec<TerminalLiteralFoldIdentity>,
    final_selected: TerminalSelectedInstructionPlanIdentity,
    final_liveness: omega_regalloc::TerminalLivenessIdentity,
    final_ranges: omega_regalloc::TerminalLiveRangeIdentity,
    final_legality: omega_regalloc::TerminalAllocationLegalityIdentity,
    final_virtual_register_count: usize,
    final_entry_transition_count: usize,
}

impl StagedOptimizedLiteralFoldCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.source
    }
    pub fn iterations(&self) -> &[StagedOptimizedLiteralFoldIterationReceipt] {
        &self.iterations
    }
    pub fn transformations(&self) -> &[TerminalLiteralFoldIdentity] {
        &self.transformations
    }
    pub const fn final_selected(&self) -> TerminalSelectedInstructionPlanIdentity {
        self.final_selected
    }
    pub const fn final_liveness(&self) -> omega_regalloc::TerminalLivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> omega_regalloc::TerminalAllocationLegalityIdentity {
        self.final_legality
    }
    pub const fn final_virtual_register_count(&self) -> usize {
        self.final_virtual_register_count
    }
    pub const fn final_entry_transition_count(&self) -> usize {
        self.final_entry_transition_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLiteralFoldIterationReceipt {
    source_selected: TerminalSelectedInstructionPlanIdentity,
    source_ranges: omega_regalloc::TerminalLiveRangeIdentity,
    source_legality: omega_regalloc::TerminalAllocationLegalityIdentity,
    choices: omega_regalloc::TerminalSpillChoiceIdentity,
    choice_policy: TerminalSpillChoicePolicy,
    choice_usage: OptimizationWorkUsage,
    recovery: omega_regalloc::TerminalRecoveryClassificationIdentity,
    recovery_policy: TerminalRecoveryClassificationPolicy,
    recovery_usage: OptimizationWorkUsage,
    fold: TerminalLiteralFoldIdentity,
    fold_policy: TerminalLiteralFoldPolicy,
    fold_usage: OptimizationWorkUsage,
    transformed_selected: TerminalSelectedInstructionPlanIdentity,
    fresh_liveness: omega_regalloc::TerminalLivenessIdentity,
    fresh_ranges: omega_regalloc::TerminalLiveRangeIdentity,
    fresh_legality: omega_regalloc::TerminalAllocationLegalityIdentity,
}

impl StagedOptimizedLiteralFoldIterationReceipt {
    pub const fn source_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> omega_regalloc::TerminalAllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn choices(self) -> omega_regalloc::TerminalSpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> TerminalSpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn recovery(self) -> omega_regalloc::TerminalRecoveryClassificationIdentity {
        self.recovery
    }
    pub const fn recovery_policy(self) -> TerminalRecoveryClassificationPolicy {
        self.recovery_policy
    }
    pub const fn recovery_usage(self) -> OptimizationWorkUsage {
        self.recovery_usage
    }
    pub const fn fold(self) -> TerminalLiteralFoldIdentity {
        self.fold
    }
    pub const fn fold_policy(self) -> TerminalLiteralFoldPolicy {
        self.fold_policy
    }
    pub const fn fold_usage(self) -> OptimizationWorkUsage {
        self.fold_usage
    }
    pub const fn transformed_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn fresh_liveness(self) -> omega_regalloc::TerminalLivenessIdentity {
        self.fresh_liveness
    }
    pub const fn fresh_ranges(self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.fresh_ranges
    }
    pub const fn fresh_legality(self) -> omega_regalloc::TerminalAllocationLegalityIdentity {
        self.fresh_legality
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLiteralFoldCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
    SpillChoice(TerminalSpillChoiceError),
    RecoveryClassification(TerminalRecoveryClassificationError),
    Fold(TerminalLiteralFoldError),
    NoAppliedFold,
    Liveness(TerminalLivenessError),
    LiveRanges(TerminalLiveRangeError),
    AllocationLegality(TerminalAllocationLegalityError),
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

impl std::fmt::Display for OptimizedLiteralFoldCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized literal-fold staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedLiteralFoldCustodyError {}

#[allow(clippy::too_many_arguments)]
pub fn stage_first_optimized_literal_fold(
    source: StagedOptimizedAllocationLegality,
    choice_policy: TerminalSpillChoicePolicy,
    recovery_policy: TerminalRecoveryClassificationPolicy,
    fold_policy: TerminalLiteralFoldPolicy,
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
    choice_policy: TerminalSpillChoicePolicy,
    recovery_policy: TerminalRecoveryClassificationPolicy,
    fold_policy: TerminalLiteralFoldPolicy,
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

/// Execute the exact selected-lowering projection to a validated fixed point.
/// The named family uses compiler-owned deterministic policies and the work
/// budget already retained by upstream optimizer custody.
pub fn run_selected_lowering_optimizations(
    source: StagedOptimizedAllocationLegality,
) -> Result<StagedSelectedLoweringOptimizationRun, OptimizedLiteralFoldCustodyError> {
    let upstream = validate_source(&source)?;
    let optimized = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized();
    let selections = optimized.selections().clone();
    let budget = optimized.budget_per_pass();
    let selected_lowering_selections =
        selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    if selected_lowering_selections.is_empty() {
        return Err(OptimizedLiteralFoldCustodyError::MissingSelectedLoweringOptimization);
    }
    if let Some(unsupported) = selected_lowering_selections
        .as_slice()
        .iter()
        .find(|optimization| {
            !matches!(
                optimization,
                Optimization::SelectedIncomingU12ExactAddImmediate
            )
        })
    {
        return Err(
            OptimizedLiteralFoldCustodyError::UnsupportedSelectedLoweringOptimization(*unsupported),
        );
    }

    let choice_policy = TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1;
    let recovery_policy =
        TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1;
    let fold_policy = TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1;
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
    );
    Ok(StagedSelectedLoweringOptimizationRun {
        source,
        selections,
        selected_lowering_selections,
        steps,
        terminal_attempt: attempt,
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
    if run.selections != *optimized.selections()
        || run.custody.selections != optimized.selections().identity()
        || run.custody.budget != expected_budget
        || run.custody.schedule
            != SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddImmediateToNoChangeV1
    {
        return Err(OptimizedLiteralFoldCustodyError::SelectionProjectionMismatch);
    }
    let projected = run
        .selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering);
    if projected != run.selected_lowering_selections
        || projected.as_slice() != [Optimization::SelectedIncomingU12ExactAddImmediate]
    {
        return Err(OptimizedLiteralFoldCustodyError::SelectionProjectionMismatch);
    }
    for step in &run.steps {
        validate_selected_lowering_schedule(
            step.choices(),
            step.recovery(),
            step.fold(),
            expected_budget,
        )?;
    }
    validate_selected_lowering_schedule(
        run.terminal_attempt.choices(),
        run.terminal_attempt.recovery(),
        run.terminal_attempt.fold(),
        expected_budget,
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
            TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
            expected_budget,
        )?,
        None => build_attempt(
            selected,
            run.source.live_range_stage().ranges(),
            run.source.legality(),
            &run.source,
            TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
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
    );
    if replayed != run.steps || terminal != run.terminal_attempt || receipt != run.custody {
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
fn build_step<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    source: &StagedOptimizedAllocationLegality,
    choice_policy: TerminalSpillChoicePolicy,
    recovery_policy: TerminalRecoveryClassificationPolicy,
    fold_policy: TerminalLiteralFoldPolicy,
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
fn build_attempt<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    source: &StagedOptimizedAllocationLegality,
    choice_policy: TerminalSpillChoicePolicy,
    recovery_policy: TerminalRecoveryClassificationPolicy,
    fold_policy: TerminalLiteralFoldPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedLiteralFoldAttempt, OptimizedLiteralFoldCustodyError> {
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let choices = choose_terminal_spill_victims(
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
    let recovery = classify_terminal_pressure_recovery(
        selected,
        ranges,
        legality,
        &choices,
        recovery_policy,
        budget,
    )
    .map_err(OptimizedLiteralFoldCustodyError::RecoveryClassification)?;
    let fold = fold_terminal_selected_incoming_literal(
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
    let liveness =
        analyze_terminal_liveness(&fold).map_err(OptimizedLiteralFoldCustodyError::Liveness)?;
    let ranges = analyze_terminal_live_ranges(&fold, &liveness)
        .map_err(OptimizedLiteralFoldCustodyError::LiveRanges)?;
    let legality = analyze_terminal_allocation_legality(
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

fn replay_step<S: ValidatedTerminalSelectedAnalysis>(
    step_index: usize,
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
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

fn custody_receipt(
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

fn iteration_receipt(
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
fn selected_lowering_custody_receipt(
    source_receipt: StagedOptimizedAllocationLegalityCustodyReceipt,
    selections: &OptimizationSelections,
    selected_lowering_selections: &OptimizationSelections,
    source: &StagedOptimizedAllocationLegality,
    steps: &[StagedOptimizedLiteralFoldStep],
    terminal_attempt: &StagedOptimizedLiteralFoldAttempt,
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
        schedule:
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddImmediateToNoChangeV1,
        budget,
        usage,
        iteration_bound,
        action_count,
        initial_virtual_register_count: source.legality().receipt().virtual_register_count(),
        iterations: steps.iter().map(iteration_receipt).collect(),
        terminal_attempt: attempt_receipt(terminal_attempt),
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

fn selected_lowering_completion_identity(
    receipt: &StagedSelectedLoweringOptimizationCustodyReceipt,
) -> SelectedLoweringOptimizationCompletionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.selected-lowering-optimization-completion.v1\0");
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
    canonical.push(match receipt.schedule {
        SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddImmediateToNoChangeV1 => 1,
    });
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
    encode_attempt_receipt(&mut canonical, receipt.terminal_attempt);
    canonical.extend_from_slice(&receipt.final_selected.bytes());
    canonical.extend_from_slice(&receipt.final_liveness.bytes());
    canonical.extend_from_slice(&receipt.final_ranges.bytes());
    canonical.extend_from_slice(&receipt.final_legality.bytes());
    encode_count(&mut canonical, receipt.final_virtual_register_count);
    SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(&canonical)
}

fn encode_iteration_receipt(
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

fn encode_attempt_receipt(
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

fn spill_choice_policy_tag(policy: TerminalSpillChoicePolicy) -> u8 {
    match policy {
        TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 => 1,
    }
}

fn recovery_policy_tag(policy: TerminalRecoveryClassificationPolicy) -> u8 {
    match policy {
        TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 => 1,
    }
}

fn literal_fold_policy_tag(policy: TerminalLiteralFoldPolicy) -> u8 {
    match policy {
        TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1 => 1,
    }
}

fn encode_count(canonical: &mut Vec<u8>, count: usize) {
    canonical.extend_from_slice(
        &u64::try_from(count)
            .expect("selected-lowering completion count fits u64")
            .to_le_bytes(),
    );
}

fn attempt_receipt(
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

fn step_usage(
    step: &StagedOptimizedLiteralFoldStep,
) -> Result<OptimizationWorkUsage, OptimizedLiteralFoldCustodyError> {
    let choices_and_recovery = add_usage(
        step.choices.receipt().usage(),
        step.recovery.receipt().usage(),
    )?;
    add_usage(choices_and_recovery, step.fold.receipt().usage())
}

fn applied_action_count(
    steps: &[StagedOptimizedLiteralFoldStep],
) -> Result<usize, OptimizedLiteralFoldCustodyError> {
    steps.iter().try_fold(0_usize, |count, step| {
        count
            .checked_add(step.fold.receipt().applied_count())
            .ok_or(OptimizedLiteralFoldCustodyError::WorkOverflow)
    })
}

fn attempt_usage(
    attempt: &StagedOptimizedLiteralFoldAttempt,
) -> Result<OptimizationWorkUsage, OptimizedLiteralFoldCustodyError> {
    let choices_and_recovery = add_usage(
        attempt.choices.receipt().usage(),
        attempt.recovery.receipt().usage(),
    )?;
    add_usage(choices_and_recovery, attempt.fold.receipt().usage())
}

fn add_usage(
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

fn ensure_selected_lowering_budget(
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

fn validate_selected_lowering_schedule(
    choices: &ValidatedTerminalSpillChoices,
    recovery: &ValidatedTerminalRecoveryClassifications,
    fold: &ValidatedTerminalLiteralFold,
    budget: OptimizationWorkBudget,
) -> Result<(), OptimizedLiteralFoldCustodyError> {
    if choices.receipt().policy()
        != TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
        || recovery.receipt().policy()
            != TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1
        || fold.receipt().policy()
            != TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1
        || choices.plan().budget != budget
        || recovery.plan().budget != budget
        || fold.plan().budget != budget
    {
        return Err(OptimizedLiteralFoldCustodyError::SelectionProjectionMismatch);
    }
    Ok(())
}
