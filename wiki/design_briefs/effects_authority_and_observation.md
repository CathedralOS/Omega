# Design Brief: Service Reach, Synchronous Invocation, Authority, And Observation

Revised 2026-08-28. This brief defines a service-reach `reaches` row, a direct
synchronous `invokes` ceiling, independent `suspends` and `blocks` operational
ceilings, and the guarded `crashes` ceiling. `terminates` remains the separate
positive progress guarantee settled by decision 23. It records each axis's
propagation/refinement laws and its relationship to authority and trust.
General trace theorems, quantitative resource entries, named or
ordinary-export service-row polymorphism, and additional operational clauses
remain explicitly deferred. The one accepted abstract-row slice is a bounded
row owned and closed by an installation-bound provider requirement.

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
        out: &write [u8]
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
| Resource consumption | explicit capabilities and dependent contracts |
| Recoverable failure | returned sums and case-specific contracts |
| Crash possibility | guarded `crashes` buckets and crash plans |
| Mutation | ownership, borrows, and state contracts |

Mutation authority is itself graded. `&mut T` permits observation and mutation;
`&write T` is an exclusive loan over an existing valid `T` that permits only
non-observing writes and content-independent projection. The distinction is
part of the checked signature and artifact identity, but neither borrow kind
adds a service-reach entry. Checked callees preserve the restriction through
their call closure. An opaque provider's claimed non-observation is admitted
unless installation supplies physical isolation evidence.

The first bounded whole-root primitive store is retained as a structural-state
event through target-neutral operations and optimization validation. Integer
literals continue through physical assignment and native store emission;
Boolean literals retain their exact Boolean definition, one-byte referent, and
borrowed-reference placement through independent assignment and native store
emission. IEEE float literals retain their exact raw-bit definition and four-
or eight-byte referent without using a floating register. Object construction
independently replays all three non-observing store families and their bytes;
installation format 73 transports that replay canonically. The classification
prevents scalar dead-code reasoning from
erasing the write; it does not claim a readable
observation of the prior referent or authorize an opaque provider.

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

An installation-bound provider requirement may leave its exact row to the
selected realization while publishing a finite upper bound:

```omega
pub boundary requirement InterruptAcknowledgement::complete(self)
reaches <= MachineControl + PortIo
requires
    self in InterruptAcknowledgement::Pending;
```

The explicit requirement path is the abstract-row identity. The selected provider row
must be a subset of the bound and replaces the symbolic row in the installed
closure. The unresolved row and bound are manifest facts before selection;
the exact provider, operation, and resolved row are manifest facts afterward.
Internal inferred callers in the same installation closure retain a symbolic
dependency on that path. An ordinary callable package or component boundary
cannot export it: the boundary binds the provider first or publishes a fixed
conservative row. Final admission rejects unresolved rows.

The operator does not add Boolean formulas to effect rows. `+` remains set
union and `<=` supplies only the upper bound. Empty, either singleton, and the
whole bound are all structurally possible; semantic provider admission decides
which implementations are legal. Distinct requirements have distinct rows.
Installation lineage, not row equality, binds an interrupt entry, the exact
completion operation that issued its debt, and the later consuming completion.

Internal machines may omit these clauses and receive inferred service,
suspension, blocking, and crash summaries. Exported machines, boundary
operations, trait requirements, and explicit top-level boundary requirements
publish each ceiling. An omitted `reaches`
row there is empty; omitted `suspends` means never parks; omitted `blocks`
means never blocks a worker; and an omitted crash cause is forbidden.
Diagnostics name the violated axis.

On a private body, an omitted `reaches` clause requests inference, while an
authored memberless clause is an explicit published empty ceiling. Syntax
retains each clause keyword and member occurrence; resolution binds every
member once to its exact boundary-trait symbol. The normalized semantic row is
the parent closure of those authored identities plus services contributed by
the invocation ceiling. Duplicate authored members remain separate source
occurrences but do not duplicate semantic set members. Package review uses the
authored member spans, or the keyword span for an empty row, and invents no
location for inference, invocation contribution, or parent closure.

Authority possession and permission remain separate. Holding a Writable
capability does not let a machine whose ceiling omits `Writable` exercise it.
Conversely, listing `Writable` does not mint the capability required by the
operation.

### Guarded crash ceilings

Crash possibility is the flow-sensitive may-axis:

```omega
machine divide(n: i32, d: i32) -> i32
crashes Trap
    d == 0
    n == i32::Minimum && d == -1
crashes Abort
    configuration_invalid
{
}
```

