# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-07-30.

## 1. How does a domain owner delegate canonical qualification authority?

`RepresentationQualification<Q>` now opens a bodyless domain only when its
satisfier is declared in the domain-owning package. The semantic rule also
allows an explicit owner-authorized delegate, but no source declaration says
which package receives that authority. Import visibility, dependency aliases,
trait visibility, and matching names cannot safely imply delegation: each is
caller-controlled or too broad, while canonical qualification licenses erased
fact establishment.

Decide:

- whether delegation is authored on the domain declaration, in an owner package
  manifest, through an owner-owned boundary requirement, or through another
  existing declaration relationship;
- how the delegate package is identified across dependency aliases, relocation,
  versioning, and separate compilation without treating a filesystem path as
  semantic identity;
- whether authority targets one exact domain, a transparent alias, a domain
  subtree, or a broader package surface, and whether the carrier and canonical
  satisfier identity are pinned independently;
- whether delegation is public and re-exportable, explicitly non-transitive, or
  may itself be delegated under a separately authored grant;
- how compatibility and revocation work when either package evolves, including
  whether changing the grant is a semantic-interface break; and
- which normalized grant identity appears beside the domain and satisfier in
  checked qualification evidence and package-admission reports.

Recommendation: use an owner-authored, non-transitive grant naming one exact
atomic bodyless domain and one normalized package identity. The grant should
be part of the owner's semantic interface, copied into the dependent package's
admission inputs, and retained in every delegated qualification-use artifact.
Do not derive authority from imports, build dependency aliases, public trait
visibility, carrier ownership, or the presence of a conformer. Until this
surface settles, third-party canonical satisfiers must continue to fail closed.

## 2. What is the normalized bounded-work plan and composition algebra?

WCSU gives Omega a static space bound: a closed activation can reserve one
fixed, nonmoving stack and retain it across suspension. It says nothing about
execution work. Three independent customers now need that time-dual:
resource-bounded interrupt roots, maximum work between semantic safe points,
and the deterministic cost vocabulary used by build-time evaluation. Reusing
the phrase "bounded work" without one normalized plan would let those lanes
quietly charge different units and compose loops differently.

The required distinction is already clear. Abstract work is deterministic and
target-parameterized; it is not wall-clock time. Sequential work adds, branch
work takes the maximum reachable arm, and an SCC requires the same authored
ranking/measure discipline used for termination. A blocking edge without a
finite wait contract does not become a large work number: semantic response is
unbounded for a named reason. A target may convert work to time only through a
derived or admitted timing model whose trust provenance remains visible.

`omega-external-roots` already contains a provider-local `FixedWork` composer
and a `StructuralWorkResourceColumn`. That is useful implementation evidence,
not a second work semantics: this question decides how it migrates into the
general machine/control-flow plan, gains measured SCC and selected-point
queries, and shares one cost vocabulary with the other customers.

Decide:

- the canonical abstract primitive-cost vocabulary and which target facts may
  parameterize it without making acceptance depend on host load or elapsed
  time;
- the exact normalized `WorkPlan` shape, including ceiling, realized work,
  evidence, and the path/cycle witness retained for a maximum or unbounded
  result;
- composition across calls, branches, loops, mutually recursive SCCs, indirect
  dispatch envelopes, cleanup edges, interrupts, and component boundaries;
- how to query maximum work between selected semantic points without making
  every loop backedge or state transition an implicit scheduling safe point;
- how external waits and foreign calls contribute a finite ceiling, a named
  unbounded edge, or a separately retained completion obligation;
- how target timing conversion records cache/frequency/platform premises and
  composes trust by the weakest input; and
- which common cost algebra is shared with build-time metering while still
  allowing build evaluation to report realized work without requiring a static
  hard ceiling.

Recommendation: add one compiler-normalized `WorkPlan` over deterministic
abstract steps. Sum along an edge/path, take maxima at alternatives, and use an
authored measure for repeated SCC composition. Preserve attribution instead of
collapsing an unbounded result to bare infinity. Keep work, external wait, and
wall-clock conversion as distinct report columns; a timing number is only as
trusted as its weakest timing premise. Do not use elapsed compiler time, infer
safe points from optimizer placement, or make build evaluation's optional
budget policy the language's work semantics.

## 3. What is the reusable hosted-FFI execution and gateway contract?

An opaque native function supplies neither checked WCSU nor Omega's blocking,
cancellation, retention, callback, and failure guarantees. A direct adapter can
run it on the current activation stack under an admitted foreign-call plan. A
gateway can instead suspend the Omega caller and execute the function on a
bounded pool of native worker stacks. That confines stack accounting and keeps
native blocking off Omega scheduler workers, but relocates rather than removes
unboundedness: a hung call retains one worker, may retain loans indefinitely,
and can exhaust the shared pool.

This choice cannot be a compiler heuristic. Some foreign APIs require the
initiating thread, thread-local state, a UI/COM apartment, or synchronous
callbacks. Others are best served by an ordinary worker gateway. Hostile code
needs a process or hardware protection boundary rather than a declared stack
number. Guarded stacks detect ordinary exhaustion but prove containment, not
successful completion.

