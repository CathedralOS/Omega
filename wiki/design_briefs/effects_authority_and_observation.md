# Design Brief: Service Reach, Synchronous Invocation, Authority, And Observation

Revised 2026-08-06. This brief defines a service-reach `reaches` row, a direct
synchronous `invokes` ceiling, independent `suspends` and `blocks` operational
ceilings, and the guarded `crashes` ceiling. `terminates` remains the separate
positive progress guarantee settled by decision 23. It records each axis's
propagation/refinement laws and its relationship to authority and trust.
General trace theorems, quantitative resource entries, service-row
polymorphism, and additional operational clauses remain explicitly deferred.

## The surface

Omega spells service reach and execution behavior separately:

```omega
machine backup(
    src: [u8]::Utf8,
    dst: [u8]::Utf8
) -> BackupResult
   reaches Readable + Queryable;
   suspends;
{
}
```

The `reaches` row is a `+`-separated set of name-resolved boundary-service
identities such as `Readable`, `Queryable`, `Clock`, or `ProcessExit`.
Boundary traits contribute service reach; ordinary traits do not.

`suspends;` says an invocation may park its current activation. `blocks;` says
it may occupy its worker while waiting. These are independent public may-
ceilings, not service identities and not members of the `reaches` row.
`terminates;` has the opposite polarity: under its pinned premises, every
invocation eventually reaches a terminal outcome. It is neither an effect nor
an operational may-clause.

There is no `budget`, `fails`, `uses`, `trust`, negative effect, or scoped mode
syntax. Checked IR, diagnostics, manifests, contract identity, and provider
admission retain all four fields separately.

Boundary traits automatically contribute their service-reach identity. A
boundary operation declares additional reached services and its operational
ceilings separately:

```omega
boundary trait Readable {
    machine read(
        path: [u8]::Utf8,
        out: &mut Vec<u8>
    ) -> ReadResult
      suspends;
}
```

The effective service row of that call contains `Readable`, and its suspension
ceiling is true. A wake operation may reach its scheduler service without being
declared `suspends`; reach is trait-granular while temporal behavior is
operation-granular.

Reach auditing has no per-trait opt-in beyond declaring the service boundary.
Checked bodies infer their complete row, bodyless surfaces publish it, and
callers inherit it. A deployment profile may classify entries such as
`DynamicLibraryLoading` as forbidden or review-critical, but that policy does
not change propagation or source contract identity. Static selection of a DLL
provider is an artifact/provider fact rather than a loader reach; only a
runtime loading operation reaches the loading service.

## Contract axes

The complete machine contract retains independent axes with distinct source
and artifact homes:

| Axis | Durable home |
|---|---|
| Service reach | `reaches` row |
| Direct synchronous boundary invocation | `invokes` clause and erased direct-edge metadata |
| Possible suspension | `suspends` clause and suspension plan |
| Possible worker blocking | `blocks` clause and blocking plan |
| Authority possession | capability values, domains, and parameters |
| Trust reach | provider/admission receipts and the trust ledger |
| Positive temporal guarantees | operation/provider contracts and context floors |
| Resource consumption | explicit capabilities and dependent contracts in v1 |
| Recoverable failure | returned sums and case-specific contracts |
| Crash possibility | guarded `crashes` buckets and crash plans |
| Mutation | ownership, borrows, and state contracts |

Frame size is compiler-derived and reported. Task activation capacity is
declared or proved through task-pool authority. Version retention is a
component/deployment budget. Heap/region capacity is an explicit resource
value. Only frame size is report-only; no unbounded retention becomes
invisible merely because it does not belong in the reach row.

## Polarity and ceilings

Service rows and operational may-clauses describe **possible** behavior and
therefore accumulate upward through calls. Absence is the negative guarantee:

```text
`suspends` omitted => the machine never parks
`blocks` omitted   => the machine never occupies a worker by blocking
Writable absent    => the machine never reaches the Writable service surface
```

Presence is permission in the published ceiling, not proof that every
execution performs the behavior. A declared `Readable + Queryable` service
ceiling may contain a body whose inferred row is only `Readable`; a machine
declared `suspends` may return immediately on every invocation observed in one
run.

Internal machines may omit these clauses and receive inferred service,
suspension, blocking, and crash summaries. Exported machines, boundary
operations, and trait requirements publish each ceiling. An omitted `reaches`
row there is empty; omitted `suspends` means never parks; omitted `blocks`
means never blocks a worker; and an omitted crash cause is forbidden.
Diagnostics name the violated axis.

