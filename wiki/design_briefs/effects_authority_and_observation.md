# Design Brief: Service Reach, Operational Ceilings, Authority, And Observation

Revised 2026-07-27 (decision 22 split amendment and direct-call
acknowledgements). This brief defines a service-
reach `effects` row plus independent `suspends` and `blocks` operational
ceilings. `terminates` remains the separate positive progress guarantee settled
by decision 23. It records each axis's propagation/refinement laws and its
relationship to authority and trust. General trace theorems, quantitative
resource entries, service-row polymorphism, and additional operational clauses
remain explicitly deferred.

## The surface

Omega spells service reach and execution behavior separately:

```omega
machine backup(
    src: [u8] in Utf8,
    dst: [u8] in Utf8
) -> BackupResult
   effects Readable + Queryable;
   suspends;
{
}
```

An `effects` row is a `+`-separated set of name-resolved boundary-service
identities such as `Readable`, `Queryable`, `Clock`, or `ProcessExit`.
Boundary traits contribute service reach; ordinary traits do not.

`suspends;` says an invocation may park its current activation. `blocks;` says
it may occupy its worker while waiting. These are independent public may-
ceilings, not service identities and not members of the `effects` row.
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
        path: [u8] in Utf8,
        out: &mut Vec<u8>
    ) -> ReadResult
      suspends;
}
```

The effective service row of that call contains `Readable`, and its suspension
ceiling is true. A wake operation may reach its scheduler service without being
declared `suspends`; reach is trait-granular while temporal behavior is
operation-granular.

## Contract axes

The complete machine contract retains seven independent axes even though only
one is spelled with `effects`:

| Axis | Durable home |
|---|---|
| Service reach | `effects` row |
| Possible suspension | `suspends` clause and suspension plan |
| Possible worker blocking | `blocks` clause and blocking plan |
| Authority possession | capability values, domains, and parameters |
| Trust reach | provider/admission receipts and the trust ledger |
| Positive temporal guarantees | operation/provider contracts and context floors |
| Resource consumption | explicit capabilities and dependent contracts in v1 |
| Failure/control outcomes | return sums, traps, cancellation, and non-return |
| Mutation | ownership, borrows, and state contracts |

Frame size is compiler-derived and reported. Task activation capacity is
declared or proved through task-pool authority. Version retention is a
component/deployment budget. Heap/region capacity is an explicit resource
value. Only frame size is report-only; no unbounded retention becomes
invisible merely because it does not belong in the effects row.

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
suspension, and blocking summaries. Exported machines, boundary operations,
and trait requirements publish each ceiling. An omitted `effects` row there is
empty; omitted `suspends` means never parks; omitted `blocks` means never
blocks a worker. Diagnostics name the violated axis.

Authority possession and permission remain separate. Holding a Writable
capability does not let a machine whose ceiling omits `Writable` exercise it.
Conversely, listing `Writable` does not mint the capability required by the
operation.

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
```

Private omitted fields are the least conservative fixed points over the checked
call graph. Recursive call components compute finite service, suspension, and
blocking fixed points. Calls to local checked machines use checked callee
summaries. Imports, generic/dynamic requirements, and boundary calls use their
pinned requirement ceilings, never facts learned from an eventually selected
provider.

Provider admission is deterministic. A machine compiled against a slot that
`suspends` but does not `block` remains nonblocking; a provider whose checked or
accepted contract blocks fails refinement. A slot declaring both clauses
admits either behavior and its consumers carry both possibilities honestly.

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

The normalized authored service row and the authored suspension/blocking
ceilings are independent parts of an exported machine's semantic contract
identity, requirement-binding identity, and component compatibility surface. Body
inference only checks inclusion on each axis.

> **Published-identity law:** every published identity is owned by a small,
> deterministic normalizer. The prover may gate legality, discharge an
> obligation, or enable erasure/optimization; it may never redefine a
> published identity.

Proof strength therefore cannot silently shrink an exported service row, erase
an operational ceiling, or change a contract hash. Internal legality may
improve when a stronger prover establishes that a path is unreachable, but
exported identity remains authored. Stable syntactic/CFG reachability feeds
normalization; heuristic entailment does not.

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

## Effects and the other axes

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
    ensures heap.remaining >= old(heap.remaining) - max_used(input.len)
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
(decision 18). There is no `fails` clause. Traps, cancellation behavior, and
non-return still appear in the normalized complete machine contract through
their existing policies, sums, and totality/productivity rules.

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

V1 recognizes service members from boundary-trait declarations and has two
closed operational clauses, `suspends` and `blocks`. Additional operational
clauses, quantitative service entries, and service-row polymorphism are
deferred until their algebras have real customers.

The compiler now uses the semantic model directly: suspension and blocking use
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
effect-row field or fingerprint input. The obsolete
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

## Deferred, explicitly

- General trace propositions, deadline/starvation contracts, and entailment
  between decision 23's opaque progress profiles.
- The normalized abstract-work plan and sequential/branch/SCC composition
  algebra in `OWNER_QUESTIONS.md` #16.
- Additional operational-clause declarations.
- Service-row polymorphism and higher-order/callback row variables.
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