Decide:

- how a binding selects direct execution, a pinned or general native-worker
  gateway, or an isolated process without creating a second component model;
- the normalized foreign-call plan for direct execution, including admitted
  same-stack contribution, blocking/failure behavior, callback topology, and
  target calling plan;
- the normalized gateway resource plan: worker count, worker stack provision,
  queue capacity, admission/exhaustion behavior, scheduling partitions,
  cancellation disposition, retained-loan custody, and shutdown/quiescence;
- whether the common API exposes both bounded `try_submit -> Accepted | Busy`
  and possibly-unbounded `suspend submit`, and how moved arguments return on
  failed admission;
- how cancellation distinguishes native acknowledged cancellation, legal
  detachment under gateway-owned storage, deferred finalization, and
  process-level termination;
- how reports keep time-to-safe-point, operation completion, cancellation
  finalization, retained-resource release, and gateway admission latency
  separate and attributed; and
- which stack-guard/failure-domain facts are derived or admitted per target,
  without claiming that a guard makes arbitrary in-process native corruption
  recoverable.

Recommendation: model a gateway as an ordinary boundary provider backed by a
bounded native-worker resource, not as a new call kind. Let binding packages
select the execution disposition explicitly. Provide bounded admission and
backpressure, retain exact loan/custody paths until native completion, and
partition unknown or blocking libraries away from latency-sensitive platform
services. Treat a guard as enforced stack containment and an overflow as an
abnormal exit; it does not prove the foreign call's WCSU or permit resuming
possibly corrupted in-process state. Keep direct FFI available for audited
leaf calls and require process isolation for hostile native code.

## 4. How are claim-content projections and backing authored?

The resource semantics are settled: content is independent of multiplicity,
each content-bearing qualification publishes one normalized projection into
the compiler-owned `Indivisible | Interval<Scalar>` vocabulary, admission
supplies backing in the same algebra, and checked transformations prove n-ary
conservation plus authorized retirement. The current source language does not
say how any of those facts are declared. Defaulting every linear claim to
`Indivisible` would incorrectly turn ordinary ownership debt into resource
content, while recognizing particular domain or field names would make
authority depend on convention.

Decide:

- how a domain owner marks one exact qualification as content-bearing, and
  whether an omitted algebra means ordinary non-content qualification or an
  `Indivisible` content claim;
- the source grammar for selecting `Indivisible` or `Interval<Scalar>`, naming
  the scalar type and coordinate-space identity, and expressing subject-relative
  half-open bounds;
- how a bodyless requirement or provider result authors algebra-denominated
  backing, including which result claim and admission identity it establishes;
- how checked machines declare or derive authorized retirement and an explicit
  `partitions`-style conservation contract when the ordinary outcome map is not
  sufficient;
- how several independent projections and one joint correspondence-bearing
  projection are distinguished without conflating domain facets; and
- which normalized projection identity is part of the semantic interface so
  separate compilation, versioning, aliases, and proof/debug artifacts agree.

Recommendation: add one owner-only content clause to the atomic qualification
declaration, with omission meaning that the qualification is not content
bearing. Let the clause choose the closed algebra and define a pure projection
over the qualified subject; make `Indivisible` an explicit or clause-local
default, never a default for linearity in general. Use separate requirement
postconditions for admitted backing and authorized retirement, normalize all
references by semantic identity, and keep the authored surface small enough
that the compiler can decide equality, containment, restriction, and separated
composition without executing owner-defined code.

## 5. How are opaque in-process executable dependencies surfaced and refused?

The boundary-provider report already names imported symbols, selected
providers, and admission receipts. That makes an opaque native dependency
auditable, but the root contract and build-profile rejection surface are not
settled. An in-process native binary joins the program's trusted computing base:
an ABI wrapper can validate calls and manage lifetimes, but cannot stop that
binary from writing arbitrary process memory. A process-isolated provider has a
different trust consequence even when it exposes the same abstract service.

Decide:

- whether transitive in-process native use appears in the machine's operational
  effect/reach contract, a separate root trust clause, only the selected-provider
  manifest, or a composed combination that does not conflate service reach with
  trust;
- how the report names the exact provider or binary rather than collapsing all
  native dependencies into one boolean category;
- how target-platform providers already accepted by the deployment profile are
  distinguished from additional third-party binaries without making every
  hosted program carry a useless universal warning;
- how a checked adapter may narrow the public API while remaining unable to
  launder the underlying in-process trust dependency;
- how moving the provider behind a process, address-space, or hardware
  isolation boundary changes the reported dependency to an endpoint rather
  than an in-process TCB expansion; and
- how a safety profile rejects forbidden dependencies before artifact
  production, independently of whether a source author acknowledged them.

Recommendation: retain exact provider identity and trust provenance
transitively, publish a root-level TCB bill of materials, and let build profiles
reject disallowed in-process providers. Treat platform baselines, third-party
in-process binaries, and isolated endpoints as different admitted relationships.
Do not let an ordinary wrapper erase the selected provider's trust class.

