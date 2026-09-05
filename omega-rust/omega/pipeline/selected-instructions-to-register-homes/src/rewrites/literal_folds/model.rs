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
    pub(super) custody: LiteralFoldCustodyReceipt,
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
    pub const fn custody(&self) -> &LiteralFoldCustodyReceipt {
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
    pub(super) custody: SelectedLoweringOptimizationCustodyReceipt,
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
    pub const fn custody(&self) -> &SelectedLoweringOptimizationCustodyReceipt {
        &self.custody
    }
}

pub use register_homes::SelectedLoweringOptimizationCustodyReceipt;

pub use register_homes::LiteralFoldAttemptReceipt;

pub use register_homes::LiteralFoldCustodyReceipt;

pub use register_homes::LiteralFoldIterationReceipt;

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
