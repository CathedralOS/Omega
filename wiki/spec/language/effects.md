# Service reach and operational ceilings

A machine's service reach, direct synchronous invocations, possible suspension,
worker blocking, and crashes are independent contract axes. Authority is held
in [values](../resources/authority.md); implementation trust is recorded by
[provider admission](../build/provider_selection.md). Neither is an effect row.

## Declarations

| Clause | Meaning |
| --- | --- |
| `reaches A + B;` | Declares reach of the named boundary services and their boundary parents. |
| `invokes handler;` | May synchronously enter that binding before returning. |
| `suspends;` | May park the current activation. |
| `blocks;` | May occupy its worker while waiting. |
| `crashes Cause` with route guards | May leave without a successor or cleanup under a covered guard. |

Boundary traits contribute nominal service identities automatically; ordinary
traits do not. A call contributes its boundary service even if that operation
authors no additional `reaches` members. A wake operation may reach a scheduler
without suspending. Mutation through ordinary borrows adds no service identity;
its access and write frame follow [structural access](../terminal-psi/structural_access.md).

A checked body directly invoking a boundary operation must declare the reached
service in `reaches`, including for a receiver-free static boundary call. Ordinary
machine calls propagate reach automatically: a wrapper need not repeat its
callees' declarations. This applies to private and exported checked bodies alike.
Omitting `reaches` does not promise empty transitive reach or permit undeclared
direct boundary use. Authored members remain conservative contributions to the
published row; they cannot mask additional transitive reach. A memberless clause
contributes no members and is not a prohibition on callees' reach.

Bodyless trait requirements and opaque boundaries publish declared reach ceilings;
omission there is empty. A selected implementation must fit its requirement.
Reach declarations and propagation grant no capability or provider authority.
Reach prohibitions, including negation or Boolean restriction syntax, are outside
this contract; `reaches` does not double as a no-effects assertion.

Suspension, blocking, and crashes retain their independent rules. On published
surfaces omitted `suspends` means never parks, omitted `blocks` means never blocks
a worker, and an omitted crash cause is forbidden. Private checked bodies may
omit these operational clauses to request inference.

May-ceilings permit behavior; they do not assert that every run performs it.
[`terminates`](termination.md) has the opposite polarity: a positive progress promise under its
pinned premises, not an effect. It does not by itself prove fairness, eventual
wakeup, a deadline, or starvation freedom. Omitting suspension/blocking proves
only the corresponding negative guarantee, not positive progress.
Recoverable failure and cancellation are result
sums, not `fails` clauses. Resource capacity follows explicit capabilities and
dependent contracts, not a `budget` or quantitative service member.

## Process-exit reach

The canonical core [ProcessExit boundary](process_exit.md) contributes ordinary
`reaches ProcessExit` demand. Calls and providers propagate it through the same
conservative ceilings as other services. The ceiling permits exit; actual
domain-bound capability authority is still required. Console I/O alone grants
neither, and importing core or selecting a provider creates no authority.

The exact exit requirement defines a non-crashing terminal transfer with no
normal result or successor. A helper that may exit retains its normal
continuation when it returns. Reach alone supplies neither a guaranteed exit
nor a guarded guarantee of return. `terminates` independently excludes
divergence under its premises; it does not authorize an endpoint. No completion
keyword or second effect row is introduced.

## Recoverable outcomes and strict use

Recoverable failure is an ordinary result sum, not a second control-flow system
or dedicated error type. Exhaustive transitions handle its cases, and a case
payload and its guarantees are available only on the corresponding arm.
Across calls, ordinary `requires`/`ensures` mediate those facts. Host boundaries
use the same result model and state which resources remain valid on each outcome;
result-by-out-parameter is an ABI detail, not an alternate source result form.

A non-Unit call result cannot be silently discarded. Intentional discard is
`_ = call();`, subject to the result's ordinary ownership and legal-disposition
obligations; the marker cannot erase linear custody. Failure propagation is an
explicit checked edge returning the caller's outcome. There is no implicit
propagation operator `?` or `fails` clause.