Authority possession and permission remain separate. Holding a Writable
capability does not let a machine whose ceiling omits `Writable` exercise it.
Conversely, listing `Writable` does not mint the capability required by the
operation.

### Guarded crash ceilings

Crash possibility is the flow-sensitive may-axis:

```omega
machine divide(n: i32, d: i32) -> i32
crashes Trap Activation
    d == 0
    n == i32::Minimum && d == -1
crashes Abort ExecutionDomain
    configuration_invalid
{
}
```

Each clause names one cause and one containment scope. Its indented entries are
alternative routes, so the clause denotes their disjunction. An empty route
list denotes `true`. `crashes Trap` is the conservative shorthand for an
unconditional `Trap` route at `ExecutionDomain`, the permanent portable top.
Omitting a cause from a published contract forbids that cause. Private omission
infers a summary.

The initial cause identities are `Trap` and `Abort`. Cause identity controls
policy and lowering; both are no-successor, no-cleanup exits. Cause identities
are append-only. Context crash policies are sparse maps keyed by cause, with an
absent cause forbidden, so adding a cause does not change an existing context's
identity.

For each derived crash site, let `D` be its path-conditioned guard and
`minimum(D)` the smallest termination scope required to keep survivors sound.
The authored contract supplies route guards `C_i` and published containment
demands `scope_i`. Coverage is two-dimensional:

```text
D implies OR_i(C_i && minimum(D) <= scope_i)
```

The declaration is a public ceiling, not an exact body summary. A body may
derive narrower guards or scopes without changing exported identity. A wider
body result rejects. Across calls, substitute arguments into the published
routes and discard every route disproved by current facts. Surviving routes
propagate upward; disproving all routes removes that cause at that invocation.
Scopes are retained as separate buckets and compared independently, so the
model needs only a stable partial order rather than a join operation.

This refinement is deliberately unlike `suspends` and `blocks`. Whether a
particular call parks or waits does not change those published booleans. Crash
guards are propositions, and proving every route impossible removes the crash
edge from the caller's semantic frontier. Specialized lowering may then erase
the physical check; separate lowering need not.

Local checked calls may use path-conditioned body summaries only when the body
is inside the same fingerprinted verification unit. Imports, generic/dynamic
requirements, boundaries, and separately verified artifacts use the published
ceiling and its certificate. No discharged obligation may depend on a body that
is absent from the verified artifact.

The checked representation materializes invocation refinement before typed
source is discarded. Each direct published-callee row records the stable
state/statement/call coordinate, the target contract fingerprint, the caller's
exact incoming path conjunction, and the independently surviving
cause/containment buckets. Concrete false routes disappear, concrete true
routes normalize to unconditional alternatives, unknown routes are retained in
the caller's positional parameter namespace, and a call with no survivors
remains explicit crash-free evidence. Same-unit body-summary selection is
live. Callable trait requirements and unresolved compile-time machine
parameters now use source-independent crash-contract capsules that pin their
published buckets to the complete normalized callable-contract fingerprint.
Recursive guarded-crash fixed points and separately compiled import capsules
remain subsequent composition steps over that carrier.

An enclosing execution context publishes a maximum tolerated scope per cause.
The map belongs to the activation, task, supervisor, or root that expects state
to survive; leaf machines do not repeat it. Provider or Build APIs may construct
that context plan, but the normalized sparse map is fingerprinted semantic
content rather than installation-supplied policy.
For every surviving route:

```text
derived_site_minimum
    <= published_route_demand
    <= context_maximum[cause]
```

Installation binds the nominal scopes to a selected target fault plan and must
also establish:

```text
published_route_demand
    <= realized_target_scope
    <= context_maximum[cause]
```

The lower installation bound is not redundant: containing a fault to one
activation is unsafe when that activation crashed with a domain-wide shared
invariant open. The upper bound protects what the context expects to survive.
Psi fingerprints and checks the portable demands; Omega installation retains
the selected plan and evidence that realizes them.

## V1 composition algebra

Service reach normalizes by deterministic, idempotent set union plus trait-
parent closure:

```text
R + R = R
R + empty = R
Readable <= Filesystem       when Filesystem inherits Readable
```

The checker enforces:

```text
inferred_service(body) subset-of declared_service_ceiling
callee_service         subset-of caller_service_ceiling
provider_service       subset-of pinned_slot_service

inferred_may_suspend => declared_suspends
callee_may_suspend   => caller_suspends
provider_may_suspend => pinned_slot_suspends

inferred_may_block => declared_blocks
callee_may_block   => caller_blocks
provider_may_block => pinned_slot_blocks

derived crash sites covered-by declared crash buckets
refined callee buckets         covered-by caller crash buckets
provider crash buckets        covered-by pinned slot crash buckets
```

Private omitted fields are the least conservative fixed points over the checked
call graph. Recursive call components compute finite service, suspension,
blocking, and guarded-crash fixed points. Crash propagation substitutes call
arguments, conjoins caller path facts, and discards disproved routes rather
than taking a plain unconditioned union. Calls to local checked machines use
checked callee summaries. Imports, generic/dynamic requirements, and boundary
calls use their pinned requirement ceilings, never facts learned from an
eventually selected provider.

Provider admission is deterministic. A machine compiled against a slot that
`suspends` but does not `block` remains nonblocking; a provider whose checked or
accepted contract blocks fails refinement. A slot declaring both clauses
admits either behavior and its consumers carry both possibilities honestly.

## Direct synchronous boundary invocation

Service reach is a transitive audit and admission ceiling. It intentionally
forgets which boundary edge was crossed next and whether an external root ran
later. The `invokes` clause preserves the direct synchronous information needed
for component-cycle and stack-topology checks:

```omega
boundary trait EventSource {
    machine register_and_fire(handler: Handler) -> Registration
    invokes handler;
}
```

`invokes handler` means this invocation may synchronously enter the binding
named by `handler` before returning. It automatically contributes the
handler's boundary-trait identity and the selected conformance's realized
operational envelope to the current invocation's normalized reach. It does not
say that every execution calls the handler.

Bodyful machines infer their `invokes` sets from checked bodies, including
forwarding through local helpers. Exported implementations check that inferred
set against their published ceiling. Bodyless requirements declare it;
omission means no synchronous invocation. Parameter paths retain per-value
precision when several bindings satisfy the same trait. A trait identity may
name an internally selected boundary binding when no parameter path exists.

Moving a handler into a linear registration establishes a future external root
instead. The registration operation does not inherit that root's reach unless
it also declares `invokes handler`. Root establishment separately requires the
selected admission policy to permit the concrete handler envelope, and the
registration value's compiler-tracked claim metadata retains that conformance
without widening to the trait ceiling.

Cycle checking consumes the realized direct `invokes` graph, never the
transitive `reaches` row. The synchronous graph across Omega component
boundaries must be acyclic. A mailbox, queue, scheduler handoff, or other
new-activation boundary breaks an edge structurally. The final artifact may
still contain reach cycles among independently entered roots.

## Call-site acknowledgements

The independent operational axes have independent call-site
acknowledgements:

```omega
ordinary_call();
suspend may_park();
block may_block_worker();
suspend block may_do_either();
```

`suspend` and `block` are prefix markers on the call, not declarations and not
execution operators. Each says that the call's statically known operational
envelope permits the corresponding behavior. The invocation may complete
immediately. The markers do not force a park or wait, alter the inferred
contract, create a task or future, change the call ABI or result, or enter
normalized machine identity.

Both markers acknowledge a point where execution may pause while live borrows,
claims, guards, and authority remain held. The distinction still matters:
suspension parks the activation and creates a continuation boundary; blocking
retains the ordinary stack while occupying the worker.

The checker compares the exact marker set with the statically known call
envelope:

```text
call may suspend  <=> `suspend` is present
call may block    <=> `block` is present
```

Missing, partial, and redundant acknowledgements reject. Local checked calls
may use the callee's checked summary. Imported operations, requirements,
generic calls, and boundary operations use their pinned envelope. A dynamic
call uses the per-requirement envelope statically retained by that value. A
transparent refinement such as `suspends false` or `blocks false` therefore
removes the corresponding marker requirement; one lucky non-waiting invocation
does not.

The canonical combined order is `suspend block`. Because suspension must retain
all partially evaluated state, a suspending call may appear only as a complete
statement, a simple `let` right-hand side, a transition subject, or a terminal
expression. It may not nest inside another call's arguments, an operator,
aggregate construction, or a condition. A blocking-only call creates no
continuation boundary and may nest, although a separate binding often improves
reviewability.

Authored generated code and authored boundary adapters obey the same syntax.
Compiler-synthesized adapters have no source token, so checked artifacts record
the synthesized acknowledgement at the call site. That record is diagnostic
and audit metadata, not semantic identity. Automatic cleanup and hermetic
semantic evaluation forbid suspension and blocking at their operational floor;
writing a marker cannot admit either behavior there.

