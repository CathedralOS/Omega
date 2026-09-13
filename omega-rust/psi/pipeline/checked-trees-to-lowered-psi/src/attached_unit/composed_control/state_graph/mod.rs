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
specified in ../../compiler/terminal-production/README.md relative to this crate.
*/

use super::*;
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedStructuralControlSuccessorPlan,
};

mod admission;
pub(super) mod body;
mod case_emission;
mod cases;
mod edges;
mod emission;
mod guarded;
mod parameters;
mod ranking;
mod result_custody;
mod returns;
mod scalars;
mod subslices;
#[cfg(test)]
mod tests;
mod topology;

pub(in crate::attached_unit) use admission::AdmittedGraph;
pub(super) use admission::admit;
pub(super) use admission::has_shared_graph_custody;
use edges::successors;
pub(super) use emission::emit;