An unproved obligation rejects; prover incompleteness neither constructs an
error result nor authorizes a trap or abort. A checked failure-returning operation
or explicitly selected Trapping operation has its own contract and admission.
There is no implicit `expect`/`unwrap` escape from proof obligations. A failure
case proved unreachable needs no executable handler, under ordinary exhaustive
transition checking.

## Normalization and composition

Resolve each authored service occurrence to its exact boundary-trait identity.
The semantic row is deterministic idempotent set union plus boundary-parent
closure and services contributed by direct invocation:

```text
R + R = R
R + empty = R
parents(R) subset-of normalize(R)
```

If `Filesystem` inherits `Readable`, reaching `Filesystem` also reaches
`Readable`. Duplicate source occurrences retain their provenance but add no
duplicate semantic member. There is no ranking or name-based service discovery.

For a checked body, a callee, and a provider satisfying a pinned requirement:

```text
published body reach  = normalize(authored reach + contributions from calls)
provider reach        subset-of requirement reach ceiling

body/callee/provider may_suspend implies corresponding ceiling permits suspension
body/callee/provider may_block   implies corresponding ceiling permits blocking
```

Reach derivation computes conservative fixed points over recursive checked-call
components. Known private helpers are included transitively. Imported checked
machines supply their published summaries, without requiring access to private
source. Dynamic and boundary calls retain pinned requirement ceilings, not a
narrower provider body eventually selected for them. Static generic calls follow
the dependency rule below. Service union and suspension/blocking booleans remain
separate; their operational acknowledgements still use the requirement envelope.
Every known checked helper on the path propagates the corresponding obligations.

### Selected-product exclusions

[Build-level behavior exclusions](../build/behavior_exclusions.md) add an
independent admission requirement over the complete selected product. Verified
implementation evidence may establish that a composition cannot exhibit behavior
permitted by its published callable ceilings. This does not shrink those ceilings,
alter generic body checking, or make a broad callable satisfy a narrower requirement.
Ordinary calls continue to use the contract and dependency rules above.

A product-level absence claim must retain exact selections, complete entry/call
coverage and independently checked evidence. Opaque or open calls retain their
contract conservatively unless stronger applicable evidence exists. No optional
optimization, provider choice alone, or caller permission can rewrite a public
summary. An actual boundary invocation retains its abstract service identity even
when a silent realization performs no physical I/O.

## Static callback reach dependencies

A call through a nominal static machine binder contributes that binder's reach
dependency automatically. Its named trait requirement bounds the permissible
selection; there is no extra authored bound or `reaches Step.reaches` clause.
Structural callable binders retain their fixed requirement contracts; this rule
does not remove them or introduce structural row-parameter syntax.

Published dependency summaries contain only finite unions of concrete service
identities and nominal binder row variables. Each variable is bounded above by
its requirement's declared row. No subtraction, lower bounds, Boolean formulas,
or arbitrary author-defined row expressions are introduced. A generic body is
checked against every permitted selection, not only observed instantiations.

The compiler derives the dependency structure from the checked call graph. Each
closed application substitutes the selected machines' published contracts into
that structure. It does not inspect a selected opaque body to narrow its promise.
For example, a body calling only `Step` has dependency `reach(Step)`; one also
calling a Console-reaching helper has `reach(Step) + Console`. These expressions
describe retained semantic data, not new source syntax. Generic wrappers compose
these dependencies through further ordinary calls and exact substitutions.

Use the ordinary deterministic service normalizer for dependency summaries:
union is associative, commutative, and idempotent, empty is its identity, and
boundary-parent closure applies. Binder identity is structural, not its spelling.
Call reordering or extraction into a private helper with the same net reach must
not change the normalized dependency. Do not prune authored call contributions
using optimizer reachability, proof heuristics, or the current consumer set.

