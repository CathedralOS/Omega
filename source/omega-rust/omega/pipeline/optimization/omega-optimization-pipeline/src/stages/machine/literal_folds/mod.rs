//! Selected-lowering literal-fold stage entrance.
//!
//! The exact-name catalog is local, this file owns selection projection and
//! dispatch, and the lower rungs separate carriers, execution/replay, and
//! identity/budget accounting.

use omega_optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    SelectedLoweringOptimizationCompletionIdentity,
};
use omega_regalloc::{
    AllocationLegalityError, LiteralFoldError, LiteralFoldIdentity, LiteralFoldPolicy,
    LiveRangeError, LivenessError, RecoveryClassificationError, RecoveryClassificationPolicy,
    SpillChoiceError, SpillChoicePolicy, ValidatedAllocationLegality, ValidatedLiteralFold,
    ValidatedLiveRanges, ValidatedLiveness, ValidatedRecoveryClassifications,
    ValidatedSelectedAnalysis, ValidatedSpillChoices, analyze_allocation_legality,
    analyze_live_ranges, analyze_liveness, choose_spill_victims, classify_pressure_recovery,
    fold_selected_incoming_literal,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt,
    validate_optimized_allocation_legality_custody,
};

mod accounting;
mod catalog;
mod execution;
mod model;

pub use catalog::ORDERED_SELECTED_LOWERING_RULES;
pub use execution::{
    stage_first_optimized_literal_fold, stage_next_optimized_literal_fold,
    validate_optimized_literal_fold_custody, validate_selected_lowering_optimization_custody,
};
pub use model::*;

use catalog::selected_lowering_contract;

/// Execute the exact selected-lowering projection to a validated fixed point.
pub fn run_selected_lowering_optimizations(
    source: StagedOptimizedAllocationLegality,
) -> Result<StagedSelectedLoweringOptimizationRun, OptimizedLiteralFoldCustodyError> {
    let selections = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
        .clone();
    let selected = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    if selected.is_empty() {
        return Err(OptimizedLiteralFoldCustodyError::MissingSelectedLoweringOptimization);
    }
    let (schedule, fold_policy) = selected_lowering_contract(&selected)?;
    execution::execute_selected_lowering_optimizations(
        source,
        selections,
        selected,
        schedule,
        fold_policy,
    )
}
