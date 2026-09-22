//! Optimizer module role: stage group. Selected-CFG rewrites and their replay
//! evidence, split by whether the stage executes them.
//!
//! The executed set is what `optimize_selected_instructions` reaches:
//! `catalog` is the stage rule catalog it admits selections through;
//! `selected_lowering` and `literal_folds` execute the selected-lowering
//! slice (literal folds under the pair-rule descriptors); `allocation_recovery`
//! executes the allocation-recovery slice through `fixed_view` (fixed-view
//! copies and fixed precolored segment homes) and the pressure
//! rematerialization it owns; `runtime_spill` and `runtime_rematerialization`
//! are the recovery rewrites register assignment replays. `block_edges` and
//! `window_hazards` are the block-boundary and hazard vocabulary those and
//! the unexecuted families share.
//!
//! `unexecuted` holds every other rewrite family: catalogued in
//! `module_catalog` with the board row that owns its execution, tested in
//! isolation, and reached by no production route. See its module doc for the
//! disposition roster.

mod allocation_recovery;
mod block_edges;
mod catalog;
mod fixed_view;
mod literal_folds;
#[cfg(test)]
mod module_catalog;
mod runtime_rematerialization;
mod runtime_spill;
mod selected_lowering;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod unexecuted;
mod window_hazards;

pub use allocation_recovery::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry,
    AllocationRecoveryRuleCatalogError, AllocationRecoveryRuleCatalogPayload, FixedViewCopy,
    FixedViewCopyDecodeError, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, FixedViewCopyValidationReceipt,
    FunctionPressureRematerialization, ORDERED_ALLOCATION_RECOVERY_RULES,
    PressureRematerializationAction, PressureRematerializationDecodeError,
    PressureRematerializationError, PressureRematerializationPlan, PressureRematerializationPolicy,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
    ValidatedFixedViewCopies, ValidatedPressureRematerialization, fixed_view_copy_identity,
    materialize_fixed_view_copies, pressure_rematerialization_identity,
    rematerialize_selected_active_resident, selected_allocation_recovery_rule,
    validate_fixed_view_copies, validate_pressure_rematerialization,
};
pub use catalog::{
    SELECTED_STAGE_RULE_CATALOG, SelectedStageRuleCatalogSlice, SelectedStageRuleRows,
    selected_stage_catalog_contains, selected_stage_rule_rows,
};
pub use fixed_view::{
    FixedPrecoloredSegmentHomeDecline, OptimizedFixedPrecoloredSegmentHomeCustodyError,
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    StagedOptimizedFixedPrecoloredSegmentHomes, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt, probe_optimized_fixed_precolored_segment_homes,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
    validate_optimized_fixed_precolored_segment_home_custody,
    validate_optimized_fixed_view_copy_custody,
};
#[cfg(any(test, feature = "test-support"))]
pub use fixed_view::{
    OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest,
    OptimizedFixedViewCopyCustodyFieldForTest,
};
pub use literal_folds::{
    OptimizedLiteralFoldCustodyError, StagedOptimizedLiteralFoldAttempt,
    StagedOptimizedLiteralFoldAttemptReceipt, StagedOptimizedLiteralFoldCustodyReceipt,
    StagedOptimizedLiteralFoldIterationReceipt, StagedOptimizedLiteralFoldStep,
    StagedOptimizedLiteralFolds, StagedSelectedLoweringOptimizationCustodyReceipt,
    StagedSelectedLoweringOptimizationRun, run_selected_lowering_optimizations,
    stage_first_optimized_literal_fold, stage_next_optimized_literal_fold,
    validate_optimized_literal_fold_custody, validate_selected_lowering_optimization_custody,
};
#[cfg(any(test, feature = "test-support"))]
pub use literal_folds::{
    OptimizedLiteralFoldCustodyFieldForTest, SelectedLoweringOptimizationCustodyFieldForTest,
};
pub use runtime_rematerialization::{
    RuntimeRematerializationError, RuntimeRematerializationReceipt,
    ValidatedRuntimeRematerialization, rematerialize_selected_runtime_value,
    validate_runtime_rematerialization,
};
pub use runtime_spill::{
    RuntimeSpillError, RuntimeSpillReceipt, RuntimeSpillSpanPolicy, ValidatedRuntimeSpill,
    spill_selected_runtime_value, spill_selected_runtime_value_with_span_policy,
    validate_runtime_spill, validate_runtime_spill_with_span_policy,
};
pub use selected_lowering::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldDecodeError, LiteralFoldError,
    LiteralFoldIdentity, LiteralFoldPlan, LiteralFoldPolicy, LiteralFoldValidationReceipt,
    ORDERED_SELECTED_LOWERING_RULES, PairConsumerBindingAdmission, PairFaultDischarge,
    PairImmediateBound, PairLiteralPosition, PairMachineEffects, PairNonUnitSurface,
    PairOperandResult, PairOperandShape, PairResultDisposition, PairTailCustody,
    PairUnitDefRelation, PairUnitEffects, SELECTED_LOWERING_RULE_CATALOG,
    SelectedInstructionPairRule, SelectedLoweringRuleCatalogEntry,
    SelectedLoweringRuleCatalogError, SelectedLoweringRuleCatalogPayload, ValidatedLiteralFold,
    enabled_pair_rules, fold_selected_incoming_literal, literal_fold_identity,
    resolve_selected_lowering_rules, validate_literal_fold,
};

/// Explicit applicability of the currently architecture-independent rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}
