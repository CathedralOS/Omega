//! Optimizer module role: stage group. Selected-CFG rewrites and their replay evidence.

mod address_fold;
mod allocation_recovery;
mod arm_relocation;
mod block_edges;
mod boundary_boolean;
mod boundary_branch;
mod bypass_relocation;
mod bypass_run_relocation;
mod catalog;
mod commuting_accesses;
mod commuting_interchange;
mod commuting_member_run_interchange;
mod commuting_relocation;
mod commuting_run_interchange;
mod commuting_run_relocation;
mod condition_state;
mod confluence_relocation;
mod confluence_run_relocation;
mod constant_boolean;
mod constant_branch;
mod copy_removal;
mod dead_compare;
mod dead_path;
mod dead_store;
mod diamond_relocation;
mod diamond_run_relocation;
mod edge_relocation;
mod edge_run_relocation;
mod fixed_view;
mod fork_relocation;
mod fork_run_relocation;
mod inflow_relocation;
mod join_relocation;
mod literal_folds;
mod load_forwarding;
mod local_relocation;
mod local_schedule;
mod member_run_interchange;
#[cfg(test)]
mod module_catalog;
mod place_storage;
mod predecessor_relocation;
mod predecessor_run_relocation;
mod redundant_extension;
mod relocation;
mod run_interchange;
mod run_relocation;
mod runtime_rematerialization;
mod runtime_spill;
mod selected_lowering;
mod store_motion;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
mod triangle_relocation;
mod window_hazards;