`runtime.start<M>` remains distinct. Supplying `M` starts a new activation and
does not acknowledge whatever `M` may later do. The call to `start` itself uses
`suspend`, `block`, both, or neither solely according to what `start` may do to
the current activation.

Completeness is relative to checked bodies and pinned/accepted contracts. A
boundary provider may lie about its implementation; that is a trust failure
recorded by provider receipts, not evidence that row inference was incomplete.

Carry policy is not a service or operational clause. It constrains whether a
particular call, suspension, migration, or relocation is locally legal while
specific values are live. Such a constraint may reject a call whose static
contract may suspend; it never masks or rewrites the published suspension
ceiling.

## Published identity and proof gating

The normalized authored service row, suspension/blocking ceilings, and guarded
crash buckets are independent parts of an exported machine's semantic contract
identity, requirement-binding identity, and component compatibility surface.
Body inference only checks inclusion on each axis.

> **Published-identity law:** every published identity is owned by a small,
> deterministic normalizer. The prover may gate legality, discharge an
> obligation, or enable erasure/optimization; it may never redefine a
> published identity.

Proof strength therefore cannot silently shrink an exported service row, erase
an operational ceiling, rewrite a published crash guard or scope demand, or
change a contract hash. Internal legality may improve when a stronger prover
establishes that a path is unreachable, but exported identity remains authored.
Stable syntactic/CFG reachability feeds normalization; heuristic entailment
does not.

This is the service/operational-contract instance of decision 19's
normalization-versus-entailment law and the component requirement-binding admission
law.

## No laundering, masking, or handlers

V1 has no service subtraction, masking, scoped allowance, or algebraic effect
handlers. Under reach semantics, a call through `Readable` has reached that
abstract service even when a checked in-memory provider supplies it. Provider
substitution can remove trust expenditure and refine suspension/blocking
behavior; it does not rewrite the caller's published service history.

This is an intentional difference from algebraic-effect handler systems:
virtualizing a service does not make the abstract service call disappear from
Omega's row. The benefit is stable provider-agnostic contracts and no hidden
control-flow construct; the cost is that a checked provider does not make an
otherwise effectful abstract program row-empty.

An implementation cannot launder service or operational behavior through an
ordinary trait or helper. A logger that reaches `Writable` propagates
`Writable`; a provider that blocks derives `may_block` in its checked contract
and cannot satisfy a slot that omits `blocks`.

## Reach and the other axes

### Authority and trust

Service rows say which boundary services may be reached. Operational clauses
say whether execution may park or block. Capability values say what authority
the machine possesses. Trust receipts say which selected realizations crossed
out of proved Omega. A checked provider and host provider may satisfy the same
contract while spending different trust; all published ceilings remain
provider-agnostic.

There is no ambient-service category. Provider threading may gain ergonomic
sugar later, but authority remains possessed and transferred as values.

### Resources

V1 resource bounds remain contracts on explicit resource capabilities:

```omega
machine parse(heap: &mut HeapBudget, input: &[u8]) -> ParseResult
    requires heap.remaining >= required(input.len)
    ensures heap.remaining >= entry(heap.remaining) - max_used(input.len)
{
}
```

Parameterized entries such as `Alloc<Peak, Retained>` are reserved for the
resource-algebra brief. A single `Alloc<N>` is not compositional: sequential
peak depends on both prior retained usage and the next peak. No quantitative
entry lands until branch, loop, parallel, cancellation, and resource-identity
composition are deterministic.

### Failure and control

Recoverable failure remains a return sum with case-specific guarantees
(decision 18). There is no `fails` clause. Cooperative cancellation remains a
sum delivered to the task. `crashes` independently publishes non-returning
`Trap` and `Abort` routes, their predicates, and their containment demands.
Calling a process-exit service may contribute both `ProcessExit` reach and an
`Abort` route, but neither axis is reconstructed from the other.

### Totality and positive liveness

`terminates` guarantees eventual terminal progress conditional on explicit
requirements and the progress premises pinned by reached operation/provider
contracts. It does not prove no suspension, fairness, a deadline, eventual
wakeup, or starvation freedom by itself. Omission of `suspends` or `blocks`
proves only the corresponding negative operational guarantee.

Decision 23 represents v1 positive progress premises as opaque, sealed
profiles on boundary traits/providers/slots. They use grant receipts,
participate in admission, and never entail proof facts. General trace theorems
and profile entailment remain deferred. See
[Termination, Ranking, And Progress](termination_ranking_and_progress.md).

