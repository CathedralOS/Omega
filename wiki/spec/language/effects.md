# Service reach and operational ceilings

A machine's service reach, direct synchronous invocations, possible suspension,
worker blocking, and crashes are independent contract axes. Authority is held
in [values](../resources/authority.md); implementation trust is recorded by
[provider admission](../build/provider_selection.md). Neither is an effect row.

## Declarations

| Clause | Meaning |
| --- | --- |
| `reaches A + B` | Bounds service reach by the named boundary services and their boundary parents. |
| `reaches none` | Explicitly bounds service reach by the empty set. |
| `invokes handler` | May synchronously enter that binding before returning. |
| `suspends` | May park the current activation. |
| `blocks` | May occupy its worker while waiting. |
| `crashes Cause` with route guards | May leave without a successor or cleanup under a covered guard. |
| `crashes none` | Promises no crash route of any cause. |

Boundary traits supply identities that may be named in `reaches`; declaring or
calling one does not implicitly add its identity. A boundary call contributes
its requirement's explicit reach ceiling, with boundary-parent closure. Every
boundary-trait machine requirement must declare that ceiling, using `reaches none`
when it permits no services, and must likewise write `crashes` under
[open callable contracts](#open-callable-contracts). A memberless service-identity trait has no operation
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
ceiling. `reaches A + B` permits a body reaching only `A`; `reaches none` rejects
any nonempty derived service row. The keyword with no value is malformed.
Omission requests inference and is distinct from an explicit empty clause. Reach does not count local mutation as a service,
and empty reach promises neither termination nor absence of blocking or crashes.

All machine clauses have no clause-ending semicolon. A following clause or body
delimits the current clause. A bodyless declaration still has its final
declaration semicolon: `machine call() reaches none suspends blocks;` ends one
complete declaration with three clauses. A checked body writes
`reaches none suspends blocks` followed directly by `{ ... }`. Termination is
not an exception. Nested callable requirements use the same [header delimiters](state_contracts.md#machine-header-delimiters);
do not insert semicolons between their clauses.

Boundary requirements publish their explicit ceilings. Opaque supplies inherit
the governing requirement's admitted contract, not an inferred native-body
summary; a realizer need not repeat that same clause. Their explicit declaration
provides a stable contract across checked, opaque and foreign providers.
A selected implementation must fit every authored requirement ceiling even when
its own optional clause is absent.
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

### Deferred requirement reach

Named decision: `deferred-callback-reach`, ratified 2026-11-07. Omission on a
bodyless non-boundary requirement defers to the selected implementation's
published reach. The generic carries a row dependency until that selection is
known. The same rule applies to structural machine requirements, nominal
machine requirements, and members called through conformance evidence binders.

| Context | Omitted `reaches` | Authored clause |
| --- | --- | --- |
| Checked body, including public APIs | Infer from checked calls. | Bound the derived reach. |
| Bodyless non-boundary requirement | Open: defer to the selected implementation's published reach. | Bound admitted implementations; `reaches none` requires empty reach. |
| Boundary requirement | Reject the open contract. | Publish a stable ceiling across providers. |
| Runtime callable type | Reject the open contract. | Publish a fixed finite ceiling; `reaches none` states empty reach. |

Every public machine publishes its checked summary, including unresolved static
dependencies on generic interfaces. A requirement relying on service-free
behavior must state `reaches none`; current pure implementations cannot supply
that guarantee to an otherwise open generic body. Omission changes only service
reach. The callable shape and all other contract axes retain their own rules.
No `reaches _` spelling is introduced on originating requirements; the existing
[transparent-refinement form](conformances.md#transparent-refinements) retains
its inherited constraints.

### Open callable contracts

Named decision: `open-callable-contracts`, settled 2026-11-09. It subsumes
`explicit-callable-reach`. A callable contract is the signature and clauses of
a callable type, a trait member or a machine-shaped logical hypothesis. It has one
meaning wherever it is written:

| Omitted clause | Meaning in a callable contract |
| --- | --- |
| `reaches` | Open. No service bound is authored; the selected operation's published reach is retained as a dependency. |
| `crashes` | Open. No crash bound is authored; the selected operation's published guarded crash routes are retained. |
| `suspends`, `blocks` | Does not suspend; does not block. |
| `terminates` | No termination promise. |

A contract that omits `reaches` or `crashes` is open. An open contract admits
only a selection made at compile time, where its dependency is resolved by
substituting the selected operation's published contract. A closed position
needs both clauses written, with `none` for empty:

- a runtime callable type, whether it is a field, parameter, result, local or
  container element;
- a boundary requirement, which publishes one stable interface across
  providers.

An open contract in a closed position rejects, and the diagnostic names the
missing clause. Openness belongs to the contract. No position reinterprets an
omitted clause as empty, so a signature copied from a requirement into a field
type cannot change from "whatever the selection does" to "nothing" without the
author writing `reaches none` or `crashes none`. Checking a statically known
selection against an explicitly closed destination is ordinary
[contract refinement](machines.md#substitution).

```omega
data CounterOps {
    clear: machine(counter: &mut Counter) reaches none crashes none terminates;
    value: machine(counter: &Counter) -> u64 reaches none crashes none terminates;
}
```

A checked machine definition is not a callable contract and keeps the rules
below: its body infers reach, a private body may infer the other axes, and a
public body with no `crashes` clause promises none. Implementation is tracked
by `OPEN-CALLABLE-CONTRACTS`; see the
[design record](../../drafts/designs/open_callable_contracts.md).

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
        reaches MotorControl
        crashes none;
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
surfaces omitted `suspends` means never parks and omitted `blocks` means never
blocks a worker. On a public checked body omitted `crashes` means no crash
route, exactly as `crashes none`. Private checked bodies may omit any of the
three to request inference. On a bodyless non-boundary requirement omitted
`crashes` defers to the selected implementation, and `crashes none` admits only
crash-free selections. A boundary requirement and a runtime callable type must
write the clause under [open callable contracts](#open-callable-contracts). Suspension and blocking are authored on published surfaces because
their call-site markers are checked against a known bound. Crashes are authored
there because a trap needs no additional authority and no marker, so inference
would let a helper edit widen a public contract to a domain-ending outcome
without any author touching that surface; a public machine that can crash
states its causes and guards, and callers permit or disprove them. Service
reach remains inferred on public bodies: a body cannot reach beyond the
authority it holds, admission policy and
[build-level behavior exclusions](../build/behavior_exclusions.md#two-independent-checks)
govern service use, and an authored `reaches` ceiling stays available for a
narrower promise. A public generic that admits a suspending, blocking or
crashing callback declares the corresponding ceiling. Its published
[dependency terms](#static-binder-contract-dependencies) separately record
which static selections contribute each possibility.

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
provider reach         subset-of requirement reach ceiling, when one is declared

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

This is the reach row of
[static binder contract dependencies](#static-binder-contract-dependencies).
A static callable invocation contributes the selected operation's reach
dependency automatically. Structural and nominal machine binders use the same
rule. A call such as `S::apply` through `const S: ElementStep<T>` contributes
a projection keyed by that evidence binder, the exact requirement member and
its substitutions. Different members of one conformance retain distinct rows.
Evidence selects an implementation; it does not attach operations to a data type.

Published summaries contain finite unions of concrete service identities and
these static row projections. An omitted requirement clause leaves its projection
open; an authored clause bounds it, including the explicit empty bound. The
selected machine or conformance already determines the row, so applications
supply no additional row argument. No subtraction, lower bounds, Boolean formulas
or arbitrary author-written row expressions are introduced.

A generic body is checked against every permitted selection. An unresolved open
projection cannot fit a finite authored ceiling in that body. A bounded
projection fits when its bound fits. Omitting the enclosing clause publishes the
derived union; an authored ceiling contributes no services to that union.
Closed callers substitute first: a Console-only `for_each<u32, Log>` application fits
a caller's `reaches Console`, while the same application fails under bare
`reaches`. The open requirement does not itself grant any service authority.

The compiler derives the dependency structure from the checked call graph. Each
closed application substitutes the selected machines' published contracts into
that structure. It does not inspect a selected opaque body to narrow its promise.
For example, a body calling only `Step` has dependency `reach(Step)`; one also
calling a Console-reaching helper has `reach(Step) + Console`. These expressions
describe retained semantic data, not new source syntax. Generic wrappers compose
these dependencies through further ordinary calls and exact substitutions.
Forwarding preserves the originating requirement's open or bounded reach policy.
An opaque selection needs an admitted published row; missing information remains
unresolved and cannot be treated as empty.

Use the ordinary deterministic service normalizer for dependency summaries:
union is associative, commutative, and idempotent, empty is its identity, and
boundary-parent closure applies. Binder identity is structural, not its spelling.
Call reordering or extraction into a private helper with the same net reach must
not change the normalized dependency. Do not prune authored call contributions
using optimizer reachability, proof heuristics, or the current consumer set.

The normalized dependency is part of the exported interface. Retain its open or
finite bound, exact binder/member and requirement identities, substitutions, and
selected contract dependencies for independent checking. A private-helper edit
adding service reach can therefore change an exported interface; this is an
accepted consequence of propagation.
Changed interface identity requires ordinary revalidation, not an automatic
package-version bump or rejection of every caller. A requirement or evaluation
context that cannot admit the new reach rejects; unconstrained callers propagate
it onward. Stale retained summaries cannot authorize the changed application.

The application's specialized row is used by ordinary callers as well as
evaluation admission. A no-reach callback can leave a traversal with empty reach;
a Console-reaching callback contributes Console. Empty reach alone establishes
none of the other evaluation obligations.

### Static binder contract dependencies

Named decision: `static-binder-contract-dependencies`, ratified 2026-11-07.

A checked generic's published contract is a function of its static callback
binders' published contracts. Check the body once under the requirements'
bounds. For each operational axis, derive a normalized term over exact binder
projections in that axis's algebra together with the body's fixed
contribution. Each application substitutes the selected callbacks' published
contracts into those terms before caller admission. The template must remain
valid for every admitted selection; a favorable selection never rescues an
invalid body. Every term reads only the selection's published contract. A
selected callback's private body, observed behavior, predicted optimizer
target and the current consumer set contribute nothing.

| Axis | Term | Omitted on a checked body | Omitted on a non-boundary requirement | Application receives |
| --- | --- | --- | --- | --- |
| `reaches` | union of fixed services and binder rows | infer | defer to the selection | fixed services plus the selected published rows |
| `suspends`, `blocks` | OR of the fixed contribution and binder flags | private infers; public means never | never | fixed contribution OR the selected published flags |
| `crashes` | per cause, union of fixed and binder routes, then path-fact disproof | private infers; public means none | defer to the selection | fixed routes plus the selected published routes, disproved routes removed |
| `terminates` | own progress AND binder promises | no guarantee | no promise | the guarantee once every binder premise is discharged by a selected published promise |
| suspension carry | conditions over the suspension term | derived | derived | discharged where substitution removes the suspension |
| result relations | proof-only relation per binder, composed along value flow | authored claim or none | the requirement's guarantee bounds it | the selected published relation under the authored claim |
| write frame | union of fixed paths and binder frames applied to their argument places | infer | defer to the selection | fixed paths plus the selected published frame |

One rule produces that table. Omission derives the answer wherever derivation
cannot widen a public surface without an author's hand. Suspension and
blocking have call-site acknowledgement markers that are checked against a
known bound, so a published surface and a requirement author them. A crash
needs no additional authority and no marker, so a published surface authors
it too, while a requirement defers. Termination is a positive promise, and a
promise that silently disappears on a body edit is worse than an effect that
appears, so it stays authored. Reach infers on a body and defers on a
requirement: a body cannot reach beyond the authority it holds, and an
authored ceiling remains available for a narrower promise. An explicit empty value, `reaches none`, `crashes none`,
`suspends false` or `blocks false`, promises the empty value and rejects any
body or selection that violates it. A
[write frame](dependent_values.md#mutation-frames) infers on a body and defers
on a requirement for the reason reach does, and has no authored form.
Boundary requirements and runtime callable
types have no body to infer from and no static selection to defer to, so they
write `reaches` and `crashes` and take the published-surface defaults for the
other axes; their write frame is the signature's ceiling.

The axes exchange facts without sharing an algebra. A disproved crash route may
use the facts a preceding callback's requirement establishes; a false suspension
condition discharges its guarded carry obligation; a binder termination premise
is discharged only by a published promise. Each connection is owned by the axis
section it belongs to.

Every term, bound and premise belongs to the enclosing generic machine's
published contract and interface identity. Changing one is an interface change
even when the authored header is unchanged, and dependents revalidate. An
admitted closed application retains its guarantees across that change; a
generic recipe and its published identity may change. Requires retain their
own authored checks. A callback's result guarantee reaches its wrapper only
through an authored claim; see [callback result relations](#callback-result-relations).

The reach, operational and carry sections are rows of this table and retain
their decision names. Crash routes compose in [guarded crashes](#guarded-crashes);
termination premises in
[published guarantees](termination.md#published-guarantees-and-private-witnesses).

### Callback result relations

Named decision: `callback-result-relations`, ratified 2026-11-08. This is the
postcondition row of the table above.

A static callback binder carries a proof-only result relation: the selected
callback's published normal-return guarantee, bounded by the binder's
requirement. A checked normal return of `F(x)` establishes that relation for
the invocation's exact arguments and returned value, and for nothing else. The
relation is evidence on the normal-return path only; it implies no progress,
and a callback that may crash or diverge still supplies it where it does
return. During generic checking the relation is opaque: the body may rely on
the facts the requirement states and on the identity of the values the relation
holds between, never on a property only some selection would publish.

A wrapper publishes a callback's result relation only by authoring the claim.
An `ensures` clause may apply a binder's result relation to exact subjects,
alone or composed with ordinary propositions and mathematical definitions; the
checker discharges the claim from the evidence above or rejects it. Composition
follows value flow, not a union: a result returned unchanged keeps its
relation; two calls chain through the intermediate value; a branch between
callbacks yields a guarded alternative; a discarded result does not transfer
its relation to an unrelated value, though the completed call still establishes
that some related result existed. Ordinary call admission is unchanged:
applying `F` to `F`'s own result needs the second invocation's preconditions
established like any other call. Applying the relation states a proposition;
it does not invoke the callable or compute its result.

Applications substitute the selected callback's published relation. A
specialized wrapper therefore inherits a stronger published guarantee from a
stronger selection, and changing a callback's published contract changes the
specialized wrapper contract under the ordinary dependency rules. Explicit
mathematical relations shared by a requirement and its consumers remain
available for independently designed specifications. The first scope is
immutable arguments and normal returns; mutable arguments follow the existing
storage-version and invalidation rules. No derived relation is exported without
an authored claim.

The projection is spelled `F::ensures(a1, ..., an, r)` on the static binder:
the callback's parameters in declaration order, then the subject standing for
its result. Any in-scope value may fill any position; `result` is the usual
final subject in a forwarder's claim, not a required token. The application is
an ordinary proof-only proposition and composes with other propositions and
named mathematical definitions. The static-selection path `::` keeps the
reference attached to the exact binder and its selected contract; it is not a
field selection, a call, or a first-class value of the unapplied relation, and
it does not introduce a `requires` projection.

```omega
public machine forward<const F: machine(value: u64) -> u64 ensures result >= value>(
    x: u64
) -> u64
ensures F::ensures(x, result)
{
    F(x)
}
```

The requirement states the least an admitted `F` must guarantee. The wrapper
promises that the selected `F`'s published relation holds between `x` and its
own result, so a selection guaranteeing `result == value + 1` gives this
application that guarantee. A two-call wrapper relates `x` to its result
through the intermediate value using the ordinary
[witness machinery](../proofs/mathematical_bindings.md#witnesses-existence-and-proof-construction);
there is no existential syntax.

### Static callback operational dependencies

Named decision: `static-callback-operational-dependencies`, ratified 2026-11-06.
This is the suspension and blocking row of
[static binder contract dependencies](#static-binder-contract-dependencies).

A checked generic calling a nominal or structural static callback publishes
suspension and blocking dependencies. Service reach follows the static union
rule above; these operational axes retain their separate Boolean bounds. Each
operational axis is a finite Boolean OR of a fixed contribution and exact
callback-binder projections,
bounded by the respective requirement permissions. The compiler derives this
summary from checked calls; there is no authored effect variable or new marker.
Inferred [callable parameter types](generics.md#inferred-callable-parameter-types)
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

The body retains the [conditional suspension carry](#conditional-suspension-carry)
obligations derived below. Ordinary ownership, access, invariant-window and
outcome checks still apply at the call, including when a selected callback cannot
suspend. A narrower originating callback contract can state that guarantee
directly.

```omega
public machine apply<const F: machine(value: u32) -> u32 suspends>(value: u32) -> u32
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

Crash routes and termination premises compose through the same substitution;
see the [per-axis table](#static-binder-contract-dependencies). Preconditions
and resource composition retain their own authored checks. Forwarding a static
callable requirement does not automatically forward a callback's logical
guarantees to its wrapper's callers; a wrapper publishes a callback's
[result relation](#callback-result-relations) only by authoring the claim.

Callable effect rows have no service subtraction, negation, masking, scoped
allowance, or algebraic-effect handlers. Independent product exclusions do not
add those operations. A checked in-memory provider for `Readable` may remove
opaque trust expenditure but cannot erase the abstract service from callers.
Ordinary traits and wrappers cannot hide reach, blocking, or suspension.
Possessing authority cannot bypass a published ceiling, and listing a service
cannot create its required capability.

#### Conditional suspension carry

Named decision: `conditional-callback-carry`, ratified 2026-11-07. This is the
suspension-carry row of
[static binder contract dependencies](#static-binder-contract-dependencies).
A generic may retain a value across a static callback under the condition
that, if the call can suspend, the value permits that crossing. Derive this condition during the
single parametric body check; no new source clause or runtime test is introduced.
This supersedes the unconditional suspension-permission check for such sites.

Use the call site's normalized suspension dependency: the existing finite OR of
fixed contributions and exact callback projections. A forwarded call can depend
on several binders. For each live value or resource claim whose suspension
permission is not already established, retain the suspension-axis carry
obligation guarded by that expression. Constant false requires no suspension
permission. Constant true, including a fixed independent suspension contribution,
requires it unconditionally. A non-constant condition can remain in the generic
contract until selection; no observed consumer set or private-body inspection
discharges it.

Each conditional row belongs to the enclosing generic machine's contract, not
to the callback requirement. Retain the exact call site, symbolic live subject
or claim identity, provenance, required suspension permission and condition.
Local acquisition subjects retain their declaration and invocation scope;
forwarding substitutes both callback binders and resource subjects/provenance.
Caller-visible subjects are checked with their corresponding arguments. A
helper-local claim uses its own acquisition contract and retained evidence;
the caller cannot manufacture a permission for that private claim.

Applications substitute selected published callback contracts and applicable
type/resource evidence. A false condition discharges only its guarded carry
obligation. Otherwise establish the required permission under the ordinary carry
rules, including `Carry::AcrossSuspend` where the claim requires it, or reject.
An unresolved condition may propagate through another checked generic contract;
it cannot be silently erased or deferred to runtime at a closed application.
Permission evidence must retain its valid provenance and cannot override an
explicit suspension-forbidden type policy. Diagnose the exact resource, crossing
and selected callback contributions that prevent admission.

Only suspension permission is conditional here. Ownership, loan access and
lifetimes, invariant-window fences, outcome/cancellation paths, CPU/thread/address
preservation and obligations caused by the ordinary call keep their existing
rules. A synchronous callback does not excuse an unfinished invariant exposed
by that call. Blocking does not acquire this conditional rule. The
`suspends false` refinement remains available for an explicitly synchronous
callback requirement.

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

Starting a task is separate: `TaskRuntime::start<M>` acknowledges only what `start`
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

A checked generic calling a static callback composes crash routes per cause
under [static binder contract dependencies](#static-binder-contract-dependencies):
the body's fixed routes together with each binder's routes, mapped through the
call site's actuals. Applications substitute the selected callbacks' published
routes and then apply the disproof above. A requirement that omits `crashes`
admits any selection and contributes that selection's published routes; a
requirement stating `crashes none` admits only crash-free selections. A public
wrapper that admits crashing selections states `crashes Trap` or `crashes
Abort` as its ceiling; the composed term still decides which applications
carry a route. A private wrapper's omitted clause publishes the composed term.

Only a body inside the same fingerprinted verification unit may supply a local
body summary; otherwise use the published ceiling. Reuse of published judgments
follows [Psi admission](../terminal-psi/product.md#publication-and-separate-consumption).
During interpretation, actual operation checks and installed host permissions
apply independently of a guest's declared ceiling.
A physical check may remain after its semantic route is disproved unless valid
specialization removes it. Crash-frontier lower bounds report definitely live obligations, not
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
dependencies, invocation contracts, suspension/blocking ceilings, crash
buckets, and write frames. Proof may discharge legality or enable optimization; it cannot shrink
an authored export, rewrite its guards, or change interface identity.
Specializing a published reach or operational dependency is contract
substitution, not proof-based shrinking of an authored opaque ceiling.
Every [static binder dependency term](#static-binder-contract-dependencies),
including crash routes, termination premises and conditional suspension-carry
rows, is part of the enclosing machine's published contract and interface
identity, including normalized subjects, provenance and conditions. A body edit
that changes such a term changes the public contract even when the authored
header is unchanged. Forwarding and specialization preserve or discharge them
by exact substitution. Changed dependencies require ordinary revalidation;
they do not change installation-bound service rows.
Stable syntactic/control-flow normalization is
not heuristic entailment. Exact authored member/keyword spans explain review;
inferred rows and parent closure receive no invented source locations.

An installation-bound requirement uses `reaches <= Bound` for its abstract
service row. Its exact requirement path owns the row. The selected provider's
row must fit the finite bound and replace it throughout the owning installation
closure before admission. This installation rule is distinct from ordinary
[static callback reach dependencies](#static-callback-reach-dependencies).
Ordinary generic exports may carry the open or bounded union dependencies
specified there. Every installed closed application must resolve them to a
finite row before admission; an unresolved variable rejects. No general row
algebra or lower-bound constraints are implied.
The independent `reaches _;` inside a
[transparent refinement](conformances.md#transparent-refinements) constrains its
base requirement under its inherited bound; it does not create another
installation-row declaration form.
[Installed roots](../build/external_roots.md#installation-bound-reach) owns
manifest and lineage requirements; equal rows never establish protocol identity.

Automatic dependency propagation is limited to the axes of the
[per-axis table](#static-binder-contract-dependencies): reach unions,
suspension/blocking OR summaries, crash route unions, termination premises and
conditional suspension-carry obligations. It introduces neither arbitrary
callback-contract projections nor new source acknowledgement syntax.
No anonymous-machine or lambda surface is accepted.
