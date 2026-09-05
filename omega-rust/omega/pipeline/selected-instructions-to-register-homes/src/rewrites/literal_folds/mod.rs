//! Optimizer module role: executable entrance. Selected-lowering literal-fold stage entrance.
//!
//! `crate::rules` owns the exact-name catalog. This file consumes its
//! selected policy and owns custody-stage dispatch; lower rungs separate
//! carriers, execution/replay, scheduling receipts, and work accounting.

use crate::{
    AllocationLegalityError, LiteralFoldError, LiteralFoldPolicy, LiveRangeError, LivenessError,
    RecoveryClassificationError, RecoveryClassificationPolicy, SpillChoiceError, SpillChoicePolicy,
    ValidatedAllocationLegality, ValidatedLiteralFold, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
    analyze_allocation_legality, analyze_live_ranges, analyze_liveness, choose_spill_victims,
    classify_pressure_recovery, fold_selected_incoming_literal, resolve_selected_lowering_rules,
};
use optimization_core::{
    Optimization, OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    SelectedLoweringOptimizationCompletionIdentity,
};

use crate::{
    AllocationLegalityCustodyReceipt, OptimizedAllocationLegalityCustodyError,
    StagedOptimizedAllocationLegality, validate_optimized_allocation_legality_custody,
};

mod accounting;
mod execution;
mod model;

pub use execution::{
    stage_first_optimized_literal_fold, stage_next_optimized_literal_fold,
    validate_optimized_literal_fold_custody, validate_selected_lowering_optimization_custody,
};
pub use model::*;

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
    let selections = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
        .clone();
    let selected_lowering =
        selections.project_phase(optimization_core::OptimizationExecutionPhase::SelectedLowering);
    let (selected, fold_policy) = resolve_selected_lowering_rules(&selected_lowering)?;
    execution::execute_selected_lowering_optimizations(source, selections, selected, fold_policy)
}