## Staging and extensibility

V1 recognizes service members from boundary-trait declarations and has the
closed operational clauses `suspends`, `blocks`, and `crashes`. Additional
operational clauses, quantitative service entries, and service-row
polymorphism are deferred until their algebras have real customers.

The compiler now uses the service-reach semantic model directly: suspension and blocking use
dedicated recursive boolean summaries, while boundary-trait declarations
mint canonical symbol-keyed identities after resolution, and normalized rows
with parent closure drive recursive inference, checked ceilings, static-machine
and checked-provider admission, contract/provider identity, snapshots, and
manifests. Checked flow/graph carriers, semantic reports, build-time purity,
assembly reach, executable manifests, and provider-plan schema identity now
consume the split representation directly. Boundary-provider approval is
symbol-exact, capability flows use normalized call topology, and categorical
provider authority/reporting no longer projects service names. Static-machine
refinement consumes exact service rows, checked trees carry grouped
`ServiceReachFacts` directly, and machine-contract identity has no legacy
legacy effect-row field or fingerprint input. The obsolete
`EffectRowId`/`EffectRowTable` carrier and global lowercase service-name/u64
table are deleted. Core, resolved trees, and typed trees retain only
symbol-resolved `ServiceReachRowId` values.
Build-script admission now consumes exact service reach and admits only the
pinned canonical `FilesystemHost` and `Console` staging slots; custom boundary
wrappers do not inherit admission from a category alias. Unknown service
identifiers resolve normally; there is no global hard-coded service table.

## Acceptance register

1. Calling `Readable::read` contributes `Readable` service reach and its
   declared suspension ceiling; calling a wake-only operation does not imply
   suspension.
2. A machine that omits `suspends` from its public contract cannot transitively
   call a suspending requirement.
3. A blocking provider cannot satisfy a slot that declares `suspends` but omits
   `blocks`; it can satisfy a slot declaring both.
4. Possessing Writable authority does not permit a call when the machine row
   omits `Writable`; listing `Writable` does not create the authority.
5. A checked in-memory Readable provider produces no trust receipt but does not
   subtract `Readable` from the machine's row.
6. Trait inheritance normalizes `Filesystem` to include its Readable,
   Writable, and Queryable service parents without resolution ranking.
7. A provider or implementation may refine to a smaller service row and may
   remove operational possibilities; it may never widen a pinned ceiling.
8. A stronger prover cannot change an exported service/operational contract ID.
9. Internal omitted service and operational fields reach finite fixed points
   across a recursive call component.
10. Recoverable errors remain result cases; no parallel failure effect is
    introduced.
11. An unmarked call has a statically known envelope that guarantees neither
    suspension nor blocking.
12. A call through a requirement that permits both axes requires
    `suspend block`, even when one selected provider is currently narrower.
13. A `suspend` call nested inside an argument or operator rejects before
    continuation lowering.
14. A bodyless operation may synchronously call a boundary binding only when
    its `invokes` ceiling names that binding.
15. A deferred registration root carries the concrete selected conformance's
    envelope without adding a synchronous edge to the registration call.
16. A realized synchronous component-boundary cycle rejects.

## Deferred, explicitly

- General trace propositions, deadline/starvation contracts, and entailment
  between decision 23's opaque progress profiles.
- General parametric work functions and target WCET. Canonical-IR fuel,
  restricted fixed-work entry/segment checking, and attributed response
  outcomes follow
  [`canonical_ir_fuel_and_resource_provisioning.md`](canonical_ir_fuel_and_resource_provisioning.md).
- Additional operational-clause declarations.
- Named service-row variables beyond the concrete envelope substitution used
  by `invokes`.
- Fixed-stack park/resume lowering and suspension-safe loans. WCSU-derived
  `StackPlan` owns capacity.
- Component-version budgets and admission mechanics beyond the pinned-row law.
- Byte-information units as a possible units-family-zero customer; decision 19
  still requires explicit scale conversion when a consumer expects canonical
  bytes.

## Review laws

Two general review laws were established repeatedly while deriving this model:

> Coincidence of projections in a blessed instance is not identity of axes.

> Every published identity is normalizer-owned; the prover only gates.

They are architecture tests, not rhetoric. The first prevents reach from being
identified with suspension, provider-origin with provider-locality, or
authority with trust. The second prevents solver evolution from changing
interface meaning.
