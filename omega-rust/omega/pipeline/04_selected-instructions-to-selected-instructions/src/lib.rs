#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-program rewrites before register assignment.
//!
//! Rewrites operate on the selected CFG and reconstruct the analyses they
//! invalidate. Register assignment consumes the resulting current program.
//!
//! Start at [`optimize_selected_instructions`] in `selected_optimization.rs`.
//! It stages [`analyses`] first ([`stage_optimized_liveness`], then
//! [`stage_optimized_live_ranges`]) and publishes the identity route when no
//! executed rule is selected. Otherwise it stages allocation legality and runs
//! exactly one executed catalog slice from [`rewrites`]:
//! [`run_selected_lowering_optimizations`] or
//! [`run_pre_allocation_optimizations`]. Either route publishes one
//! [`SelectedInstructionOptimizationOutput`] from
//! `selected_optimization::optimization_output`.

mod analyses;
pub mod register_homes;
mod rewrites;
mod selected_optimization;

// The stage entry and the output it publishes.
pub use selected_optimization::optimization_output::{
    SelectedInstructionOptimizationError, SelectedInstructionOptimizationEvidence,
    SelectedInstructionOptimizationOutput,
};
pub use selected_optimization::optimize_selected_instructions;

// Current selected-program facts and their independent validation.
pub use analyses::{
    AllocationLegalityError, FixedPrecoloredIntervalError, FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSplitRequirementError, LiveRangeError, LivenessError, MachineEffectError,
    MachineEffectStageError, OptimizedAllocationLegalityCustodyError,
    OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError,
    OptimizedSelectedReanalysisError, OwnedSelectedProgram, RecoveryClassificationError,
    SelectedProgramRef, SpillChoiceError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, StagedOptimizedLiveRanges,
    StagedOptimizedLiveness, StagedOptimizedSelectedReanalysis,
    StagedOptimizedSelectedReanalysisCustodyReceipt, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedFixedPrecoloredIntervals,
    ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges, ValidatedLiveness, ValidatedPreAllocationMachineEffects,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
    analyze_allocation_legality, analyze_fixed_precolored_intervals,
    analyze_fixed_precolored_split_requirements, analyze_live_ranges, analyze_live_ranges_reusing,
    analyze_liveness, analyze_liveness_reusing, analyze_machine_effects,
    assign_fixed_precolored_segment_homes, choose_spill_victims, classify_pressure_recovery,
    materialize_allocator_availability, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_allocation_legality_with_availability, stage_optimized_live_ranges,
    stage_optimized_liveness, stage_optimized_selected_reanalysis, validate_allocation_legality,
    validate_fixed_precolored_intervals, validate_fixed_precolored_segment_homes,
    validate_fixed_precolored_split_requirements, validate_live_ranges, validate_liveness,
    validate_machine_effects, validate_optimized_allocation_legality_custody,
    validate_optimized_live_range_custody, validate_optimized_liveness_custody,
    validate_optimized_selected_reanalysis_custody, validate_pre_allocation_machine_effects,
    validate_recovery_classifications, validate_spill_choices,
};
pub(crate) use analyses::{
    AllocationLegalityValidationReceipt, AllocatorAvailabilityError,
    AllocatorAvailabilityValidationReceipt, FixedPrecoloredIntervalValidationReceipt,
    FixedPrecoloredSegmentHomeValidationReceipt, FixedPrecoloredSplitRequirementValidationReceipt,
    LiveRangeValidationReceipt, LivenessValidationReceipt, RecoveryClassificationValidationReceipt,
    SpillChoiceValidationReceipt, StagedOptimizedLiveRangeCustodyReceipt,
    StagedOptimizedLivenessCustodyReceipt,
};
// Selected-CFG rewrites: the executed catalog slices, the recovery rewrites
// register assignment replays, and the unexecuted families.
pub use rewrites::unexecuted;
pub use rewrites::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AddressFoldError, AllocationRecoveryRuleCatalogEntry,
    AllocationRecoveryRuleCatalogError, AllocationRecoveryRuleCatalogPayload, ConstantBooleanError,
    ConstantBranchError, CopyRemovalError, FixedPrecoloredSegmentHomeDecline, FixedViewCopy,
    FixedViewCopyError, FixedViewCopyPlan, FixedViewCopyPolicy, FixedViewCopySourceEvidence,
    LiteralFoldIdentity, LiteralFoldPolicy, OptimizedFixedPrecoloredSegmentHomeCustodyError,
    OptimizedFixedViewCopyCustodyError, OptimizedLiteralFoldCustodyError,
    OptimizedPreAllocationCustodyError, PRE_ALLOCATION_RULE_CATALOG, PairConsumerBindingAdmission,
    PairFaultDischarge, PairImmediateBound, PairLiteralPosition, PairMachineEffects,
    PairNonUnitSurface, PairOperandResult, PairOperandShape, PairResultDisposition,
    PairTailCustody, PairUnitDefRelation, PairUnitEffects, PreAllocationPolicy,
    PreAllocationRuleCatalogEntry, PreAllocationRuleCatalogError, PreAllocationRuleCatalogPayload,
    PreAllocationTransformationIdentity, PressureRematerializationError,
    PressureRematerializationPolicy, RedundantExtensionError,
    RegisterAllocationRuleTargetApplicability, RuntimeRematerializationError, RuntimeSpillError,
    RuntimeSpillSpanPolicy, SELECTED_LOWERING_RULE_CATALOG, SELECTED_STAGE_RULE_CATALOG,
    SelectedInstructionPairRule, SelectedLoweringRuleCatalogEntry,
    SelectedLoweringRuleCatalogError, SelectedLoweringRuleCatalogPayload,
    SelectedStageRuleCatalogSlice, SelectedStageRuleRows,
    StagedOptimizedFixedPrecoloredSegmentHomes, StagedOptimizedFixedViewCopies,
    StagedOptimizedLiteralFoldCustodyReceipt, StagedOptimizedLiteralFolds,
    StagedPreAllocationOptimizationCustodyReceipt, StagedPreAllocationOptimizationRun,
    StagedSelectedLoweringOptimizationCustodyReceipt, StagedSelectedLoweringOptimizationRun,
    ValidatedFixedViewCopies, ValidatedLiteralFold, ValidatedPressureRematerialization,
    ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
    probe_optimized_fixed_precolored_segment_homes, rematerialize_selected_active_resident,
    rematerialize_selected_runtime_value, resolve_pre_allocation_rules,
    resolve_selected_lowering_rules, run_pre_allocation_optimizations,
    run_selected_lowering_optimizations, selected_allocation_recovery_rule,
    spill_selected_runtime_value, spill_selected_runtime_value_with_span_policy,
    stage_first_optimized_literal_fold, stage_next_optimized_literal_fold,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
    validate_fixed_view_copies, validate_optimized_fixed_precolored_segment_home_custody,
    validate_optimized_fixed_view_copy_custody, validate_optimized_literal_fold_custody,
    validate_pre_allocation_optimization_custody, validate_pressure_rematerialization,
    validate_runtime_rematerialization, validate_runtime_spill,
    validate_runtime_spill_with_span_policy, validate_selected_lowering_optimization_custody,
};
pub(crate) use rewrites::{
    FixedViewCopyDestination, FixedViewCopyValidationReceipt, FunctionLiteralFold,
    FunctionPressureRematerialization, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan,
    LiteralFoldValidationReceipt, PressureRematerializationAction, PressureRematerializationPlan,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    StagedOptimizedFixedViewCopyCustodyReceipt, StagedOptimizedLiteralFoldAttempt,
    StagedOptimizedLiteralFoldAttemptReceipt, StagedOptimizedLiteralFoldIterationReceipt,
    StagedOptimizedLiteralFoldStep, StagedOptimizedPreAllocationAttempt,
    StagedOptimizedPreAllocationAttemptReceipt, StagedOptimizedPreAllocationIterationReceipt,
    StagedOptimizedPreAllocationStep, ValidatedAddressFold, ValidatedConstantBoolean,
    ValidatedConstantBranch, ValidatedCopyRemoval, ValidatedPreAllocationTransformation,
    ValidatedRedundantExtension, fixed_view_copy_identity,
};

#[cfg(any(test, feature = "test-support"))]
pub use rewrites::test_support;

// Durable transform-output identities live with the selected-instruction
// representation; the transform publishes them under its root so callers keep
// one vocabulary.
pub use target_operations_to_selected_instructions::{
    AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity, FixedViewCopyIdentity,
    PressureRematerializationIdentity,
};

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.
#[cfg(test)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
