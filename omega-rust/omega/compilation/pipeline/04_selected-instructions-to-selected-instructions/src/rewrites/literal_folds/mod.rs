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

use crate::{
    AllocationLegalityError, LiteralFoldError, LiteralFoldIdentity, LiteralFoldPolicy,
    LiveRangeError, LivenessError, MachineEffectStageError, RecoveryClassificationError,
    SelectedLoweringRuleCatalogError, SpillChoiceError,
    StagedOptimizedAllocationLegalityCustodyReceipt, ValidatedAllocationLegality,
    ValidatedLiteralFold, ValidatedLiveRanges, ValidatedLiveness, ValidatedRecoveryClassifications,
    ValidatedSpillChoices,
};
pub(crate) use execution::validate_selected_lowering_optimization_custody;

impl From<SelectedLoweringRuleCatalogError> for OptimizedLiteralFoldCustodyError {
    fn from(error: SelectedLoweringRuleCatalogError) -> Self {
        match error {
            SelectedLoweringRuleCatalogError::WrongPhase(_) => Self::SelectionProjectionMismatch,
            SelectedLoweringRuleCatalogError::MissingSelection => {
                Self::MissingSelectedLoweringOptimization
            }
            SelectedLoweringRuleCatalogError::UnsupportedSelection(optimization) => {
                Self::UnsupportedSelectedLoweringOptimization(optimization)
            }
        }
    }
}

/// Execute the exact selected-lowering projection to a validated fixed point.
pub(crate) fn run_selected_lowering_optimizations(
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
    validated_analysis_accessors!();
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

impl StagedSelectedLoweringOptimizationRun {
    /// The completed run as register allocation reads it: the final program
    /// and the facts rebuilt over it (the last step's, or the admitted
    /// source's when the suite applied no fold), and the run's ledger.
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