Each clause names one cause. Its indented entries are alternative routes, so the
clause denotes their disjunction. An empty route list denotes `true`. Omitting a
cause from a published contract forbids that cause. Private omission infers a
summary.

Every route guard is a total proof expression. Direct Trapping arithmetic is
therefore forbidden in the guard and cannot create a crash route by being
"evaluated." Fixed integers and addresses use explicit proof-`Int` embedding;
floats use `FloatMeaning`. The body operation alone creates the derived crash
site, under the primitive-specific guard defined by the compiler-owned
denotation catalog. See
[Total Specification Arithmetic](total_specification_arithmetic.md).

The initial cause identities are `Trap` and `Abort`. Cause identity controls
policy and lowering; both are no-successor, no-cleanup exits.

For each derived crash site, let `D` be its path-conditioned guard. The authored
contract supplies route guards `C_i`. Coverage is:

```text
D implies OR_i(C_i)
```

The declaration is a public ceiling, not an exact body summary. A body may
derive narrower guards without changing exported identity; an uncovered site
rejects. Across calls, substitute arguments into the published routes and
discard every route disproved by current facts. Surviving routes propagate
upward; disproving all routes removes that cause at that invocation.

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
exact incoming path conjunction, a separate source-independent structural
consequence set, and the independently surviving cause/guard buckets.
Concrete false routes disappear, concrete true
routes normalize to unconditional alternatives, unknown routes are retained in
the caller's positional parameter namespace, and a call with no survivors
remains explicit crash-free evidence. Same-unit body-summary selection is
live. Callable trait requirements and unresolved compile-time machine
parameters now use source-independent crash-contract capsules that pin their
published buckets to the complete normalized callable-contract fingerprint.
Same-unit private fixed points now retain a temporary canonical predicate tree
through nonrecursive edges and widen recursive SCC edges to unconditional
cause buckets. Separately compiled import capsules remain blocked until
the semantic import/export carrier and its certificate binding are specified.

Checked ownership also records a canonical lower bound of claims and invariant
windows known to be live at each crash site. That lower bound is audit and
diagnostic evidence. It cannot establish that unlisted state, external storage,
devices, or peers remain valid, and therefore cannot license survivor
execution.

Fault-tolerant continuation is a separate architectural proof. It requires a
closed-custody component boundary, an explicit per-resource owner-death
protocol, or an external reset/transaction protocol, plus a target realization
of the isolation and restart plan. In the absence of that independent proof an
uncontained crash terminates the execution domain. Ordinary crash contracts do
not carry containment scopes or recovery promises.

## Composition algebra

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

The bounded installation slice does not weaken that rule. Its symbolic row is
admitted only inside the owning installation closure, whose manifest already
states the bound. It may propagate through compiler-derived internal call
metadata there, but it cannot become an unpinned ordinary import. Installation
substitutes the selected row throughout that closure before final admission.

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

Package review preserves the exact authored `suspends` and `blocks` keyword
spans separately from those semantic ceilings. Syntax, resolved, and typed
owners carry them through copying, specialization, and synthesized trait
defaults; the review projection joins them to the checked interface and rejects
missing or contradictory custody. Omission and inference have no authored
location. Because an exported machine's operational fact is its public may-
ceiling, review must not describe a permissive ceiling as an observation that
the current body actually suspended or blocked.

> **Published-identity law:** every published identity is owned by a small,
> deterministic normalizer. The prover may gate legality, discharge an
> obligation, or enable erasure/optimization; it may never redefine a
> published identity.

Proof strength therefore cannot silently shrink an exported service row, erase
an operational ceiling, rewrite a published crash guard, or change a contract
hash. Internal legality may improve when a stronger prover
establishes that a path is unreachable, but exported identity remains authored.
Stable syntactic/CFG reachability feeds normalization; heuristic entailment
does not.

This is the service/operational-contract instance of decision 19's
normalization-versus-entailment law and the component requirement-binding admission
law.

## No laundering, masking, or handlers

The language has no service subtraction, masking, scoped allowance, or algebraic effect
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

### Package service review and installed terminal authority

`reaches` and physical provider bindings answer different questions. The
normalized service row is stable across installations and supports review of a
dependency before selecting its providers. The installed terminal-authority
row is derived afterward by walking the exact selected-provider closure to
target-qualified imports, syscalls, compiler intrinsics, firmware/vtable
operations, and checked physical operations. Neither replaces the other.

