//! One composed Unit control state graph, checked against its authored
//! source and then written as one Terminal machine.
//!
//! The common Unit closure prepares every root and callee through `admit`,
//! which returns the graph records `callable::emit` consumes. There is no
//! composed-only entrypoint or boolean custody selector before this route.
//! Reachability, parameter preparation and aliased claim transport run here
//! once, in dependency order, and failures retain their actual diagnostic.
//!
//! `admission::admit` checks the contract identity and the ranking witness
//! (`ranking::require_witness_ranks`), computes the reachable states
//! (`topology::live`), then checks each state in authored order: its
//! structural parameters (the persistent receiver through `parameters`), its
//! scalar prefix (`scalars`), and its terminator, at the statement ordinal
//! the stage 04 plan records, through `guarded`, `cases`, `returns` or
//! `edges`. The body's effect operations are stage 04's plan and emit as
//! planned; nothing here rejoins them to their authored statements. It then
//! binds invocation claim aliases (`claims::bind_invocation_claims`) and admits the
//! reachable states' calls.
//!
//! `emission::emit` lowers the machine result (`returns::result`) and the
//! entry claims, emits each reachable state through `emission/state.rs`, and
//! ends by retaining state ranks on the machine (`ranking::retain`). A state
//! emits its scalar prefix (`scalars`), prepares closed-case payload bindings
//! (`case_emission`), emits returns and guarded exits (`returns`, `guarded`),
//! and lowers each successor edge in `emission/successor_edge.rs` with its
//! discards (`result_custody`), view windows (`subslices`), copied case
//! leaves (`case_leaf_copy`), scalar arguments (`scalars`) and ranks
//! (`ranking`).
//!
//! Outside that route, `composed_control::scalar_calls` reads
//! `edges::successors` and `scalars::successor_value`, the operation frame
//! reads `case_emission::result`, and `machine_lowering` reads
//! `topology::live` through `attached_unit::unit_graph_live_states`.

/*
We lower the shared checked state graph through the ordinary call catalog. The
historical Unit name now includes normal scalar-sum results: returning a case
must preserve its payload, not force the caller onto a separate graph template.

Preparation and emission have different jobs. admission.rs retains the states,
call boundaries, parameter aliases and claim transport emission.rs needs before
allocating blocks and places. The checked plan already selects this producer;
there is no speculative preparation, source-shape retry or discarded unaliased
claim resolution ahead of the real graph records.

Invocation parameters are immutable, so a backedge to the authored entry needs
a one-shot invocation block followed by an ordinary parameterized state block.
A persistent receiver keeps its invocation place; transferred views and scalar
values use their state-edge bindings. Do not turn self into another per-edge
view or overwrite invocation parameters to make a loop fit.

Local results need not become successor parameters. result_custody.rs rejoins
their exact establishment and authored transfer roster; each ordinary edge
disposes its own remaining affine locals after evaluating successor arguments.
That partition uses existing Terminal edge cleanup, not a global result-drop
flag. Copy payloads retain dominance without acquiring disposal obligations.
*/

/*
A primitive scalar result reuses the ordinary single-state completion rather
than a graph-specific one. A final expression's value is the binding its last
operation establishes in the `Return` role (the operation frame evaluates it),
or an earlier immutable binding a final name reads. Value-only transition exits
run the ordinary exit evaluator (`Evaluation::scalar_control_result`) inside
the state and return its joined value. Either way the state then disposes the
same whole roots a Unit return does.
*/

/*
returns.rs checks the selected nominal case and complete field roster against
the authored return. We evaluate fields once in authored order, then encode them
in declaration order; those orders can differ. The construction place and the
machine result place are distinct. Bounded fields need their declaration-range
proofs even when the surrounding control graph is otherwise admitted.

Ordered value exits use guarded.rs and the ordinary structural-value emitter.
Source-to-execution coverage belongs to tests/omega: the ownership state-graph
fixture composes direct projections, retained local aliases, calls and backedges.
*/
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedStructuralControlSuccessorPlan,
};

// The two steps: `admission` checks the graph against its authored source;
// `emission` writes the admitted graph.
mod admission;
mod emission;

// Whole-graph records: reachable states, claim transport across edges,
// forwarded whole entry-parameter aliases, shared parameters that hold a forwarded
// exclusive loan, and state ranks with their edge comparisons.
mod claims;
mod entry_aliases;
mod exclusive_handoffs;
mod ranking;
mod topology;

// Within one state: the state-local scalar values.
pub(super) mod scalars;

// A state's terminator and successor edges: closed-case dispatch and
// payloads, successor operands, guarded exits, returns, the locals each edge
// disposes, and view windows.
pub(super) mod case_emission;
mod case_leaf_copy;
mod cleanup;
mod edges;
mod guarded;
mod result_custody;
mod returns;
mod subslices;

pub(in crate::unit::attached_unit) use admission::AdmittedGraph;
pub(in crate::unit::attached_unit) use admission::admit;
pub(super) use edges::successors;
pub(super) use emission::emit;
pub(crate) use topology::live;

/// The machine that authored `state`. A fused roster mixes owners — a
/// `self.callee` transition moves control into another machine's authored
/// state — so machine-keyed fact, custody, and statement lookups must resolve
/// each position's owner instead of assuming the composed plan's dispatch
/// machine.
pub(super) fn owner(
    checked: &super::CheckedTrees,
    state: symbols::SymbolHandle,
) -> Result<symbols::SymbolHandle, super::LoweringError> {
    Ok(crate::declarations::state_declaration(checked, state)?
        .0
        .symbol)
}