The normalized dependency is part of the exported interface. Retain its exact
requirement identities, substitutions, and selected contract dependencies for
independent checking. A private-helper edit adding service reach can therefore
change an exported interface; this is an accepted consequence of propagation.
Changed interface identity requires ordinary revalidation, not an automatic
package-version bump or rejection of every caller. A requirement or evaluation
context that cannot admit the new reach rejects; unconstrained callers propagate
it onward. Stale retained summaries cannot authorize the changed application.

The application's specialized row is used by ordinary callers as well as
evaluation admission. A no-reach callback can leave a traversal with empty reach;
a Console-reaching callback contributes Console. Empty reach alone establishes
none of the other evaluation obligations.

Suspension and blocking do not acquire binder projections or conditional markers.
They remain fixed by the callback requirement: calls use its exact acknowledgement
envelope and, if it permits suspension, the restricted syntactic positions for
all selections. Preconditions, postconditions, crashes, termination, and resource
composition remain independently checked; no whole-contract forwarding exists.

Callable effect rows have no service subtraction, negation, masking, scoped
allowance, or algebraic-effect handlers. Independent product exclusions do not
add those operations. A checked in-memory provider for `Readable` may remove
opaque trust expenditure but cannot erase the abstract service from callers.
Ordinary traits and wrappers cannot hide reach, blocking, or suspension.
Possessing authority cannot bypass a published ceiling, and listing a service
cannot create its required capability.

## Direct synchronous invocation

`invokes` retains direct edges that transitive reach intentionally forgets.
Parameter paths distinguish separate bindings satisfying the same trait. A trait
identity may name an internally selected binding when no parameter path exists.
Bodyful machines infer these edges, including forwarding through local helpers;
exported implementations check them against their published ceiling. Bodyless
omission permits no synchronous entry.

