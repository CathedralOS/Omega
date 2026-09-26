//! One composed Unit control state graph, checked against its authored
//! source and then written as one Terminal machine.
//!
//! Three entries, all used by the parent `composed_control`:
//! `has_shared_graph_custody` selects this route (called by
//! `lower_composed_unit_control_machine` and `callable::admit`); `admit`
//! checks a `CheckedComposedUnitControlMachinePlan` and returns an
//! `AdmittedGraph` (called by `callable::admit`); `emit` writes the admitted
//! graph (called by `callable::emit`).
//!
//! `admission::admit` checks the contract identity and the ranking witness
//! (`ranking::validate_witness`), computes the reachable states
//! (`topology::live`), then checks each state in authored order: its
//! structural parameters (the persistent receiver through `parameters`), its
//! scalar prefix (`scalars`), its body statements (`body`), and its
//! terminator through `guarded`, `cases`, `returns` or `edges`. It then
//! resolves claim transport across edges (`claims::resolve`) and admits the
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

Admission and emission have different jobs. admission.rs rejoins retained states,
source calls, operands, and successors before emission.rs allocates blocks and
places. has_shared_graph_custody selects this route from retained custody, not
from a convenient topology match. Once selected, a failed rejoin is fatal:
retrying an older shape matcher could ignore the very statement or operand that
made the checked plan invalid.

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
same whole roots a Unit return does. Tests: src/tests/composed_scalar_results.rs.
*/

/*
returns.rs checks the selected nominal case and complete field roster against
the authored return. We evaluate fields once in authored order, then encode them
in declaration order; those orders can differ. The construction place and the
machine result place are distinct. Bounded fields need their declaration-range
proofs even when the surrounding control graph is otherwise admitted.

The source-to-execution examples live at src/tests/byte_write_loop.rs in this
crate: scalar_case_return_preserves_authored_multifield_identity_and_rejects_plan_drift
covers reordered fields and corrupt plans, and
scalar_case_return_multistate_borrowed_view_and_ordinary_call_observe_count
combines returns, loops, and caller-visible mutation. Ordered value exits use
guarded.rs and the ordinary structural-value emitter; their native constructor,
borrowed-getter, and selected-effect coverage lives in
tests/native-differential/tests/scalar_case_results/guarded_returns.rs at the
repository root. Full source coverage is
specified in ../../07_lowered-psi-to-terminal-psi/terminal_production.md relative to this crate.
*/
use typed_trees_to_checked_trees::checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedStructuralControlSuccessorPlan,
};

// The two steps: `admission` checks the graph against its authored source;
// `emission` writes the admitted graph.
mod admission;
mod emission;

// Whole-graph records: reachable states, claim transport across edges, and
// state ranks with their edge comparisons.
mod claims;
mod ranking;
mod topology;

// Within one state: the persistent receiver, the body statements, and the
// state-local scalar values.
mod body;
mod parameters;
pub(super) mod scalars;

// A state's terminator and successor edges: closed-case dispatch and
// payloads, successor operands, guarded exits, returns, the locals each edge
// disposes, and view windows.
pub(super) mod case_emission;
mod case_leaf_copy;
mod cases;
mod edges;
mod guarded;
mod result_custody;
mod returns;
mod subslices;

#[cfg(test)]
mod tests;

pub(in crate::unit::attached_unit) use admission::AdmittedGraph;
pub(super) use admission::admit;
pub(super) use admission::has_shared_graph_custody;
pub(super) use edges::successors;
pub(super) use emission::emit;
pub(crate) use topology::live;
