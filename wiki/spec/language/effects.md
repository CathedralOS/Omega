# Service reach and operational ceilings

A machine's service reach, direct synchronous invocations, possible suspension,
worker blocking, and crashes are independent contract axes. Authority is held
in [values](../resources/authority.md); implementation trust is recorded by
[provider admission](../build/provider_selection.md). Neither is an effect row.

## Declarations

| Clause | Meaning |
| --- | --- |
| `reaches A + B;` | May reach the named boundary services and their boundary parents. |
| `invokes handler;` | May synchronously enter that binding before returning. |
| `suspends;` | May park the current activation. |
| `blocks;` | May occupy its worker while waiting. |
| `crashes Cause` with route guards | May leave without a successor or cleanup under a covered guard. |

Boundary traits contribute nominal service identities automatically; ordinary
traits do not. A call contributes its boundary service even if that operation
authors no additional `reaches` members. A wake operation may reach a scheduler
without suspending. Mutation through ordinary borrows adds no service identity;
its access and write frame follow [structural access](../terminal-psi/structural_access.md).

Exported machines, trait requirements, boundary operations, and explicit top-level
boundary requirements publish their ceilings. Omitted reach is empty; omitted
`suspends` means never parks; omitted `blocks` means never blocks a worker;
an omitted crash cause is forbidden. Private checked bodies may omit clauses
to request inference. An authored memberless `reaches` on a private body is an
explicit empty ceiling, not inference.

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
inferred body reach    subset-of declared reach ceiling
callee reach          subset-of caller reach ceiling
provider reach        subset-of requirement reach ceiling

body/callee/provider may_suspend implies corresponding ceiling permits suspension
body/callee/provider may_block   implies corresponding ceiling permits blocking
```

Private inference computes conservative fixed points over recursive checked-call
components. Service union and suspension/blocking booleans remain separate.
Local checked calls may use checked summaries. Imported, generic, dynamic, and
boundary calls use pinned requirement ceilings, not the narrower implementation
eventually selected for them. Dynamic values retain per-requirement envelopes.
Every known checked helper on the path propagates the corresponding obligations.

The language has no service subtraction, negation, masking, scoped allowance, or
algebraic-effect handlers. A checked in-memory provider for `Readable` may remove
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

Deterministic normalizers own published service rows, invocation contracts,
suspension/blocking ceilings, and crash buckets. Proof may discharge legality or
enable optimization; it cannot shrink an authored export, rewrite its guards,
or change interface identity. Stable syntactic/control-flow normalization is
not heuristic entailment. Exact authored member/keyword spans explain review;
inferred rows and parent closure receive no invented source locations.

An installation-bound requirement uses `reaches <= Bound` for its abstract
service row. Its exact requirement path owns the row. The selected provider's
row must fit the finite bound and replace it throughout the owning installation
closure before admission. Ordinary callable package/component interfaces must
bind the provider or publish a fixed conservative row. No ordinary exported
row variables, Boolean row formulas, or lower-bound constraints are implied.
The independent bounded `reaches _;` inside a
[transparent refinement](conformances.md#transparent-refinements) constrains its
base requirement; it does not create another installation-row declaration form.
[Installed roots](../build/external_roots.md#installation-bound-reach) owns
manifest and lineage requirements; equal rows never establish protocol identity.