pub use address_fold::{
    AddressFoldError, AddressFoldReceipt, ValidatedAddressFold, fold_selected_address,
    validate_address_fold,
};
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
pub use arm_relocation::{
    ArmRelocationError, ArmRelocationReceipt, ValidatedArmRelocation,
    relocate_selected_instruction_out_of_arm, validate_arm_relocation,
};
pub use boundary_boolean::{
    BoundaryBooleanError, BoundaryBooleanReceipt, ValidatedBoundaryBoolean,
    fold_selected_boundary_boolean, validate_boundary_boolean_fold,
};
pub use boundary_branch::{
    BoundaryBranchError, BoundaryBranchReceipt, ValidatedBoundaryBranch,
    fold_selected_boundary_branch, validate_boundary_branch_fold,
};
pub use bypass_relocation::{
    BypassRelocationError, BypassRelocationReceipt, ValidatedBypassRelocation,
    relocate_selected_instruction_through_bypass, validate_bypass_relocation,
};
pub use bypass_run_relocation::{
    BypassRunRelocationError, BypassRunRelocationReceipt, ValidatedBypassRunRelocation,
    relocate_selected_run_through_bypass, validate_bypass_run_relocation,
};
pub use catalog::{
    SELECTED_STAGE_RULE_CATALOG, SelectedStageRuleCatalogSlice, SelectedStageRuleRows,
    selected_stage_catalog_contains, selected_stage_rule_rows,
};
pub use commuting_interchange::{
    CommutingInterchangeError, CommutingInterchangeReceipt, ValidatedCommutingInterchange,
    interchange_selected_commuting_pair, validate_commuting_interchange,
};
pub use commuting_member_run_interchange::{
    CommutingMemberRunInterchangeError, CommutingMemberRunInterchangeReceipt,
    ValidatedCommutingMemberRunInterchange, interchange_selected_commuting_member_and_run,
    validate_commuting_member_run_interchange,
};
pub use commuting_relocation::{
    CommutingRelocationError, CommutingRelocationReceipt, ValidatedCommutingRelocation,
    relocate_selected_commuting_member, validate_commuting_relocation,
};
pub use commuting_run_interchange::{
    CommutingRunInterchangeError, CommutingRunInterchangeReceipt, ValidatedCommutingRunInterchange,
    interchange_selected_commuting_runs, validate_commuting_run_interchange,
};
pub use commuting_run_relocation::{
    CommutingRunRelocationError, CommutingRunRelocationReceipt, ValidatedCommutingRunRelocation,
    relocate_selected_commuting_run, validate_commuting_run_relocation,
};
pub use confluence_relocation::{
    ConfluenceRelocationError, ConfluenceRelocationReceipt, ValidatedConfluenceRelocation,
    relocate_selected_instruction_into_confluence, validate_confluence_relocation,
};
pub use confluence_run_relocation::{
    ConfluenceRunRelocationError, ConfluenceRunRelocationReceipt, ValidatedConfluenceRunRelocation,
    relocate_selected_run_into_confluence, validate_confluence_run_relocation,
};
pub use constant_boolean::{
    ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean,
    fold_selected_constant_boolean, validate_constant_boolean_fold,
};
pub use constant_branch::{
    ConstantBranchError, ConstantBranchReceipt, ValidatedConstantBranch,
    fold_selected_constant_branch, validate_constant_branch_fold,
};
pub use copy_removal::{
    CopyRemovalError, CopyRemovalReceipt, ValidatedCopyRemoval, remove_selected_copy,
    validate_copy_removal,
};
pub use dead_compare::{
    DeadCompareError, DeadCompareReceipt, EquivalentCompareError, EquivalentCompareReceipt,
    RedundantCompareError, RedundantCompareReceipt, ValidatedDeadCompare,
    ValidatedEquivalentCompare, ValidatedRedundantCompare, remove_dead_compare,
    remove_equivalent_compare, remove_redundant_compare, validate_dead_compare,
    validate_equivalent_compare, validate_redundant_compare,
};
pub use dead_store::{
    DeadStoreEliminationError, DeadStoreEliminationReceipt, ValidatedDeadStoreElimination,
    eliminate_selected_dead_store, validate_dead_store_elimination,
};
pub use diamond_relocation::{
    DiamondRelocationError, DiamondRelocationReceipt, ValidatedDiamondRelocation,
    relocate_selected_instruction_through_diamond, validate_diamond_relocation,
};
pub use diamond_run_relocation::{
    DiamondRunRelocationError, DiamondRunRelocationReceipt, ValidatedDiamondRunRelocation,
    relocate_selected_run_through_diamond, validate_diamond_run_relocation,
};
pub use edge_relocation::{
    EdgeRelocationError, EdgeRelocationReceipt, ValidatedEdgeRelocation,
    relocate_selected_instruction_across_edge, validate_edge_relocation,
};
pub use edge_run_relocation::{
    EdgeRunRelocationError, EdgeRunRelocationReceipt, ValidatedEdgeRunRelocation,
    relocate_selected_run_across_edge, validate_edge_run_relocation,
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
pub use fork_relocation::{
    ForkRelocationError, ForkRelocationReceipt, ValidatedForkRelocation,
    relocate_selected_instruction_into_arm, validate_fork_relocation,
};
pub use fork_run_relocation::{
    ForkRunRelocationError, ForkRunRelocationReceipt, ValidatedForkRunRelocation,
    relocate_selected_run_into_arm, validate_fork_run_relocation,
};
pub use inflow_relocation::{
    InflowRelocationError, InflowRelocationReceipt, ValidatedInflowRelocation,
    relocate_selected_instruction_onto_inflow, validate_inflow_relocation,
};
pub use join_relocation::{
    JoinRelocationError, JoinRelocationReceipt, ValidatedJoinRelocation,
    relocate_selected_instruction_out_of_join, validate_join_relocation,
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
pub use load_forwarding::{
    StoredLoadForwardingError, StoredLoadForwardingReceipt, ValidatedStoredLoadForwarding,
    forward_selected_stored_load, validate_stored_load_forwarding,
};
pub use local_relocation::{
    LocalRelocationError, LocalRelocationReceipt, ValidatedLocalRelocation,
    relocate_selected_instruction, validate_local_relocation,
};
pub use local_schedule::{
    LocalScheduleError, LocalScheduleReceipt, ScheduledRelocationError, ScheduledRelocationReceipt,
    ValidatedLocalSchedule, ValidatedScheduledRelocation, relocate_scheduled_run,
    schedule_selected_pair, validate_local_schedule, validate_scheduled_relocation,
};
pub use member_run_interchange::{
    MemberRunInterchangeError, MemberRunInterchangeReceipt, ValidatedMemberRunInterchange,
    interchange_selected_member_and_run, validate_member_run_interchange,
};
pub use predecessor_relocation::{
    PredecessorRelocationError, PredecessorRelocationReceipt, ValidatedPredecessorRelocation,
    relocate_selected_instruction_into_predecessor, validate_predecessor_relocation,
};
pub use predecessor_run_relocation::{
    PredecessorRunRelocationError, PredecessorRunRelocationReceipt,
    ValidatedPredecessorRunRelocation, relocate_selected_run_into_predecessor,
    validate_predecessor_run_relocation,
};
pub use redundant_extension::{
    RedundantExtensionError, RedundantExtensionReceipt, ValidatedRedundantExtension,
    remove_selected_redundant_extension, validate_redundant_extension_removal,
};
pub use relocation::{
    MemberRunRelocationError, MemberRunRelocationReceipt, ValidatedMemberRunRelocation,
    relocate_selected_member_run, validate_member_run_relocation,
};
pub use run_interchange::{
    RunInterchangeError, RunInterchangeReceipt, ValidatedRunInterchange, interchange_selected_runs,
    validate_run_interchange,
};
pub use run_relocation::{
    RunRelocationError, RunRelocationReceipt, ValidatedRunRelocation, relocate_selected_run,
    validate_run_relocation,
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
pub use store_motion::{
    StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion,
    sink_selected_store_mutation, validate_store_mutation_motion,
};
pub use triangle_relocation::{
    TriangleRelocationError, TriangleRelocationReceipt, ValidatedTriangleRelocation,
    relocate_selected_instruction_out_of_triangle, validate_triangle_relocation,
};

/// Explicit applicability of the currently architecture-independent rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}
