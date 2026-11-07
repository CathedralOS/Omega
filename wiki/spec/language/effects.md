# Service reach and operational ceilings

A machine's service reach, direct synchronous invocations, possible suspension,
worker blocking, and crashes are independent contract axes. Authority is held
in [values](../resources/authority.md); implementation trust is recorded by
[provider admission](../build/provider_selection.md). Neither is an effect row.

## Declarations

| Clause | Meaning |
| --- | --- |
| `reaches A + B` | Bounds service reach by the named boundary services and their boundary parents. |
| `reaches` with no members | Explicitly bounds service reach by the empty set. |
| `invokes handler` | May synchronously enter that binding before returning. |
| `suspends` | May park the current activation. |
| `blocks` | May occupy its worker while waiting. |
| `crashes Cause` with route guards | May leave without a successor or cleanup under a covered guard. |

Boundary traits supply identities that may be named in `reaches`; declaring or
calling one does not implicitly add its identity. A boundary call contributes
its requirement's explicit reach ceiling, with boundary-parent closure. Every
boundary-trait machine requirement must declare that ceiling, using bare `reaches`
when it permits no services. A memberless service-identity trait has no operation
requiring an annotation. Ordinary traits are not service
identities. A wake operation may reach a scheduler without suspending.
Mutation through ordinary borrows adds no service identity;
its access and write frame follow [structural access](../terminal-psi/structural_access.md).

Named decision: `optional-reach-ceilings`, ratified 2026-10-22. A checked body
may omit `reaches`, including when it directly calls a boundary operation. Its
reach is then inferred from every checked call and pinned requirement contract.
This applies equally to private helpers, exported library APIs, named
implementation members, and an installed application entry. `main` has no
annotation exemption because no checked body needs one.

A present clause is an upper bound, not an assertion of exact equality and not
an additive contribution. The complete derived reach must fit its normalized
ceiling. `reaches A + B` permits a body reaching only `A`; bare `reaches` rejects
any nonempty derived service row. Omission requests inference and is distinct
from an explicit empty clause. Reach does not count local mutation as a service,
and empty reach promises neither termination nor absence of blocking or crashes.