The receiving D41 realization authority classifies a terminal mechanism under
its accepted versioned target policy. The key is a closed role-tagged post-
normalization sum rather than a flat tuple: structural compiler intrinsic;
target ABI, syscall, and checked argument contract; normalized foreign locator
and admitted contract; exact firmware/table identity and receiver contract; or
an exact checked physical-operation catalog entry. Each role carries only its
meaningful coordinates and its discriminant enters identity. Provider context
and service schema remain join inputs and cannot narrow physical authority.

The exact service identity and normalized schema map to permitted terminal
classes; the structural mechanism maps to exercised classes; every exercised
class must be contained in the permitted set. Policy may be partial over the
operating system's coordinate universe but is demand-complete for an admitted
artifact: every terminal leaf has exactly one explicit row, including explicit
empty rows. Unknown, duplicate, cyclic, string-only, or missing leaves fail
closed. Names, filenames, aliases, package roles, reviewer judgments, and risk
labels are never authority identity.

The first production receiving-policy join covers the closed compiler-
intrinsic family. Native realization explicitly receives the accepted policy,
classifies each demanded compiler builtin before target settlement, and carries
the policy version and strong commitment in the authority-free native artifact
identity. Exact-policy replay rejects substitution. This records physical
classification policy only; it does not prove service/schema containment or
mint provider-execution admission evidence.

The direct normalized-foreign rung uses the same versioned policy through one
role-tagged `TerminalMechanismIdentity` (introduced in version 2; version 3 adds
the Linux write-byte ProcessOutput row). Each explicit foreign row binds
the exact selected target and normalized locator identity to the strong
contract commitment of its canonical admitted `BoundaryEntryPlan`; a provider
report fingerprint is never the contract key. Native realization classifies
every directly demanded PE-by-name, PE-by-ordinal, versioned-ELF, or Mach-O
import before provider settlement. Missing or duplicate policy rows, locator
or contract substitution, wrong target, duplicate selected/external rows, and
legacy string-backed imports fail closed. The accepted whole-table identity
remains in native-artifact replay. Complete selected-provider-closure traversal
and exact service/schema containment remain the next distinct join.

Receiving-policy version 4 adds the first checked-physical leaf without
flattening it into either of those roles. An x86 `PortWrite` reached through
one exact installed selected checked adapter is identified by its complete
deployment profile and port; the written byte remains event data rather than
physical-mechanism identity. Closure review independently requires the
operation's service in the verified machine ceiling, an explicit exact
physical-policy row classifying it as `PortIo`, and `PortIo` permission on the
enclosing selected requirement and schema. A direct root has no such provider
context and rejects. So do a non-x86 profile, target or port substitution, and
two distinct checked mechanisms under the same bounded requirement. This
retains classification evidence only; it does not perform or authorize port
I/O or mint provider, installation, or invocation custody.

Receiving-policy version 5 adds the exact direct-syscall role as a
classification carrier. Its key commits the complete target profile, the
`u32` syscall number (including valid number zero), and a distinct nonempty
identity for the compiler-checked argument contract. That contract identity is
not the boundary calling-plan commitment: it must cover the conservative
unconstrained argument case or the exact retained constants, ranges, handle
provenance, and other constraints used to narrow authority. Explicit rows are
canonicalized into whole-policy identity; target, number, or contract
substitution and absent rows fail closed; the current table also rejects every
profile outside the evaluated Linux x86-64/AArch64 syscall domain.

The first checked-contract rung is now live. Provider settlement derives a
domain-separated conservative identity from the verified Terminal boundary:
the complete scalar carriers plus each structural parameter's canonical
position, multiplicity, access, and exact carrier identity. Every retained call
occurrence must match that arity and structural access. Root or projected
structural qualifications and boundary requirements reject until the abstract
plan retains their stable semantic-domain declarations; module-local domain IDs
never enter the digest. The accepted identity treats every runtime value
admitted by those unqualified carriers as reachable; it claims no constant,
range, handle-provenance, or descriptor narrowing. Settlement rejoins one
selected syscall row to one retained external row with the exact target and
number, classifies the derived mechanism, and passes that same mechanism to
closure review. The reviewer consumes it only when the selected profile and
`u32` number match exactly. Missing, duplicate, unsupported, unclassified, or
substituted coordinates fail closed. No service or method spelling can
synthesize the identity, and this rung does not resolve filesystem descriptor
confinement or the unsettled authority classes in `OWNER_QUESTIONS.md` Q4.

