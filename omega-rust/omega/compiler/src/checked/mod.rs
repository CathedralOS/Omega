//! Compiler orchestration from assembled syntax to one sealed checked compilation.
//!
//! Start at `checking.rs`. This compiler operation takes the assembled forest, evaluates
//! its build machine, resolves and types the source that build generated
//! against the retained base, settles providers, dispatch, entry bindings and
//! task activations, admits package declarations, checks, and seals the
//! result with its source custody. The `checking` folder holds the build
//! continuation, direct calls into the Psi stages, execution
//! settlement; `compilation_products` owns the sealed carrier and its projections.
//! The checked product settles trust through the realization trust owner;
//! Build evaluation owns declaration admission before any Build effects.
//! Checked-product selection executes in Psi stage 04. Exact-rule rollback
//! settlement is owned by `optimization_core`.

mod checking;

pub(crate) use checking::build_continuation::{BuiltCheckedProgram, evaluate_build_and_continue};
pub use checking::compile_thread::run_on_compile_thread;
pub(crate) use checking::execution_settlement::check_selected_execution;
pub use checking::{
    CheckedCompilation, CheckedCompileOutcome, CheckedCompileRequest,
    IndependentComponentDiscovery, IndependentComponentSelection, PreparedCheckedSource,
    RestrictedBuildGrants, checked_compilation_call_count, compile_to_checked,
    executed_preliminary_check_count, executed_settled_check_count, preliminary_check_totals,
    settled_check_totals, source_assembly_invocation_count, typed_frontier_reuse_count,
};
pub use optimization_core::{
    OptimizationRollback, OptimizationRollbackInputError, OptimizationRollbackSettlement,
};
