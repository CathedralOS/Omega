#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-program rewrites before register assignment.
//!
//! Rewrites operate on the selected CFG and reconstruct the analyses they
//! invalidate. Register assignment consumes the resulting current program.
//!
//! Start at [`optimize_selected_instructions`] in `selected_optimization.rs`:
//! liveness and live ranges from [`analyses`], then either the identity route
//! or the selected-lowering [`rewrites`] under allocation legality, published
//! as one [`SelectedInstructionOptimizationOutput`] from
//! `selected_optimization::optimization_output`.

mod analyses;
mod peepholes;
mod rewrites;
mod selected_optimization;

pub use analyses::{
    AllocationLegalityError, AllocationLegalityValidationReceipt, AllocatorAvailabilityError,
    AllocatorAvailabilityValidationReceipt, BlockMachineEffects, FixedPrecoloredIntervalError,
    FixedPrecoloredIntervalValidationReceipt, FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSegmentHomeValidationReceipt, FixedPrecoloredSplitRequirementError,
    FixedPrecoloredSplitRequirementValidationReceipt, FunctionMachineEffects,
    InstructionMachineEffects, LiveRangeError, LiveRangeValidationReceipt, LivenessError,
    LivenessValidationReceipt, MachineEffectError, MachineEffectStageError,
    OptimizedAllocationLegalityCustodyError, OptimizedLiveRangeCustodyError,
    OptimizedLivenessCustodyError, OptimizedSelectedReanalysisError, OwnedSelectedProgram,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, PreAllocationMachineEffectReceipt, RecoveryClassificationError,
    RecoveryClassificationValidationReceipt, SelectedProgramRef, SpillChoiceError,
    SpillChoiceValidationReceipt, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, StagedOptimizedLiveRangeCustodyReceipt,
    StagedOptimizedLiveRanges, StagedOptimizedLiveness, StagedOptimizedLivenessCustodyReceipt,
    StagedOptimizedSelectedReanalysis, StagedOptimizedSelectedReanalysisCustodyReceipt,
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedFixedPrecoloredIntervals,
    ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges, ValidatedLiveness, ValidatedPreAllocationMachineEffects,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
    analyze_allocation_legality, analyze_fixed_precolored_intervals,
    analyze_fixed_precolored_split_requirements, analyze_live_ranges, analyze_live_ranges_reusing,
    analyze_liveness, analyze_liveness_reusing, analyze_machine_effects,
    analyze_pre_allocation_machine_effects, assign_fixed_precolored_segment_homes,
    choose_spill_victims, classify_pressure_recovery, materialize_allocator_availability,
    pre_allocation_machine_effect_identity, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_allocation_legality_for_frameless_leaf,
    stage_optimized_allocation_legality_with_availability, stage_optimized_live_ranges,
    stage_optimized_liveness, stage_optimized_selected_reanalysis, validate_allocation_legality,
    validate_allocator_availability, validate_fixed_precolored_intervals,
    validate_fixed_precolored_segment_homes, validate_fixed_precolored_split_requirements,
    validate_live_ranges, validate_liveness, validate_machine_effects,
    validate_optimized_allocation_legality_custody, validate_optimized_live_range_custody,
    validate_optimized_liveness_custody, validate_optimized_selected_reanalysis_custody,
    validate_pre_allocation_machine_effects, validate_recovery_classifications,
    validate_spill_choices, validate_staged_optimized_liveness_custody,
    validated_machine_effect_catalog,
};
#[cfg(any(test, feature = "test-support"))]
pub use analyses::{
    OptimizedAllocationLegalityCustodyFieldForTest, OptimizedLiveRangeCustodyFieldForTest,
    OptimizedLivenessCustodyFieldForTest, OptimizedSelectedReanalysisCustodyFieldForTest,
};
pub use peepholes::{
    ConditionMaterializationError, ConditionMaterializationReceipt, CopiedCallOperandError,
    CopiedCallOperandReceipt, ProjectedAccessError, ProjectedAccessReceipt, TerminatorPairError,
    TerminatorPairReceipt, ValidatedConditionMaterialization, ValidatedCopiedCallOperand,
    ValidatedProjectedAccess, ValidatedTerminatorPair, fold_selected_condition_materialization,
    fold_selected_copied_call_operand, fold_selected_projected_access,
    fold_selected_terminator_pair, validate_condition_materialization_fold,
    validate_copied_call_operand_fold, validate_projected_access_fold,
    validate_terminator_pair_fold,
};
pub use rewrites::unexecuted;
pub use rewrites::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry,
    AllocationRecoveryRuleCatalogError, AllocationRecoveryRuleCatalogPayload,
    FixedPrecoloredSegmentHomeDecline, FixedViewCopy, FixedViewCopyDecodeError,
    FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPlan, FixedViewCopyPolicy,
    FixedViewCopySourceEvidence, FixedViewCopyValidationReceipt, FunctionLiteralFold,
    FunctionPressureRematerialization, LiteralFoldAction, LiteralFoldDecodeError, LiteralFoldError,
    LiteralFoldIdentity, LiteralFoldPlan, LiteralFoldPolicy, LiteralFoldValidationReceipt,
    ORDERED_ALLOCATION_RECOVERY_RULES, ORDERED_SELECTED_LOWERING_RULES,
    OptimizedFixedPrecoloredSegmentHomeCustodyError, OptimizedFixedViewCopyCustodyError,
    OptimizedLiteralFoldCustodyError, PairConsumerBindingAdmission, PairFaultDischarge,
    PairImmediateBound, PairLiteralPosition, PairMachineEffects, PairNonUnitSurface,
    PairOperandResult, PairOperandShape, PairResultDisposition, PairTailCustody,
    PairUnitDefRelation, PairUnitEffects, PressureRematerializationAction,
    PressureRematerializationDecodeError, PressureRematerializationError,
    PressureRematerializationPlan, PressureRematerializationPolicy,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
    RegisterAllocationRuleTargetApplicability, RuntimeRematerializationError,
    RuntimeRematerializationReceipt, RuntimeSpillError, RuntimeSpillReceipt,
    RuntimeSpillSpanPolicy, SELECTED_LOWERING_RULE_CATALOG, SELECTED_STAGE_RULE_CATALOG,
    SelectedInstructionPairRule, SelectedLoweringRuleCatalogEntry,
    SelectedLoweringRuleCatalogError, SelectedLoweringRuleCatalogPayload,
    SelectedStageRuleCatalogSlice, SelectedStageRuleRows,
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    StagedOptimizedFixedPrecoloredSegmentHomes, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt, StagedOptimizedLiteralFoldAttempt,
    StagedOptimizedLiteralFoldAttemptReceipt, StagedOptimizedLiteralFoldCustodyReceipt,
    StagedOptimizedLiteralFoldIterationReceipt, StagedOptimizedLiteralFoldStep,
    StagedOptimizedLiteralFolds, StagedSelectedLoweringOptimizationCustodyReceipt,
    StagedSelectedLoweringOptimizationRun, ValidatedFixedViewCopies, ValidatedLiteralFold,
    ValidatedPressureRematerialization, ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
    enabled_pair_rules, fixed_view_copy_identity, fold_selected_incoming_literal,
    literal_fold_identity, materialize_fixed_view_copies, pressure_rematerialization_identity,
    probe_optimized_fixed_precolored_segment_homes, rematerialize_selected_active_resident,
    rematerialize_selected_runtime_value, resolve_selected_lowering_rules,
    run_selected_lowering_optimizations, selected_allocation_recovery_rule,
    selected_stage_catalog_contains, selected_stage_rule_rows, spill_selected_runtime_value,
    spill_selected_runtime_value_with_span_policy, stage_first_optimized_literal_fold,
    stage_next_optimized_literal_fold, stage_optimized_fixed_precolored_segment_homes,
    stage_optimized_fixed_view_copies, validate_fixed_view_copies, validate_literal_fold,
    validate_optimized_fixed_precolored_segment_home_custody,
    validate_optimized_fixed_view_copy_custody, validate_optimized_literal_fold_custody,
    validate_pressure_rematerialization, validate_runtime_rematerialization,
    validate_runtime_spill, validate_runtime_spill_with_span_policy,
    validate_selected_lowering_optimization_custody,
};
#[cfg(any(test, feature = "test-support"))]
pub use rewrites::{
    OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest,
    OptimizedFixedViewCopyCustodyFieldForTest, OptimizedLiteralFoldCustodyFieldForTest,
    SelectedLoweringOptimizationCustodyFieldForTest,
};
pub use selected_optimization::optimization_output::{
    SelectedInstructionOptimizationError, SelectedInstructionOptimizationEvidence,
    SelectedInstructionOptimizationOutput,
};
pub use selected_optimization::optimize_selected_instructions;

#[cfg(any(test, feature = "test-support"))]
pub use rewrites::test_support;

// Durable transform-output identities live with the selected-instruction
// representation; the transform publishes them under its root so callers keep
// one vocabulary.
pub use selected_instructions::{FixedViewCopyIdentity, PressureRematerializationIdentity};