An invoked handler contributes its boundary identity and selected concrete
operational envelope to the current invocation. Durable registration instead
establishes a future external root; it adds no synchronous edge or handler reach
to registration unless registration also invokes that handler. Root admission
separately checks the concrete envelope. The realized synchronous graph across
component boundaries must be acyclic. A new activation, mailbox, or scheduler
handoff breaks an edge; an extra synchronous wrapper does not. See
[callback registration](../build/private_callbacks.md#registration-and-lifetime).

## Call-site acknowledgements

```omega
ordinary_call();
suspend may_park();
block may_wait();
suspend block may_do_either();
```

The marker set must equal the statically known call envelope: `suspend` exactly
when suspension is possible, `block` exactly when blocking is possible. Missing,
partial, or redundant markers reject. Combined order is `suspend block`.
A pinned transparent refinement removing an operational possibility removes its
marker; observing a non-waiting run does not.

Markers acknowledge held borrows, claims, guards, and authority across possible
pauses. They do not force execution to pause, create a task/future, change ABI or
result, change inferred effects, or enter normalized machine identity. Suspension
parks an activation and requires a continuation; blocking retains its stack and
worker. Carry policy may reject a crossing with particular live values but
cannot rewrite the callee's envelope.

A suspending call may occur only as a complete statement, simple `let`
right-hand side, transition subject, or terminal expression. It cannot nest
inside an argument, operator, aggregate construction, or condition. A
blocking-only call may nest because it creates no continuation boundary.

Authored generated code and adapters obey these rules. Compiler-synthesized
adapters retain diagnostic/audit acknowledgement metadata without invented
source tokens. A marker cannot bypass an execution context's operational floor:
automatic [cleanup](../../language_guide/chapter_17_drops_and_cleanup.md#cleanup-machines)
and [hermetic semantic evaluation](evaluation.md#invocation-admission) forbid
source suspension and blocking.
Interpreter budget suspension is not a source `suspends` permission.

Starting a task is separate: `runtime.start<M>` acknowledges only what `start`
may do to the current activation, not what `M` may do in its new activation.
See [Terminal suspension](../terminal-psi/calls_and_outcomes.md#suspension).

## Guarded crashes

Each `crashes` clause names one cause (`Trap` or `Abort`); its route guards are
alternatives, and an empty route list means `true`. Guards are total proof
expressions. Trapping runtime arithmetic cannot execute inside a guard to create
a route. Fixed machine integers/addresses use proof-integer embedding; floats
use `FloatMeaning`. The body operation creates the crash site under its
compiler-defined denotation.

For each derived site guard `D`, the same-cause published guards `C_i` must satisfy
`D implies OR_i(C_i)`. Across calls, substitute actuals, conjoin current path
facts, and discard disproved routes. Disproving every route removes that cause
at this invocation without changing the published contract. In contrast,
suspension/blocking ceilings are not narrowed by one non-waiting path.

Only a body inside the same fingerprinted verification unit may supply a local
body summary; otherwise use the published ceiling and certificate. A physical
check may remain after its semantic route is disproved unless valid specialization
removes it. Crash-frontier lower bounds report definitely live obligations, not
safe survivors or complete external custody. [Crash semantics](../terminal-psi/calls_and_outcomes.md#crash)
owns verification and no-cleanup behavior; restart needs separate containment
and recovery evidence. Process-exit reach and an `Abort` route are independent.

### Recovery and execution domains

A crash clause proves permitted routes, not recovery. Its local frontier is only
a lower bound: caller frames, suspended activations, external storage, devices
and peers may have affected state outside it. An uncontained crash terminates
the complete execution domain; there is no ambient lock-poisoning, survivor or
resumption guarantee.

Continuation needs independent structure: a closed-custody component isolating
all invalidated mutable state, a resource's explicit owner-death/recovery
protocol, or external reset/reconciliation/transactional guarantees. The target
must realize the isolation and restart plan. Restart establishes a fresh
activation, never resumes abandoned computation. Component replacement uses
cooperative drain, coexistence or migration, not hidden asynchronous destruction.

Crashes have explicit no-successor abandonment, not a missing cleanup list.
Recoverable outcomes follow ordinary cleanup-bearing edges. There is no implicit
unwind; any future unwinding or resumable-fault protocol requires explicit graph
edges, cleanup and proof obligations. Graceful shutdown remains ordinary cleanup
followed by its selected exit operation.

The same survivor-contract requirements apply to non-crashing
[process-exit abandonment](process_exit.md#abandonment-and-survivors).
Permission to end a domain cannot waive safety guarantees relied upon by
surviving components. Process exit is a distinct outcome, not an `Abort` route.

## Published identity and installation rows

Deterministic normalizers own published service rows and reach dependencies,
invocation contracts, suspension/blocking ceilings, and crash buckets. Proof may
discharge legality or enable optimization; it cannot shrink an authored export, rewrite its guards,
or change interface identity. Specializing a published reach dependency is
contract substitution, not proof-based shrinking of an authored opaque ceiling.
Stable syntactic/control-flow normalization is
not heuristic entailment. Exact authored member/keyword spans explain review;
inferred rows and parent closure receive no invented source locations.

An installation-bound requirement uses `reaches <= Bound` for its abstract
service row. Its exact requirement path owns the row. The selected provider's
row must fit the finite bound and replace it throughout the owning installation
closure before admission. This installation rule is distinct from ordinary
[static callback reach dependencies](#static-callback-reach-dependencies).
Only the bounded union dependencies specified there are admitted on ordinary
exports; no general row algebra or lower-bound constraints are implied.
The independent bounded `reaches _;` inside a
[transparent refinement](conformances.md#transparent-refinements) constrains its
base requirement; it does not create another installation-row declaration form.
[Installed roots](../build/external_roots.md#installation-bound-reach) owns
manifest and lineage requirements; equal rows never establish protocol identity.

Automatic reach propagation does not introduce general callback-contract
projections or conditional call acknowledgements. Named callbacks retain fixed
suspension/blocking requirement envelopes.
No anonymous-machine or lambda surface is accepted.
