//! Executable plans over checked facts: the builders that turn a checked
//! program's control, call and ownership facts into the plans a Terminal
//! producer consumes, kept separate from temporal flow analysis.
//!
//! `finalize_execution` completes the initial plans once checking has
//! settled; `selected_execution` rebuilds them after exact provider
//! settlement; `execution_plans` builds the dependent plans without
//! publishing intermediate facts. The four `terminal_*` owners are the
//! builders those entries call: `terminal_unit` plans Unit machines (calls,
//! control, state graphs, cleanup, returns), `terminal_scalar` the scalar
//! machine graphs and selections, `terminal_cleanup` the structural control
//! cleanup plans over edges and projections, and `terminal_debug` the debug
//! metadata plans. `guard_complement` is the one judgment both graph
//! builders use to accept a two-guard tail without an authored fallback.

pub(crate) mod execution_plans;
pub(crate) mod finalize_execution;
pub(crate) mod guard_complement;
pub(crate) mod selected_execution;
pub(crate) mod terminal_cleanup;
pub(crate) mod terminal_debug;
pub(crate) mod terminal_scalar;
pub(crate) mod terminal_unit;
