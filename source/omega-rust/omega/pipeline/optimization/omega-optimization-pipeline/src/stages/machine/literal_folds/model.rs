//! Selected-lowering carriers, receipts, schedules, and errors.

use super::*;

/// One explicitly requested pressure decision, semantic classification,
/// literal fold, and complete analysis reconstruction. No source analysis fact
/// crosses the transformed selected-CFG boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedLiteralFoldStep {
    pub(super) choices: ValidatedSpillChoices,
    pub(super) recovery: ValidatedRecoveryClassifications,
    pub(super) fold: ValidatedLiteralFold,
    pub(super) liveness: ValidatedLiveness,
    pub(super) ranges: ValidatedLiveRanges,
    pub(super) legality: ValidatedAllocationLegality,
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
    pub(super) choices: ValidatedSpillChoices,
    pub(super) recovery: ValidatedRecoveryClassifications,
    pub(super) fold: ValidatedLiteralFold,
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
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) steps: Vec<StagedOptimizedLiteralFoldStep>,
    pub(super) custody: StagedOptimizedLiteralFoldCustodyReceipt,
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
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) selections: OptimizationSelections,
    pub(super) selected_lowering_selections: OptimizationSelections,
    pub(super) steps: Vec<StagedOptimizedLiteralFoldStep>,
    pub(super) attempt: StagedOptimizedLiteralFoldAttempt,
    pub(super) custody: StagedSelectedLoweringOptimizationCustodyReceipt,
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
    pub(super) identity: SelectedLoweringOptimizationCompletionIdentity,
    pub(super) source: StagedOptimizedAllocationLegalityCustodyReceipt,
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) selected_lowering_selections: OptimizationSelectionIdentity,
    pub(super) budget: OptimizationWorkBudget,
    pub(super) usage: OptimizationWorkUsage,
    pub(super) iteration_bound: usize,
    pub(super) action_count: usize,
    pub(super) initial_virtual_register_count: usize,
    pub(super) iterations: Vec<StagedOptimizedLiteralFoldIterationReceipt>,
    pub(super) attempt: StagedOptimizedLiteralFoldAttemptReceipt,
    pub(super) final_selected: SelectedInstructionPlanIdentity,
    pub(super) final_liveness: omega_regalloc::LivenessIdentity,
    pub(super) final_ranges: omega_regalloc::LiveRangeIdentity,
    pub(super) final_legality: omega_regalloc::AllocationLegalityIdentity,
    pub(super) final_virtual_register_count: usize,
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
    pub const fn final_liveness(&self) -> omega_regalloc::LivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> omega_regalloc::LiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> omega_regalloc::AllocationLegalityIdentity {
        self.final_legality
    }
    pub const fn final_virtual_register_count(&self) -> usize {
        self.final_virtual_register_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLiteralFoldAttemptReceipt {
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) source_ranges: omega_regalloc::LiveRangeIdentity,
    pub(super) source_legality: omega_regalloc::AllocationLegalityIdentity,
    pub(super) choices: omega_regalloc::SpillChoiceIdentity,
    pub(super) choice_policy: SpillChoicePolicy,
    pub(super) choice_usage: OptimizationWorkUsage,
    pub(super) recovery: omega_regalloc::RecoveryClassificationIdentity,
    pub(super) recovery_policy: RecoveryClassificationPolicy,
    pub(super) recovery_usage: OptimizationWorkUsage,
    pub(super) fold: LiteralFoldIdentity,
    pub(super) fold_policy: LiteralFoldPolicy,
    pub(super) fold_usage: OptimizationWorkUsage,
    pub(super) applied_count: usize,
    pub(super) transformed_selected: SelectedInstructionPlanIdentity,
}

impl StagedOptimizedLiteralFoldAttemptReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> omega_regalloc::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> omega_regalloc::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn choices(self) -> omega_regalloc::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn recovery(self) -> omega_regalloc::RecoveryClassificationIdentity {
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
    pub(super) source: StagedOptimizedAllocationLegalityCustodyReceipt,
    pub(super) iterations: Vec<StagedOptimizedLiteralFoldIterationReceipt>,
    pub(super) transformations: Vec<LiteralFoldIdentity>,
    pub(super) final_selected: SelectedInstructionPlanIdentity,
    pub(super) final_liveness: omega_regalloc::LivenessIdentity,
    pub(super) final_ranges: omega_regalloc::LiveRangeIdentity,
    pub(super) final_legality: omega_regalloc::AllocationLegalityIdentity,
    pub(super) final_virtual_register_count: usize,
    pub(super) final_entry_transition_count: usize,
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
    pub const fn final_liveness(&self) -> omega_regalloc::LivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> omega_regalloc::LiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> omega_regalloc::AllocationLegalityIdentity {
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
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) source_ranges: omega_regalloc::LiveRangeIdentity,
    pub(super) source_legality: omega_regalloc::AllocationLegalityIdentity,
    pub(super) choices: omega_regalloc::SpillChoiceIdentity,
    pub(super) choice_policy: SpillChoicePolicy,
    pub(super) choice_usage: OptimizationWorkUsage,
    pub(super) recovery: omega_regalloc::RecoveryClassificationIdentity,
    pub(super) recovery_policy: RecoveryClassificationPolicy,
    pub(super) recovery_usage: OptimizationWorkUsage,
    pub(super) fold: LiteralFoldIdentity,
    pub(super) fold_policy: LiteralFoldPolicy,
    pub(super) fold_usage: OptimizationWorkUsage,
    pub(super) transformed_selected: SelectedInstructionPlanIdentity,
    pub(super) fresh_liveness: omega_regalloc::LivenessIdentity,
    pub(super) fresh_ranges: omega_regalloc::LiveRangeIdentity,
    pub(super) fresh_legality: omega_regalloc::AllocationLegalityIdentity,
}

impl StagedOptimizedLiteralFoldIterationReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> omega_regalloc::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> omega_regalloc::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn choices(self) -> omega_regalloc::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn recovery(self) -> omega_regalloc::RecoveryClassificationIdentity {
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
    pub const fn fresh_liveness(self) -> omega_regalloc::LivenessIdentity {
        self.fresh_liveness
    }
    pub const fn fresh_ranges(self) -> omega_regalloc::LiveRangeIdentity {
        self.fresh_ranges
    }
    pub const fn fresh_legality(self) -> omega_regalloc::AllocationLegalityIdentity {
        self.fresh_legality
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLiteralFoldCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
    SpillChoice(SpillChoiceError),
    RecoveryClassification(RecoveryClassificationError),
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