A row publishes the union over every authority reachable through its argument
values. Narrowing requires retained compiler-checked constants, ranges, handle
provenance, or another exact constraint proof whose identity enters the
mechanism role. A narrower service name is not such proof. An explicit empty
row says only that no dangerous-authority class is exercised; it is not a
purity, side-effect, foreign-code-trust, or provider-custody claim.

The closed classes are filesystem content read/write, filesystem metadata
query/mutation, directory enumeration, filesystem namespace mutation, process
output, process termination, machine control, port I/O, interrupt control,
interrupt entry, and root-memory access. Exact service and mechanism identity
remain retained beside those groups. Broad historical `Filesystem` and
`Process` risk labels are review summaries rather than terminal-policy keys.

Cross-platform provider choice is expressed by separate realizations
satisfying one portable requirement. It is not expressed by adding Linux,
Windows, and firmware mechanisms with `+`, because `+` is ordinary reach union.
For filesystem service rows, the portable authority groups distinguish at
least content read, content write, metadata query, directory enumeration,
namespace mutation, and metadata mutation while exact operation identity
remains retained. Until raw integer descriptors migrate to typed unforgeable
handles, those groups bound operations but make no object-confinement claim.
The representation layer exposes that closed six-facet set as an explicit
filesystem-policy authoring vocabulary. It canonicalizes into the same
terminal classes consumed by selected-closure containment while still
requiring the consumer to provide exact schema and requirement coordinates;
the helper does not inspect service paths or readable method names.
The current real `FilesystemHost` schema has explicit consumer-policy coverage
for the 36 requirements whose class unions follow from these rules, including
all six facets for flag-polymorphic opens. Fourteen control/lifecycle cohorts
remain deliberately unmapped pending `OWNER_QUESTIONS.md` Q4; neither an empty
disposition nor a filesystem class is inferred from their names. This policy
partition is separate from target syscall/import mechanism classification and
from any future descriptor-confinement claim.

The current filename-and-trait keyed dangerous-authority classifier is a
transitional implementation defect. Receiving target policy now classifies the
closed compiler-intrinsic family and direct normalized PE-by-name,
PE-by-ordinal, versioned-ELF, and Mach-O imports without admitting string-backed
bootstrap rows. This physical classification does not replace the transitional
table by itself. The obsolete filename table is removed only when complete
selected-provider-closure traversal joins those leaves to an exact
service/schema permission table plus binding-derived containment, before
filesystem faceting, so the migration never expands the filename classifier.

The first exact package-permission carrier is explicit consumer input, not
discovery output. An accepted semantic binding may retain rows from its complete
normalized schema digest and one exact requirement identity to canonical
permitted classes. Checked replay rejoins each supplied requirement to exactly
one method; a partial table is legal at this layer because the installed
closure, not binding construction, owns demand-completeness. A candidate
Console or filesystem binding carries no permission unless the consumer adds
one explicitly.

Package review retains each supplied row as its own blocking obligation with
the resolved service nominal, schema digest, requirement identity, classes, and
service/requirement source custody. A broad `Process` row therefore cannot
stand in for `Console::exit_process -> ProcessTermination`, and accepting the
broad row does not create the exact permission. Direct native compilation
cross-checks resolved package rows against the receiving permission policy;
missing or changed rows and duplicates across bindings reject. Root-policy-to-
native transport and complete legacy-row replacement remain separate unfinished
joins, so the filename classifier is still present but cannot feed the exact
table.

The in-memory transport boundary is now explicit. Accepted ordinary closure
evidence projects a canonical exact accepted-permission set solely from
terminal-permission obligations accepted by fresh root-policy replay. A
retained Terminal native proposal carries the checked package rows across
frontend destruction. Re-entry requires those rows to equal the independently
accepted set, then checks that set against the distinct receiving policy with
the same validator as direct native compilation. The manager-owned entrypoint
also matches the retained report's package production manifest to the accepted
root's exact dependency closure, source-consumption commitment, selected build-
machine identity, deterministic invocation-local evaluation usage, build-
observation identity, and target before it derives the set from opaque
evidence. Aggregate review-sponsor ceilings and session-wide peaks remain
orchestration custody rather than invocation identity. Compiler-report
custody separately rejects a retained proposal whose profile or native target
differs from that production subject. Thus neither a freely constructed policy
nor a reconstructed proposal grants package admission. Terminal Psi does not
acquire policy authority, and unrelated receiving-policy rows remain legal.
Executable application review and manager re-entry now cover exact success and
source-consumption, build-observation, and coordinated proposal/policy
substitution. The generated `builder.roots.bind` marker retains its authored
`bind` span, while later expression normalization may recover location only
from one exact private authored selection. A build with no filesystem reach
has a canonical empty Output tree even without a physical sponsor; this keeps
sponsored review and ordinary production observation identities equal without
discarding Output custody.
The command-line project workflow still stops at raw package inputs; it must
gain accepted-evidence/root-policy orchestration and invoke this route
before the join is automatic in production.

