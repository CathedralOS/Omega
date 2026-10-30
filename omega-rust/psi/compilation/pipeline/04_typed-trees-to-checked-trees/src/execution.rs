//! Executable plans over checked facts: the builders that turn a checked
//! program's control, call and ownership facts into the plans a Terminal
//! producer consumes, kept separate from temporal flow analysis.
//!
//! `finalize_execution` completes the initial plans once checking has
//! settled. The four `terminal_*` owners are the builders it calls:
//! `terminal_unit` plans Unit machines (calls, control, state graphs,
//! cleanup), `terminal_scalar` the scalar
//! machine graphs and selections, `terminal_cleanup` the structural control
//! cleanup plans over edges and projections, and `terminal_debug` the debug
//! metadata plans. `guard_complement` is the one judgment both graph
//! builders use to accept a two-guard tail without an authored fallback.
//! `citation_erasure` retires fact-only citation calls before any of them
//! plans, so no planner sequences a theorem that emits no runtime work.
//! `scalar_computations` is this home's other builder: the values pass calls
//! it to produce the checked scalar computation plans that `terminal_unit`
//! and `terminal_scalar` consume, so one plan builder produces scalar and
//! Unit computations.

use checked_trees::{CheckFacts, FlowPermissionEventFact};

pub(crate) mod citation_erasure;
pub(crate) mod content_consumption;
pub(crate) mod finalize_execution;
pub(crate) mod guard_complement;
pub(crate) mod scalar_computations;
pub(super) mod selected_transitions;
pub(crate) mod successor_completion;
pub(crate) mod terminal_cleanup;
pub(crate) mod terminal_debug;
pub(crate) mod terminal_scalar;
pub(crate) mod terminal_unit;

/// Every recorded ownership permission event, in arena order.
///
/// The arena is the one home those events have; each consumer filters by
/// machine, state, source and kind at its own site.
pub(crate) fn permission_events(
    facts: &CheckFacts,
) -> impl Iterator<
    Item = (
        arena::Handle<FlowPermissionEventFact>,
        &FlowPermissionEventFact,
    ),
> {
    facts.flow.ownership.permissions.iter()
}
