use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
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
    RemainingTransitions { count: usize },
    EmptySequence,
    StepMismatch { step: usize },
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
    if fold.receipt().applied_count() == 0 {
        return Err(OptimizedLiteralFoldCustodyError::NoAppliedFold);
    }
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