### Resources

Resource bounds remain contracts on explicit resource capabilities:

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
`Trap` and `Abort` routes and their predicates.
Calling a process-exit service may contribute both `ProcessExit` reach and an
`Abort` route, but neither axis is reconstructed from the other.

### Totality and positive liveness

`terminates` guarantees eventual terminal progress conditional on explicit
requirements and the progress premises pinned by reached operation/provider
contracts. It does not prove no suspension, fairness, a deadline, eventual
wakeup, or starvation freedom by itself. Omission of `suspends` or `blocks`
proves only the corresponding negative operational guarantee.

Decision 23 represents positive progress premises as opaque, sealed domains
explicitly classified by their owner with `satisfies ProgressProfile`. Exact
`established by` boundary requirements use grant receipts and participate in
admission. Operation termination contracts author premise schemas; checked
calls instantiate them rather than inferring progress from reach, suspension,
or parameter mention. Profiles never entail proof facts. General trace theorems
and profile entailment remain deferred. See
[Termination, Ranking, And Progress](termination_ranking_and_progress.md).

## Staging and extensibility

The current language recognizes service members from boundary-trait
declarations and has the closed operational clauses `suspends`, `blocks`, and
`crashes`. Additional operational clauses, quantitative service entries, and
named or ordinary-export service-row polymorphism are deferred until their
algebras have real customers. The bounded installation-row slice above remains
path-keyed, non-exportable, and closed by root admission.

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
effect-row field or fingerprint input. Resolved and typed machine records also
carry only normalized supply mode, not a parallel `boundary` compatibility
bit. The obsolete
`EffectRowId`/`EffectRowTable` carrier and global lowercase service-name/u64
table are deleted. Typed machines and state signatures retain only
symbol-resolved `ServiceReachRowId` values. Authored identifiers end in a
syntax-to-resolved normalization sidecar after symbol assignment, row
construction, and source-facing diagnostics; published resolved and typed
records contain no parallel spelling contract.
Build-script admission consumes compiler-owned Build-facet effects rather than
granting special status to runtime library services. Source reads, staged-output
writes, generated-source publication, and build logging are operations on the
compiler-issued `BuildSource`, `BuildOutput`, and `BuildLog` facets. Their exact
demands and observations compose through helpers into the free build root, and
the filesystem sponsor enforces physical roots, limits, and custody. An ordinary
boundary service reached from a build remains an ordinary runtime service and
is not admitted because it is named `FilesystemHost`, `Console`, or anything
similar. Unknown service identifiers continue to resolve normally, with no
global name table; build admission rejects every runtime boundary service and
recognizes only exact compiler-owned Build-facet calls.

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
17. A realization outside an installation row's declared upper bound rejects;
    the bound itself grants no service authority.
18. A bounded abstract row may propagate through its owning installation
    closure but rejects at an ordinary callable package/component boundary and
    at final admission if unresolved.
19. Preselection manifests report the abstract row and bound; selected
    manifests additionally report the exact provider, operation, resolved row,
    and refinement evidence.
20. Equal entry and completion rows do not establish interrupt-protocol
    coherence; mismatched provider execution, operation, policy, or token
    lineage rejects independently.
21. Reach rows retain union and subset laws only; exclusive-or, negation,
    subtraction, and lower-bound formulas are not source contracts.

## Deferred, explicitly

- General trace propositions, deadline/starvation contracts, and entailment
  between decision 23's opaque progress profiles.
- General parametric work functions and target WCET. Canonical-IR fuel,
  restricted fixed-work entry/segment checking, and attributed response
  outcomes follow
  [`canonical_ir_fuel_and_resource_provisioning.md`](canonical_ir_fuel_and_resource_provisioning.md).
- Additional operational-clause declarations.
- Named or ordinary-export service-row variables beyond the concrete envelope
  substitution used by `invokes` and per-requirement bounded installation rows.
- Implement the settled call-keyed Terminal suspension row and fixed-stack
  park/resume lowering; widen suspension-safe loans only with exact crossing
  evidence. WCSU-derived `StackPlan` owns capacity.
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