All machine clauses have no clause-ending semicolon. A following clause or body
delimits the current clause. A bodyless declaration still has its final
declaration semicolon: `machine call() reaches suspends blocks;` ends one complete
declaration with three clauses. A checked body writes `reaches suspends blocks`
followed directly by `{ ... }`. Termination is not an exception. Nested callable
requirements use the same [header delimiters](state_contracts.md#machine-header-delimiters);
do not insert semicolons between their clauses.

Boundary requirements publish their explicit ceilings. Opaque supplies inherit
the governing requirement's admitted contract, not an inferred native-body
summary; a realizer need not repeat that same clause. A bodyless non-boundary
requirement still has no body from which to infer reach; omission there is empty.
A selected checked or opaque implementation must fit its requirement even when
its own optional clause is absent. Neither inference nor an empty row lets a
provider hide its calls.
Foreign and assumed supplies retain their separate trust admission and physical
authority classifications. Pure arithmetic may have a selected boundary
implementation without acquiring an arithmetic-service reach.
Reach declarations and propagation grant no capability or provider authority.
Negation and Boolean row-restriction syntax are outside this contract. An empty
ceiling is the ordinary zero-member bound, not a separate purity or authority
mechanism. The compiler must publish a checked reach summary whether or not the
author wrote a clause. An explicit published ceiling may conservatively remain
wider than its checked body; review retains the declared ceiling and derived
reach separately. Manifest admission and `Build::audit_service` policy consume
those checked records independently of whether the author annotated the body.

Mandatory direct-call annotations and additive checked-body rows are
superseded behavior, not an alternate language mode.

### Implementation boundaries and capability identities

An OS-specific provider interface is not itself a mandatory service effect.
For example, Windows and POSIX read requirements can both explicitly reach
the core-owned `FilesystemContentRead` identity; writes reach
`FilesystemContentWrite`. Their family interface names enter the row only if
explicitly named, never merely because provider selection uses those traits.
Ordinary portable helpers propagate the declared capability rows.

A package can define its own boundary-trait identity and explicitly name it
in an operation's row, including the containing trait itself:

```omega
boundary trait MotorControl {
    machine set_speed(speed: u32)
        reaches MotorControl;
}
```

A boundary trait used only as a service identity need not have callable
requirements. Naming it in a row creates neither a provider-selection demand
nor a `Binding` value. Actual boundary calls still need their own selected
implementation and applicable authority. All service identities propagate by
the same rules, regardless of whether
[package audit policy](../packages/review.md#critical-service-audit-policy)
marks them critical. Audit registration changes presentation and acceptance,
not reach, inheritance, authority or attenuation.

Suspension, blocking, and crashes retain their independent rules. On published
surfaces omitted `suspends` means never parks, omitted `blocks` means never blocks
a worker, and an omitted crash cause is forbidden. Private checked bodies may
omit these operational clauses to request inference. A public generic that
admits a suspending or blocking callback declares the corresponding ceiling.
Its published [operational dependency summary](#static-callback-operational-dependencies)
separately records which static selections contribute those possibilities.

May-ceilings permit behavior; they do not assert that every run performs it.
[`terminates`](termination.md) has the opposite polarity: a positive progress promise under its
pinned premises, not an effect. It does not by itself prove fairness, eventual
wakeup, a deadline, or starvation freedom. Omitting suspension/blocking proves
only the corresponding negative guarantee, not positive progress.
Recoverable failure and cancellation are result
sums, not `fails` clauses. Resource capacity follows explicit capabilities and
dependent contracts, not a `budget` or quantitative service member.

## Process-exit reach

The canonical core [ProcessExit requirement](process_exit.md) explicitly declares
`reaches ProcessExit`. Calls and providers propagate it through the same
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
derived body reach     = normalize(contributions from calls)
derived body reach     subset-of authored ceiling, when a clause is present
published reach        = authored ceiling, when a clause is present
published reach        = derived body reach, otherwise
provider reach         subset-of requirement reach ceiling

body/callee/provider may_suspend implies corresponding ceiling permits suspension
body/callee/provider may_block   implies corresponding ceiling permits blocking
```

Reach derivation computes conservative fixed points over recursive checked-call
components. Known private helpers are included transitively. Imported checked
machines supply their published summaries, without requiring access to private
source. Boundary calls and dynamic calls without retained narrower evidence use
their pinned requirement ceilings, not a narrower provider body eventually
selected by Build or at run time. A checked machine-value construction instead retains
the selected implementation's available checked contract as described below.
Static generic calls follow the dependency rule below. Service union and
suspension/blocking booleans remain separate; operational acknowledgements use
the applicable retained call envelope.
Every known checked helper on the path propagates the corresponding obligations.

### Known dynamic selections

Named decision: `known-dynamic-contract-narrowing`, ratified 2026-11-02.

When checking establishes which machine a callable value selects, its calls
use that machine's retained checked operational contract rather than the
callable type's potentially wider bound. A known in-memory reader can contribute
no filesystem reach and no suspension even when the callable contract permits
both. Construction,
forwarding and storage do not by themselves discard that established evidence.
Joins retain all possible demands and only guarantees justified for every possible
selection; an unknown alternative contributes the requirement ceiling.

Narrowing uses an available checked contract, not private-source inspection or
an optimization's prediction. Public and explicitly annotated callable ceilings
remain published bounds; a smaller implementation body does not silently
rewrite one. Substituting a published static-callback dependency is ordinary
contract application, not inspection-based shrinking of that ceiling. A reusable
body accepting an unknown machine value must remain valid
for every fitting selection. It may publish checked dependencies on the input's
envelope, but cannot assume the harmless selection used by one current caller.
A narrower authored requirement or retained checked specialization/dependency
justifies a narrower application, not the observed set of callers.

This follows [runtime callable envelopes](../terminal-psi/dynamic_dispatch.md#source-eligibility-and-operational-envelopes).
It changes neither boundary requirements selected by Build nor their admission,
and it creates no runtime authority. Missing selection evidence falls back to
the governing requirement contract, not to a guessed implementation.

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
summary. An actual boundary invocation retains its explicit abstract reach row
even when a silent realization performs no physical I/O. Its requirement identity
remains separately available for provider selection and audit, even with empty reach.

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

### Static callback operational dependencies

Named decision: `static-callback-operational-dependencies`, ratified 2026-11-06.

A checked generic calling a nominal or structural static callback publishes
suspension and blocking dependencies. Nominal callback reach retains the union
rule above; structural callable reach retains its declared bound. Each
operational axis is a finite Boolean OR of a fixed contribution and exact
callback-binder projections,
bounded by the respective requirement permissions. The compiler derives this
summary from checked calls; there is no authored effect variable or new marker.
Forwarded [callback requirements](generics.md#forwarded-callback-requirements)
retain the same exact origins and substitutions.

A public generic declares `suspends` and/or `blocks` whenever its body under
the callback requirements' upper bounds needs them. Those clauses authorize
possibilities; the published dependency summary determines which are possible
for each application. An omitted public clause still forbids that behavior
for every admitted selection. Private checked bodies may infer their ceilings.

Each application substitutes its selected callbacks' available published
contracts into the summary before caller admission. A callback that cannot
park contributes false to suspension. An independent potentially suspending
call contributes true regardless of that callback. Blocking follows the same
rule independently. Unknown/opaque selections use their requirement ceilings;
a callback's observed behavior, private body or predicted optimizer target
cannot supply a narrower contract. A fixed published ceiling without an
applicable checked dependency remains conservative.

Check the generic body once against the complete callback requirements:
its authored `suspend`/`block` markers acknowledge their upper bounds and its
suspending calls occupy the permitted syntactic positions. This also applies
to forwarding calls with unresolved static selections. Do not re-reject those
markers as redundant after substitution removes a possibility. At a caller
with known selections, markers must include the specialized envelope; extra
acknowledgements remain permitted.
The template acknowledgement and the instantiated operational envelope are
retained separately through checking and lowering.

Carry, loan, invariant-window and outcome checks inside the generic body also
use those upper bounds. A synchronous application cannot repair an invalid
generic body. Ordinary loans may cross a permitted suspension when their
preservation obligations hold; forbidden live resources or unclosed windows
still reject. This decision introduces no conditional carry obligations.
Use a narrower originating callback contract when the body needs that guarantee.

```omega
public machine apply<machine F>(value: u32) -> u32
requires machine F(value: u32) -> u32
    suspends;
suspends
{
    suspend F(value)
}

public machine local(value: u32) -> u32 {
    value
}

public machine example() -> u32 {
    apply<local>(42)
}
```

This example uses an authored structural callable contract. A nominal slot
additionally requires the selected machine's exact satisfaction edge; inferred
wrapper requirements do not create one.

The body of `apply` acknowledges possible suspension. Its summary is
`suspend(F)`; `example` substitutes the published non-suspending `local` and
needs no marker or suspension permission. A selected callback whose published
contract permits suspension requires `suspend apply<Callback>(42)` and a
caller context admitting suspension, even if a particular execution never parks.

Operational dependency summaries compose through wrapper calls, including
recursive components with an anchored contract, and belong to interface
identity and dependency invalidation. Normalize OR associatively,
commutatively and idempotently; preserve exact requirement identities and
substitutions. Adding a fixed effect or changing a callback contract requires
ordinary revalidation. No current-consumer enumeration or optimizer pruning
changes the published summary. Closed lowering realizes the specialized
envelope, without manufacturing continuation or blocking requirements solely
from the template's wider acknowledgement.

Preconditions, postconditions, crashes, termination and resource composition
retain their own checks. Forwarding a static callable requirement does not
automatically forward arbitrary runtime guarantees from that callback to its
wrapper's callers.

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

An invoked handler retains its exact identity in the synchronous call graph and
contributes its explicit reach and selected concrete operational envelope to the
current invocation; the handler's trait name is not an additional reach member.
Durable registration instead
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

The marker set must include the statically known call envelope: `suspend`
when suspension is possible and `block` when blocking is possible. In a generic
body, use the abstract envelope under its callback requirements; at a closed
application, use the specialized published dependency summary. Missing or partial
acknowledgements reject. Redundant acknowledgements are permitted without a
warning, including after specialization or a checked refinement removes an
operational possibility. Observing a non-waiting run cannot narrow the envelope.
The two axes are independent: `block` cannot replace a required `suspend` or
conversely. Combined order is `suspend block`; duplicate or reversed markers reject.

Named decision: `conservative-call-acknowledgements`, ratified 2026-11-06.
Authored markers and the checked envelope remain separate through checking and
lowering. Effects, carry, loans, invariant-window crossings, context admission
and continuation creation follow that envelope, not the presence of extra
markers. Generic bodies still satisfy their callback upper bounds; a synchronous
application cannot excuse an invalid template. Ordinary call-boundary and
invariant obligations remain even when the envelope permits no pause.

Markers acknowledge held borrows, claims, guards, and authority across possible
pauses. They do not force execution to pause, create a task/future, change ABI or
result, change inferred effects, or enter normalized machine identity. Suspension
parks an activation and requires a continuation; blocking retains its stack and
worker. Carry policy may reject a crossing with particular live values but
cannot rewrite the callee's envelope.

A call written with `suspend`, including a redundant acknowledgement, may occur
only as a complete statement, simple `let` right-hand side, transition subject,
or terminal expression, including a
[terminal machine call in a transition arm](state_contracts.md#terminal-calls-in-transition-arms).
Such an arm acknowledges every required effect, including when its target is a
nested machine. Nesting and later jump/frame-reuse lowering grant no exemption.
A `suspend`-marked call cannot nest inside an argument, operator, aggregate
construction, or condition. A call marked only `block` may nest when its checked
envelope permits no suspension. Keeping a redundant `suspend` reserves a legal
suspension position; it does not promise that live values could survive a later
change to the callee's envelope.

The opt-in [source rewrite](../build/command_line.md#source-rewriting) can remove
unnecessary markers from the checked source context. Formatting or ordinary
compilation does not remove them.

Borrowed arguments and other live loans may cross suspension under ordinary
[loan and carry preservation](../resources/carry.md#borrows-across-suspension).
Neither a `suspends` ceiling nor an acknowledgement supplies that evidence.
Borrowed/service-related types are not categorically forbidden, and boundary
identity does not exempt them from preservation checks.

Authored generated code and adapters obey these rules. Compiler-synthesized
adapters retain diagnostic/audit acknowledgement metadata without invented
source tokens. A marker cannot bypass an execution context's operational floor:
automatic [cleanup](../../language_guide/chapter_17_drops_and_cleanup.md#select-automatic-cleanup)
and [hermetic semantic evaluation](evaluation.md#invocation-admission) forbid
source suspension and blocking in the checked envelope. A redundant marker alone
does not violate that floor.
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

Deterministic normalizers own published service rows, reach and operational
dependencies, invocation contracts, suspension/blocking ceilings, and crash
buckets. Proof may discharge legality or enable optimization; it cannot shrink
an authored export, rewrite its guards, or change interface identity.
Specializing a published reach or operational dependency is contract
substitution, not proof-based shrinking of an authored opaque ceiling.
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

Automatic dependency propagation is limited to the specified reach unions
and suspension/blocking OR summaries. It introduces neither arbitrary
callback-contract projections nor new source acknowledgement syntax.
No anonymous-machine or lambda surface is accepted.
