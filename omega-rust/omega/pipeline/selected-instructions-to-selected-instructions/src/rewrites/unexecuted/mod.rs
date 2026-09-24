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
//! Disposition roster. A family stays only while it proves something no
//! other family here proves. `relocation` derives the crossed window from
//! the CFG itself, so every shape whose traversal set the move preserves —
//! the in-block move, the single `Jump` edge, the block chain, the complete
//! diamond, the bypassed triangle, and each of their run forms — is one
//! admission over a run and a destination, with its own test in
//! `relocation/tests.rs`. The twelve families that enumerated those same
//! windows by hand are deleted rather than staged.
//!
//! `relocation` takes whichever direction has acyclic paths, so the upstream
//! shapes — across the sole edge into a block, out of a join through the
//! diamond or the bypassed triangle that feeds it — are the same admission
//! with the arrival block being the run's own rather than the destination.
//! Blocks on a common cycle reach each other both ways and refuse.
//!
//! What remains is what `relocation` does not admit. Its audit refuses a
//! move whose gained or lost traversal set is nonempty, so the
//! traversal-changing families — `arm_relocation`, `fork_relocation`,
//! `inflow_relocation`, `confluence_relocation` and their run forms, the six
//! that read the `dead_path` audit — carry their own proofs. The
//! `commuting_*` families prove ordering under memory commutation, a
//! different audit, and interchange swaps two runs rather than moving one.
//!
//! `scheduled_relocation` is a second general member-run mechanism, not a
//! per-shape family: it admits the dominance-based sink `relocation` refuses,
//! where a traversal stops executing the run and `dead_path` proves nothing
//! observes what the run wrote, at the cost of a narrower subject — pure
//! register and condition-state work, cross-block only. Neither subsumes the
//! other, so both stay until one module carries both admissions. It was
//! declared under `local_schedule`, the in-block pair interchange, and read
//! nothing from that parent.
//!
//! `interchange` is the other consolidation. Six families divided by
//! granularity — two members, a member against a run, two runs — crossed
//! with a memory rule, where each `commuting_*` name was the same geometry
//! with the roster rule widened from "at most one accounted actor" to "every
//! pair of rows that newly trades order commutes". A member is a run of one,
//! and a window whose trading pairs carry rows on at most one side satisfies
//! the widened rule by having no pair to check, so the widest of the six is
//! the general one: two disjoint contiguous runs of at least one member each
//! exchange places, admitted when every newly trading row pair commutes.
//!
//! - `address_fold` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `arm_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `boundary_boolean` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `boundary_branch` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `commuting_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `interchange` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `commuting_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `confluence_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `confluence_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `constant_boolean` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `constant_branch` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `dead_compare` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `dead_store` — staged, owner row **ALIAS-AWARE-MEMORY**
//! - `fork_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `fork_run_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `inflow_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `load_forwarding` — staged, owner row **ALIAS-AWARE-MEMORY**
//! - `scheduled_relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `peepholes` (`condition_materialization`, `copied_call_operand`,
//!   `projected_access`, `terminator_pair`) — staged, owner row
//!   **DECLARATIVE-PEEPHOLES**
//! - `relocation` — staged, owner row **EXACT-MACHINE-SIMPLIFICATIONS**
//! - `store_motion` — staged, owner row **ALIAS-AWARE-MEMORY**
//!
//! `commuting_accesses`, `condition_state`, `dead_path` and `place_storage`
//! are the shared vocabulary only these families read; `super::block_edges`
//! and `super::window_hazards` are shared with the executed set.

mod address_fold;
mod arm_relocation;
mod boundary_boolean;
mod boundary_branch;
mod commuting_accesses;
mod commuting_relocation;
mod commuting_run_relocation;
mod condition_state;
mod confluence_relocation;
mod confluence_run_relocation;
mod constant_boolean;
mod constant_branch;
mod dead_compare;
mod dead_path;
mod dead_store;
mod fork_relocation;
mod fork_run_relocation;
mod inflow_relocation;
mod interchange;
mod load_forwarding;
pub mod peepholes;
mod place_storage;
mod relocation;
mod scheduled_relocation;
mod store_motion;

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
pub use commuting_relocation::{
    CommutingRelocationError, CommutingRelocationReceipt, ValidatedCommutingRelocation,
    relocate_selected_commuting_member, validate_commuting_relocation,
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
pub use interchange::{
    InterchangeError, InterchangeReceipt, ValidatedInterchange, interchange_selected_runs,
    validate_selected_interchange,
};
pub use load_forwarding::{
    StoredLoadForwardingError, StoredLoadForwardingReceipt, ValidatedStoredLoadForwarding,
    forward_selected_stored_load, validate_stored_load_forwarding,
};
pub use relocation::{
    MemberRunRelocationError, MemberRunRelocationReceipt, ValidatedMemberRunRelocation,
    relocate_selected_member_run, validate_member_run_relocation,
};
pub use scheduled_relocation::{
    ScheduledRelocationError, ScheduledRelocationReceipt, ValidatedScheduledRelocation,
    relocate_scheduled_run, validate_scheduled_relocation,
};
pub use store_motion::{
    StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion,
    sink_selected_store_mutation, validate_store_mutation_motion,
};