## 6. What does contained execution failure do to outstanding obligations?

Process-wide nuclear abort leaves no continuing runtime. A contained activation,
callback, component, or worker may instead be force-terminated while the rest of
the system survives. Execution quiescence then does not imply obligation
quiescence: the dead execution may have held a lock, carried a linear claim,
owned a retained foreign loan, or been responsible for a provider entry pin.
Reclaiming its artifact merely because no instruction is still executing would
silently orphan those obligations.

Decide:

- which obligations are owned by the execution, its component cohort, a stable
  provider ledger, or another named custodian at the instant of forced exit;
- which obligations may be mechanically returned by runtime teardown and which
  require semantic code that can no longer run;
- whether an unresolved obligation poisons the execution, registration,
  component version, isolation domain, or whole process;
- which reclamation and replacement operations remain blocked by that poison,
  and which explicit recovery authority may clear or transfer it;
- how forced-exit reports name the originating execution and every retained
  holding path instead of presenting only a generic non-quiescent status; and
- how this composes with nuclear abort, ordinary edge cleanup, foreign-worker
  failure, callback drain, and component replacement without inventing cleanup
  that did not execute.

Recommendation: separate execution quiescence from obligation quiescence.
Runtime teardown may discharge only obligations whose provider contract
explicitly assigns teardown that authority. Everything else remains attributed,
poisons the owning cohort, and blocks reclamation until an authorized recovery
or a wider failure boundary retires the cohort.

## 7. How are modular concurrency environment premises authored and discharged?

Omega can derive normalized atomic events and concurrent transitions from a
closed machine graph, but a separately compiled package cannot know which
operations its consumers will run concurrently. Whole-program exploration alone
therefore cannot justify a reusable protocol contract. A package must publish
the fact it establishes together with the smallest environment premise under
which the proof holds, and a consumer must discharge that premise when the
package is instantiated or composed.

The premise is not a restatement of the package body or a fixed thread count.
It may constrain which public operations overlap, which atomic locations the
environment may modify, which callback or re-entry edges exist, and which
fairness or progress hypotheses are admitted. A finite exploration bound is
evidence only for that bound unless an authored cutoff theorem connects it to
the unbounded protocol.

Decide:

- the source surface for an open package to declare permitted concurrent
  operations, environment writes, re-entry edges, and positive progress
  assumptions without exposing the internal event graph;
- which premises a checked body can infer and which must be authored at a
  bodyless, imported, generic, dynamic, or otherwise open surface;
- how premises compose through package calls, transparent refinements,
  protocol wrappers, dynamic operational envelopes, and selected providers;
- how a consumer discharges a premise from ownership, access contracts,
  activation topology, provider receipts, or another selected protocol proof;
- how bounded exploration records activation bounds and authored cutoff
  evidence without promoting testing to an unbounded theorem;
- how opaque or admitted providers retain exact trust provenance in the
  resulting proof rather than laundering an assumption into a derived fact;
  and
- how diagnostics connect a failed composition site to the originating
  package assumption and a concrete counterexample trace.

Recommendation: reuse normalized machine contracts and selected-conformance
evidence for an assume/guarantee protocol layer. Infer the smallest premise
where the complete body and activation graph are closed; require an authored
premise at open published surfaces; and make consumers discharge it explicitly
or through derived composition evidence. Keep finite exploration parameters in
the proof artifact, never in semantic contract identity unless the published
protocol itself is deliberately bounded.

## 8. What is the public float-conversion requirement family?

The float record settles conversion semantics but not their public operation
names or signatures. `FloatSemantics` already defines format conversion,
integer-to-float rounding, and exact/trapping/saturating float-to-integer
results. The language record also settles that compatibility `as` is not the
public conversion API, that directed rounding uses separately named operations,
and that arithmetic policies are static qualifications rather than runtime mode
arguments. Publishing guessed names now would freeze a core API that the owning
brief never chose.

Decide:

- whether destination-qualified operations such as `F32::from_f64` and
  `I32::from_f64` are the canonical identities, or whether one generic
  conversion requirement carries source and destination types;
- whether exact, trapping, and saturating float-to-integer behavior is selected
  solely from the destination qualification or appears in distinct requirement
  identities;
- the separately named toward-zero/toward-positive/toward-negative format and
  integer-to-float variants, without introducing a runtime rounding-mode
  parameter;
- how source-visible primitive carrier requirements cite the proof-only
  `FloatSemantics` conversion functions and integer meaning;
- whether same-format conversion is a real identity operation or absent from
  the requirement family; and
- which names replace compatibility `as` in diagnostics and migration canaries.

Recommendation: use destination-qualified, statically typed requirement
identities; let the destination arithmetic-policy qualification select
exact/trapping/saturating result adapters; keep directed rounding as separate
operation names; and omit same-format conversion. This follows the settled
operand-driven provider model without carrying type or policy tags at runtime.
