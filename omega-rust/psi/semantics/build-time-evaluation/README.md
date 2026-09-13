# Build-time evaluation

This target-neutral service owns the admission floor, checked zero-argument and
fixed-array evaluation, ownership-taking const-generic pre-resolution evaluation,
and machine-backed concrete const-domain fact discharge. Omega schedules the
service; it does not reinterpret its language semantics.

[Range endpoints](src/range_endpoints.rs) evaluate exact resolved machine calls
before type checking. Their [arguments](src/range_endpoints/arguments.rs) must be
closed integer expressions received by exact unconstrained builtin integer
parameters. The shared numeric query establishes context-independent arithmetic;
the scalar constant evaluator retains carrier checks and fractional warnings.
Each argument keeps its own selection admission before the callee's common
floor is checked. Record and case-payload types receive exact call identities
from ordinary declaration-scope resolution before this service runs. Nested
calls, constrained arguments and generic applications remain outside this route.

The public ownership-taking pre-resolution and pre-check conveyors keep these
Psi phases separate. Omega interposes target machine selection and schedules
provider-dependent evaluation after the actual provider plans are selected.
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
synthesis. Nongeneric machine owners also admit Boolean indices through
the same probe, including literal Boolean expressions and fixed-integer
comparisons without named operands. Comparisons between two anonymous numeric
trees use exact rational values through the shared validation evaluator, without
integer truncation, floating rounding or integer-landing warnings. The temporary
probe carries original source/import custody; its symbols and placeholder layouts
never become the published program. Exact unconstrained builtin integer or Boolean
destinations and each integer operator's builtin meaning are required. Named Boolean
constants retain their selected typed literal and canonical Boolean atom. Fixed-width
integer comparisons produce canonical Boolean results using the shared typed order operation, after
each operand lands in its selected carrier. Boolean equality and inequality
compose named Boolean constants and comparison results on the same value path;
every occurrence still requires its selected builtin meaning. Boolean `&&` and `||`
selectively evaluate the right operand under the
[expression schedule](../../../../wiki/spec/language/expressions.md#evaluation-schedule);
unselected operands still retain declaration admission, lexical custody, static
operand types and complete anonymous-rational landing. This static pass never
executes landed arithmetic; valid anonymous landing warnings occur once even
when the containing landed operation is skipped. Declaration visibility
and direct package selection are checked before evaluation.
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
the same resolved substitution as module constants. Aggregate arguments,
including fixed-array destinations, pass the same original-owner lexical check
before the separate legacy materializer can erase their names. Destination shape
only chooses the value evaluator; it cannot authorize a runtime binding.
Module-owned literal integer/Boolean arrays can use the existing structural
index canonicalizer after original lexical selection; they do not need a scalar
probe. Namespace admission validates even unused array declarations before their
initializers disappear. Type-scoped module arrays retain an exact nongeneric
carrier in the declaring module through ordinary authored-selection evidence;
public constants cannot hide a private carrier. Closed nominal record/case and
fixed-array constants use the separate structural canonicalizer, preserving exact
carrier and constructor selection before erasure and rejoining the receiving
parameter afterward. Module-local nongeneric attachments share the ordinary
scope checks; scalar literals also enter the scalar probe. Foreign/generic
attachments, machine-computed nominal aggregate indices, open templates, authored
operator execution, constrained destinations and address-dependent arithmetic
need their own complete contexts;
the standalone probe does not claim those forms.

Scalar `match` indices use the same typed probe and exact numeric evaluator.
Subject evaluation and authored first-match selection precede the selected arm;
later patterns and unselected landed operations do not execute. Every arm still
owes coverage, compatible types, actual anonymous landing obligations, and exact
declaration/operator admission. Selected result edges let anonymous arithmetic
compose across a dispatch without choosing a premature integer width or cloning
the typed program. The result alone determines canonical index identity; the
separate authored-selection roster still includes skipped source occurrences.
Nonzero obligations compose exact rational bounds over all result arms. The
private `value/match_dispatch/rational_bounds.rs` owner retains at most three
closed intervals: negative, positive, and possibly zero. Opposite-sign
alternatives remain separate through arithmetic, so direct dispatch and
surrounding operations use the same proof. The traversal neither executes
subjects nor enumerates independent arm combinations. The rational lattice also
excludes zero when its offset is not an integer multiple of its spacing. In that
case, intersecting a zero-crossing hull with the nearest lattice points restores
zero-free sign intervals, including for subsequent division. A range that still
contains zero leaves the obligation open; it does not prove a zero divisor
actually executes.
Integer destinations additionally require all-arm carrier bounds and an exact
rational lattice (offset plus integer multiples of a stride). Arithmetic and
joins transport that lattice, so fractional intermediates may cancel before
landing; integral interval endpoints alone do not prove integral interior values.
Final declaration destinations and typed peers use the same check. Division by
singleton sign intervals transports and joins the numerator lattice for each
exact divisor, including opposite-sign alternatives produced by arithmetic.
Wider nonconstant divisor intervals still need stronger divisibility evidence;
integral quotient endpoints alone cannot supply it. Nonzero proofs still lose
gaps not captured by the lattice and correlated result facts.

Fractional warnings retain an exact origin and complete final value even inside
skipped operations. With one dispatch-bearing operand per anonymous arithmetic
node, the existing evaluator visits each complete result path using retained
arm edges, without evaluating subjects or patterns. Independent integer-only
dispatches need no warning traversal. Independent fractional histories still
require compositional exact diagnostic evidence and reject explicitly; no arm
Cartesian product, ranged substitute for the final value, or dropped warning
is used to admit them.
Exercise the source customer with
`cargo run -p omega -- --check tests/omega/pass/modules/match_constant_indices/main.omg`
and its canonical-type, selection, and rejection controls with
`cargo nextest run -p compiler --test module_machine_indices value_dispatch:: --no-fail-fast`.
Declaration landing, body/index identity and skipped invalid-arm controls use
`cargo nextest run -p compiler --test module_machine_indices computed_declarations:: --no-fail-fast`.

## Semantic admission boundary

Computed integer/Boolean declarations run through
[initializer evaluation](src/const_initializers.rs) before index normalization.
A non-executing resolution pass selects dependencies without inventing values;
ready dependency layers share the existing typed scalar evaluator. Anonymous
rationals land once, while references preserve their declared carriers. Every
declaration must complete, including unused ones and dependencies mentioned only
in skipped branches. Declaration-owned receipts retain original syntax, exact
selected values and builtin operators through copying and later source extension.
Public index exposure does not republish a selected constant's private
implementation dependencies. Provisional probe values/layouts never supply a
selected dependency or published identity. Long dependency chains still require
one frontend pass per layer; no performance improvement is claimed.

The source acceptance command is
`cargo run -p omega -- --check tests/omega/pass/modules/computed_constant_initializers/main.omg`.
Its checked body/index, module and package controls are
`cargo nextest run -p compiler --test module_machine_indices computed_declarations:: --no-fail-fast`.
Machine-call, aggregate, constrained/target-dependent and floating declaration
evaluation remain separate unfinished obligations; this scalar path does not
establish native aggregate execution or NaN representation identity.

Fixed-array length calls whose reachable closure needs an authored operator
retain their pre-check continuation until Omega supplies selected execution.
Independent length calls still run before Build; a Build invocation that needs
a pending length rejects with that dependency. Deferred root evaluation also
survives a supported generated-source extension. The seeded extension's existing
limits on new computed lengths and wire schemas still apply.

The connected selected path admits primitive Float boundary operations backed
by the exact selected compiler intrinsic. Omega joins the requirement to the
actual provider plan; Psi independently checks the current operator occurrence,
ordered operands, format, and arithmetic policy before using the shared Float
semantics kernel. A matching spelling or scalar signature cannot authorize an
unrelated provider body. Ordinary helper calls retain the same closure admission.

Private fold receipts retain the exact invoked machine, receiving declaration
and type-child path, selected operations, and provider-plan commitment. Final
checking rejoins independently derived operator facts and evaluates the retained
invocation again before accepting its literal length. This establishes custody
within checked source compilation; the portable target capsule and application
closure described below remain separate obligations. Selected ordinary provider
bodies and the earlier const-generic normalization stage still need connected
execution paths.

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
