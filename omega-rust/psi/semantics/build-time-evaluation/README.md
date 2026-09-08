# Build-time evaluation

This target-neutral service owns the admission floor, checked zero-argument and
fixed-array evaluation, ownership-taking const-generic pre-resolution evaluation,
and machine-backed concrete const-domain fact discharge. Omega schedules the
service; it does not reinterpret its language semantics.

The public ownership-taking pre-resolution and pre-check conveyors keep these
Psi phases separate. Omega may interpose target machine selection, then performs
calling-policy ABI, provider, artifact, and native realization afterward.
Target decisions must not become Psi language elaboration.

The [logical-work specification](../../../../wiki/spec/resources/logical_work.md)
owns budget meaning and the separation between executed work and verifier work.
Per-invocation telemetry is not another interchangeable work currency.

[Package boundaries](../../../../wiki/spec/packages/boundaries.md#admit-before-execution)
requires declaration admission before early evaluation. The
[selection-custody note](../../pipeline/typed-trees-to-checked-trees/authored_selections.md#early-evaluation)
maps that gate to exact calls, candidate sets, and authored application sites.

Closed integer expressions for data applications and root-owned domain families in
concrete data fields, payloads and nongeneric machine type annotations use
[typed expression probes](src/const_generic_expressions.rs) before generic data
synthesis. Nongeneric machine owners also admit named Boolean indices through
the same probe. The temporary probe carries original source/import custody; its
symbols and placeholder layouts never become the published program. Exact
unconstrained builtin integer or Boolean destinations and each integer operator's
builtin meaning are required. Named Boolean constants retain their selected typed literal and
canonical Boolean atom; this does not evaluate Boolean expressions. Declaration
visibility and direct package selection are checked before evaluation.
Existing fixed-integer kernels enforce every node's carrier bounds; anonymous
arithmetic lands once through the shared rational evaluator,
including fractional-intermediate warnings. The canonical result is distinct
from the arena-backed constant and operator occurrences retained at the real use.
Machine parameters, results, locals and casts first resolve in their original
lexical owners, including prior-local frontiers. Every named operand must select
a constant there, and the standalone probe must retain exactly that selection;
runtime parameters and locals cannot become same-spelled constants. Only public
entry signatures publish public-interface occurrences; body and internal-state
annotations retain private implementation exposure. Root scalar constants use
the same resolved substitution as module constants, while legacy aggregate
materialization remains separate.
Open templates, aggregate indices, computed Boolean expressions, authored operator
execution, constrained destinations and address-dependent arithmetic need their
own complete contexts; the standalone probe does not claim those forms.

## Semantic admission boundary

The [semantic-evaluation contract](../../../../wiki/spec/language/evaluation.md)
is broader than the current implementation. [admission.rs](src/admission.rs)
owns the common reachable-closure floor; its
[closure validator](src/admission/closure_validation.rs) currently rejects
authored `requires` anywhere in the reachable machine/callable closure because
pre-check evaluation has no discharged concrete-invocation proof context.
It also rejects declared linear runtime carriers across attached/machine-owned
data, parameters/results, and locals using structural multiplicity, and rejects
recursive call cycles without admitted termination evidence. These are
fail-closed gates, not a prohibition on proof-admissible resources or measured
recursion in the language.

The [result checker](src/admission/const_evaluable.rs) admits complete closed
pure values and rejects escaping references/slices, Text, dynamic/opaque shapes,
interior-mutable and non-copy results, and unresolved generic shapes.
The [interpreter bridge](../checked-interpreter/src/build_time.rs) creates fresh
argument values and snapshots results; its broader transport enum alone does
not establish `ConstEvaluable`. Temporary local mutation and borrows do not
escape through that snapshot. The separate augmenting API returns argument
snapshots for effectful build evaluation and is not hermetic admission.

[Layout/materialization notes](layouts.md) describe the supported geometry and
atomic replay entrances. The full target capsule, target-dependent application
closure, semantic-result/usage cache split, richer resource/trust admission,
and generator expansion remain implementation work. Returning a value or
passing the common floor does not establish those artifacts.

## Evaluator usage

The interpreter's precursor step schedule charges entered states, executed
statements, and evaluated expressions. Its invocation usage is not canonical
Terminal fuel and cannot certify IR fixed work. Build reports retain it beside
configuration, not inside `BuildConfig`, Terminal semantics, or artifact identity.
Usage schema and step-schedule identity are independent.

Successful result accounting counts one cell for each scalar/unit/Text/aggregate
root plus recursive fields, case payloads, and array elements. Text payload
bytes are separate from cell count; type/member names and Rust allocation
overhead are excluded. Augmenting results sum all returned argument snapshots.
Checked arithmetic rejects overflow rather than publishing partial accounting.
Live cells are charged from reservation through the final alias, not estimated
as Rust allocation size. Text backing has a separately charged lifetime.

The [build observation custody note](../../../omega/build/build-evaluation/observation_custody.md)
owns current sponsor ceilings and exact initial/replay reconciliation. The
[replay note](../../../omega/build/build-evaluation/replay.md) owns supported
filesystem sequences. Those bounds and schema-version histories are neither
semantic-evaluation laws nor host CPU/RSS containment guarantees.
