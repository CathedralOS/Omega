//! Optimizer module role: stage group. Rewrite families the stage catalogues
//! but does not execute.
//!
//! Every family here has a proposer, an independent validator and its own
//! tests, and no production route: `optimize_selected_instructions` executes
//! only the slices `super::catalog` admits, and none of these families has a
//! catalog row yet. They stay compiled and tested so the owning board item
//! can give each one a stage-catalog row or delete it; nothing outside this
//! module may call them until that row exists (`super::module_catalog`
//! reconciles the roster below and rejects a production caller).
//!
//! Disposition roster — every family is `staged-with-owner-row`; none is
//! executed, and none is superseded or retired on the board today, so none
//! is deleted:
//!
//! - `address_fold` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `arm_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `boundary_boolean` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `boundary_branch` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `bypass_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `bypass_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `commuting_interchange` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `commuting_member_run_interchange` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `commuting_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `commuting_run_interchange` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `commuting_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `confluence_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `confluence_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `constant_boolean` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `constant_branch` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `dead_compare` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `dead_store` — staged, owner row **ALIAS-AWARE-MEMORY**
//! - `diamond_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `diamond_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `edge_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `edge_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `fork_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `fork_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `inflow_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `join_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `load_forwarding` — staged, owner row **ALIAS-AWARE-MEMORY**
//! - `local_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `local_schedule` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `member_run_interchange` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `peepholes` (`condition_materialization`, `copied_call_operand`,
//!   `projected_access`, `terminator_pair`) — staged, owner row
//!   **DECLARATIVE-PEEPHOLES**
//! - `predecessor_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `predecessor_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `run_interchange` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `store_motion` — staged, owner row **ALIAS-AWARE-MEMORY**
//! - `triangle_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//!
//! `commuting_accesses`, `condition_state`, `dead_path` and `place_storage`
//! are the shared vocabulary only these families read; `super::block_edges`
//! and `super::window_hazards` are shared with the executed set.

mod address_fold;
mod arm_relocation;
mod boundary_boolean;
mod boundary_branch;
mod bypass_relocation;
mod bypass_run_relocation;
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
mod dead_compare;
mod dead_path;
mod dead_store;
mod diamond_relocation;
mod diamond_run_relocation;
mod edge_relocation;
mod edge_run_relocation;
mod fork_relocation;
mod fork_run_relocation;
mod inflow_relocation;
mod join_relocation;
mod load_forwarding;
mod local_relocation;
mod local_schedule;
mod member_run_interchange;
pub mod peepholes;
mod place_storage;
mod predecessor_relocation;
mod predecessor_run_relocation;
mod relocation;
mod run_interchange;
mod run_relocation;
mod store_motion;
mod triangle_relocation;

pub use address_fold::{
    AddressFoldError, AddressFoldReceipt, ValidatedAddressFold, fold_selected_address,
    validate_address_fold,
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
pub use store_motion::{
    StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion,
    sink_selected_store_mutation, validate_store_mutation_motion,
};
pub use triangle_relocation::{
    TriangleRelocationError, TriangleRelocationReceipt, ValidatedTriangleRelocation,
    relocate_selected_instruction_out_of_triangle, validate_triangle_relocation,
};
