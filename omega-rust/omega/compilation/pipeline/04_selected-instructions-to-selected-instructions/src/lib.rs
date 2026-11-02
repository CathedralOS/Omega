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

#[macro_use]
extern crate selected_instruction_analyses;

// Current selected-program facts and the shared work accounting live in
// the selected-instruction analyses package; this stage stages and rewrites
// over them.
use selected_instruction_analyses as analyses;
use selected_instruction_analyses::work_accounting;

use selected_instruction_analyses::codec_bytes;
use selected_instruction_analyses::cursor;
mod selected_input;
pub use selected_input::ValidatedSelectedAnalysis;
pub use selected_instruction_analyses::SelectedProgramView;
mod rewrites;
mod selected_optimization;

// The stage entry and the output it publishes.
pub use selected_optimization::optimization_output::{
    SelectedInstructionOptimizationError, SelectedInstructionOptimizationEvidence,
    SelectedInstructionOptimizationOutput,
};

pub use selected_instruction_analyses::SelectedOptimizationInput;
pub use selected_optimization::optimize_selected_instructions;

// The refusal every bounded pass in this stage shares.
pub use selected_instruction_analyses::WorkDefect;

// Current selected-program facts and their independent validation.
pub use analyses::{
    AllocationLegalityError, LiveRangeError, LivenessError, MachineEffectStageError,
    OptimizedAllocationLegalityCustodyError, OwnedSelectedProgram, RecoveryClassificationError,
    RecoveryReanalysisError, SelectedProgramRef, SpillChoiceError,
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    StagedOptimizedLiveRanges, UnsealedRecoveryFacts, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPreAllocationMachineEffects, ValidatedRecoveryClassifications, ValidatedSpillChoices,
    analyze_allocation_legality, analyze_allocation_legality_reusing, analyze_live_ranges,
    analyze_live_ranges_reusing, analyze_liveness, analyze_liveness_reusing,
    analyze_machine_effects, choose_spill_victims, classify_pressure_recovery,
    stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
};
pub(crate) use analyses::{
    OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError, stage_optimized_live_ranges,
    stage_optimized_liveness,
};

// Selected-CFG rewrites: the executed catalog slices, the recovery rewrites
// register assignment replays, and the unexecuted families.
pub use rewrites::unexecuted;
pub(crate) use rewrites::{
    AddressFoldError, ConstantBooleanError, ConstantBranchError, CopiedCallOperandError,
    CopyRemovalError, DeadCompareError, EquivalentCompareError, LiteralFoldPolicy,
    PairResultDisposition, PairUnitDefRelation, PreAllocationPolicy,
    PreAllocationTransformationIdentity, RedundantCompareError, RedundantExtensionError,
    SELECTED_STAGE_RULE_CATALOG, SelectedInstructionPairRule, SelectedLoweringRuleCatalogError,
    SelectedStageRuleRows, resolve_pre_allocation_rules, resolve_selected_lowering_rules,
    run_pre_allocation_optimizations, run_selected_lowering_optimizations,
    validate_pre_allocation_optimization_custody, validate_selected_lowering_optimization_custody,
};
pub(crate) use rewrites::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan,
    LiteralFoldValidationReceipt, StagedOptimizedLiteralFoldAttempt,
    StagedOptimizedLiteralFoldAttemptReceipt, StagedOptimizedLiteralFoldIterationReceipt,
    StagedOptimizedLiteralFoldStep, StagedOptimizedPreAllocationAttempt,
    StagedOptimizedPreAllocationAttemptReceipt, StagedOptimizedPreAllocationIterationReceipt,
    StagedOptimizedPreAllocationStep, ValidatedAddressFold, ValidatedConstantBoolean,
    ValidatedConstantBranch, ValidatedCopiedCallOperand, ValidatedCopyRemoval,
    ValidatedDeadCompare, ValidatedEquivalentCompare, ValidatedPreAllocationTransformation,
    ValidatedRedundantCompare, ValidatedRedundantExtension,
};
pub use rewrites::{
    LiteralFoldIdentity, OptimizedLiteralFoldCustodyError, OptimizedPreAllocationCustodyError,
    StagedOptimizedLiteralFoldCustodyReceipt, StagedOptimizedLiteralFolds,
    StagedPreAllocationOptimizationCustodyReceipt, StagedPreAllocationOptimizationRun,
    StagedSelectedLoweringOptimizationCustodyReceipt, StagedSelectedLoweringOptimizationRun,
    ValidatedLiteralFold,
};

// Durable transform-output identities live with the selected-instruction
// representation; the transform publishes them under its root so callers keep
// one vocabulary.
pub use selected_instructions::{
    AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity,
    CopiedCallOperandIdentity, DeadCompareIdentity, EquivalentCompareIdentity,
    FixedViewCopyIdentity, PressureRematerializationIdentity, RedundantCompareIdentity,
};

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.

pub(crate) use rewrites::{
    StoredLoadForwardingError, ValidatedStoredLoadForwarding, forward_selected_stored_load,
};
