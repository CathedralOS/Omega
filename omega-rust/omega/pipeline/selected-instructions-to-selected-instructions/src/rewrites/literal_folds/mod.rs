//! Optimizer module role: executable entrance. Selected-lowering literal-fold stage entrance.
//!
//! `crate::rules` owns the exact-name catalog. This file consumes its
//! selected policy and owns custody-stage dispatch; lower rungs separate
//! carriers, execution/replay, scheduling receipts, and work accounting.

use crate::{ValidatedSelectedAnalysis, resolve_selected_lowering_rules};
use optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
    OptimizationWorkUsage, SelectedLoweringOptimizationCompletionIdentity,
};
use register_homes::{RecoveryClassificationPolicy, SpillChoicePolicy};
use selected_instructions::SelectedInstructionPlanIdentity;

use crate::StagedOptimizedAllocationLegality;

mod accounting;
mod execution;
#[cfg(any(test, feature = "test-support"))]
mod test_support;

use crate::{
    AllocationLegalityError, LiteralFoldError, LiteralFoldIdentity, LiteralFoldPolicy,
    LiveRangeError, LivenessError, MachineEffectStageError,
    OptimizedAllocationLegalityCustodyError, RecoveryClassificationError, SpillChoiceError,
    StagedOptimizedAllocationLegalityCustodyReceipt, ValidatedAllocationLegality,
    ValidatedLiteralFold, ValidatedLiveRanges, ValidatedLiveness, ValidatedRecoveryClassifications,
    ValidatedSpillChoices,
};
pub use execution::{
    stage_first_optimized_literal_fold, stage_next_optimized_literal_fold,
    validate_optimized_literal_fold_custody, validate_selected_lowering_optimization_custody,
};

#[cfg(any(test, feature = "test-support"))]
pub use test_support::{
    OptimizedLiteralFoldCustodyFieldForTest, SelectedLoweringOptimizationCustodyFieldForTest,
};

impl From<crate::SelectedLoweringRuleCatalogError> for OptimizedLiteralFoldCustodyError {
    fn from(error: crate::SelectedLoweringRuleCatalogError) -> Self {
        match error {
            crate::SelectedLoweringRuleCatalogError::WrongPhase(_) => {
                Self::SelectionProjectionMismatch
            }
            crate::SelectedLoweringRuleCatalogError::MissingSelection => {
                Self::MissingSelectedLoweringOptimization
            }
            crate::SelectedLoweringRuleCatalogError::UnsupportedSelection(optimization) => {
                Self::UnsupportedSelectedLoweringOptimization(optimization)
            }
        }
    }
}

/// Execute the exact selected-lowering projection to a validated fixed point.
pub fn run_selected_lowering_optimizations(
    source: StagedOptimizedAllocationLegality,
) -> Result<StagedSelectedLoweringOptimizationRun, OptimizedLiteralFoldCustodyError> {
    let selections = source.selections().clone();
    let selected_lowering =
        selections.project_phase(optimization_core::OptimizationExecutionPhase::SelectedLowering);
    let (selected, fold_policy) = resolve_selected_lowering_rules(&selected_lowering)?;
    execution::execute_selected_lowering_optimizations(source, selections, selected, fold_policy)
}

/// One explicitly requested pressure decision, semantic classification,
/// literal fold, and complete analysis reconstruction. No source analysis fact
/// crosses the transformed selected-CFG boundary.
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
    pub const fn liveness(&self) -> &ValidatedLiveness {
        &self.liveness
    }
    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        &self.ranges
    }
    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        &self.legality
    }
}

/// One independently validated selected-lowering attempt before deciding
/// whether another transformed-CFG analysis cycle is required. A terminal
/// attempt has `applied_count() == 0` and is positive fixed-point evidence.
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
    pub const fn attempt(&self) -> StagedOptimizedLiteralFoldAttemptReceipt {
        self.attempt
    }
    pub const fn final_selected(&self) -> SelectedInstructionPlanIdentity {
        self.final_selected
    }
    pub const fn final_liveness(&self) -> selected_instructions::LivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> selected_instructions::LiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> register_homes::AllocationLegalityIdentity {
        self.final_legality
    }
    pub const fn final_virtual_register_count(&self) -> usize {
        self.final_virtual_register_count
    }
}

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

impl StagedOptimizedLiteralFoldAttemptReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> selected_instructions::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> register_homes::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn choices(self) -> register_homes::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn recovery(self) -> register_homes::RecoveryClassificationIdentity {
        self.recovery
    }
    pub const fn recovery_policy(self) -> RecoveryClassificationPolicy {
        self.recovery_policy
    }
    pub const fn recovery_usage(self) -> OptimizationWorkUsage {
        self.recovery_usage
    }
    pub const fn fold(self) -> LiteralFoldIdentity {
        self.fold
    }
    pub const fn fold_policy(self) -> LiteralFoldPolicy {
        self.fold_policy
    }
    pub const fn fold_usage(self) -> OptimizationWorkUsage {
        self.fold_usage
    }
    pub const fn applied_count(self) -> usize {
        self.applied_count
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedLiteralFoldCustodyReceipt {
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    iterations: Vec<StagedOptimizedLiteralFoldIterationReceipt>,
    transformations: Vec<LiteralFoldIdentity>,
    final_selected: SelectedInstructionPlanIdentity,
    final_liveness: selected_instructions::LivenessIdentity,
    final_ranges: selected_instructions::LiveRangeIdentity,
    final_legality: register_homes::AllocationLegalityIdentity,
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
    pub fn transformations(&self) -> &[LiteralFoldIdentity] {
        &self.transformations
    }
    pub const fn final_selected(&self) -> SelectedInstructionPlanIdentity {
        self.final_selected
    }
    pub const fn final_liveness(&self) -> selected_instructions::LivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> selected_instructions::LiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> register_homes::AllocationLegalityIdentity {
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
    transformed_selected: SelectedInstructionPlanIdentity,
    fresh_liveness: selected_instructions::LivenessIdentity,
    fresh_ranges: selected_instructions::LiveRangeIdentity,
    fresh_legality: register_homes::AllocationLegalityIdentity,
}

impl StagedOptimizedLiteralFoldIterationReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> selected_instructions::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> register_homes::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn choices(self) -> register_homes::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn recovery(self) -> register_homes::RecoveryClassificationIdentity {
        self.recovery
    }
    pub const fn recovery_policy(self) -> RecoveryClassificationPolicy {
        self.recovery_policy
    }
    pub const fn recovery_usage(self) -> OptimizationWorkUsage {
        self.recovery_usage
    }
    pub const fn fold(self) -> LiteralFoldIdentity {
        self.fold
    }
    pub const fn fold_policy(self) -> LiteralFoldPolicy {
        self.fold_policy
    }
    pub const fn fold_usage(self) -> OptimizationWorkUsage {
        self.fold_usage
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn fresh_liveness(self) -> selected_instructions::LivenessIdentity {
        self.fresh_liveness
    }
    pub const fn fresh_ranges(self) -> selected_instructions::LiveRangeIdentity {
        self.fresh_ranges
    }
    pub const fn fresh_legality(self) -> register_homes::AllocationLegalityIdentity {
        self.fresh_legality
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLiteralFoldCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
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

impl std::fmt::Display for OptimizedLiteralFoldCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized literal-fold staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedLiteralFoldCustodyError {}
