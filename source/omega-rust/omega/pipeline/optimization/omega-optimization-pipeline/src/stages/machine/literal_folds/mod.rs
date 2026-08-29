//! Selected-lowering literal-fold stage entrance.
//!
//! `omega_regalloc::rules` owns the exact-name catalog. This file consumes its
//! selected policy and owns custody-stage dispatch; lower rungs separate
//! carriers, execution/replay, scheduling receipts, and work accounting.

use omega_optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
    OptimizationWorkUsage, SelectedLoweringOptimizationCompletionIdentity,
};
use omega_regalloc::{
    AllocationLegalityError, LiteralFoldError, LiteralFoldIdentity, LiteralFoldPolicy,
    LiveRangeError, LivenessError, RecoveryClassificationError, RecoveryClassificationPolicy,
    SpillChoiceError, SpillChoicePolicy, ValidatedAllocationLegality, ValidatedLiteralFold,
    ValidatedLiveRanges, ValidatedLiveness, ValidatedRecoveryClassifications,
    ValidatedSelectedAnalysis, ValidatedSpillChoices, analyze_allocation_legality,
    analyze_live_ranges, analyze_liveness, choose_spill_victims, classify_pressure_recovery,
    fold_selected_incoming_literal, selected_lowering_rule_policy,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt,
    validate_optimized_allocation_legality_custody,
};

mod accounting;
mod execution;
mod model;
mod schedule;

pub use execution::{
    stage_first_optimized_literal_fold, stage_next_optimized_literal_fold,
    validate_optimized_literal_fold_custody, validate_selected_lowering_optimization_custody,
};
pub use model::*;
pub use omega_regalloc::ORDERED_SELECTED_LOWERING_RULES;

use schedule::selected_lowering_schedule;

impl From<omega_regalloc::SelectedLoweringRuleCatalogError> for OptimizedLiteralFoldCustodyError {
    fn from(error: omega_regalloc::SelectedLoweringRuleCatalogError) -> Self {
        match error {
            omega_regalloc::SelectedLoweringRuleCatalogError::MissingSelection => {
                Self::MissingSelectedLoweringOptimization
            }
            omega_regalloc::SelectedLoweringRuleCatalogError::UnsupportedSelection(
                optimization,
            ) => Self::UnsupportedSelectedLoweringOptimization(optimization),
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
    let (selected, fold_policy) = selected_lowering_rule_policy(&selections)?;
    let schedule = selected_lowering_schedule(fold_policy);
    execution::execute_selected_lowering_optimizations(
        source,
        selections,
        selected,
        schedule,
        fold_policy,
    )
}
