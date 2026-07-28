# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Question numbers remain
stable when a resolved entry is removed, so gaps are intentional.

Last pruned: 2026-07-28.

## 11. What is the next wire-family, presence, and evolution contract?

`compact_binary` now has generated scalar, bounded repeated-scalar,
length-prefixed byte/text, and one-level nested runtime codecs. Decoding is
canonical and fail-closed, and the build publishes adjacent-era compatibility
and migration verdicts. The remaining task text used to group “additional
encoding families” and “version negotiation” as engineering, but their runtime
shape depends on language policy that is still explicitly open in chapter 21.

Decide:

- which presence model ships first (optional, required, defaulted, or an
  explicit sum), and whether absence is a wire fact, a runtime-value fact, or
  both;
- whether unknown fields reject, skip, or survive round trips, and whether that
  choice belongs to the grammar policy, schema, or decode call;
- how a decoder selects a historical era and invokes typed migration machines
  without a builtin `Versioned<T>` container or hidden dynamic dispatch;
- when a type/presence change may keep its field number, when it requires a new
  number, and which migration declaration discharges the compatibility report;
- where publish-time predecessor comparison runs and which verdicts package or
  deployment policy may reject; and
- which grammar follows `compact_binary`, including whether ecosystem
  compatibility (for example protobuf unknown-field behavior) is a distinct
  policy rather than a mode of the native grammar.

Recommendation: keep `compact_binary` v0 strict and current-era-only until the
selection contract is explicit. Put unknown-field and canonicalization behavior
on the grammar policy, express presence through ordinary declared value shapes
rather than implicit nullability, and use checked adjacent-era migration
machines selected by a boundary package. Publish the existing compatibility
artifact as input to package policy; do not infer a universal optional/default
representation, silently skip unknown fields, or manufacture runtime version
dispatch before these choices are settled.

## 12. How does checked source obtain and register a sealed external entry reference?

`boundary machine` already declares an exported callable, and normalized
`CallPlan + StatePlan` data already drives ordinary inbound ABI lowering.
Static artifact derivation may retain an entry identity privately. The Windows
WndProc customer requires the stronger operation that was previously deferred:
`RegisterClassEx` stores a callback value and invokes it later. Omega cannot
currently turn a selected machine into a runtime value, and exposing its code
address would bypass requirement identity, forward-edge integrity, effects,
and external-root accounting.

Decide:

- which existing requirement/satisfier relationship authorizes derivation of a
  callback reference, and whether the source-visible carrier belongs to the
  same sealed satisfier-identity family as local dynamic dispatch while retaining a
  distinct external-entry lowering;
- how the carrier binds requirement identity, selected satisfier, evaluated
  boundary-entry plan, artifact/version identity, and permitted audience
  without exposing a numeric address or a public constructor;
- whether registration borrows a reusable immutable entry reference or consumes
  a scoped registration authority, and which linear receipt represents the
  OS-held callback until unregistration/quiescence;
- when registration adds the callback to the external-root ledger, how effects,
  stack/work/state ceilings remain visible while the foreign caller owns the
  edge, and how replacement or revocation removes that root safely;
- how the private ABI lowering materializes the native callback pointer only
  inside the admitted foreign binding, including callback-specific context,
  lifetime, thread, and reentrancy contracts;
- how each callback entry mode declares whether it continues the current stack
  chain or starts a distinct activation with its own `StackPlan`, without
  assuming those are the only modes a future target may define;
- how a foreign provider publishes a complete upper bound on which entries may
  synchronously invoke which registered callbacks, with missing evidence
  conservatively meaning any callback rather than an unsound empty set;
- how the checker prefers static callback-cycle exclusion, and how an
  intentionally recursive callback uses chain-owned enforced depth plus a
  protocol-valid overflow disposition when exclusion is impossible; and
- which local descriptor machinery from language-guide chapter 14 may be shared
  without making an external callback merely a `dyn Trait` descriptor entry or
  erasing the internal-versus-external calling distinction.

Recommendation: derive an opaque, non-forgeable entry reference only from an
admitted selected `satisfies` edge whose requirement pins the evaluated
`CallPlan + StatePlan`. Keep the artifact/entry value reusable and immutable;
have the registration boundary consume separate scoped authority and return a
linear registration receipt that owns the foreign-held root until explicit
revocation and quiescence. Materialize a native code pointer only inside that
admitted binding. Make the entry mode determine stack accounting: continuing
entry joins the current mixed call chain, while a new activation receives a
separately provisioned `StackPlan`. Require a provider-complete admitted
callback-invocation upper bound; prefer proving the mixed graph acyclic, and
admit bounded re-entry only through an enforced chain-owned measure whose
overflow action is valid for that protocol. Reuse sealed satisfier identity and
root-ledger machinery, but do not add raw machine addresses, integer conversion,
an arbitrary function-pointer type, or a Win32-specific callback construct.

## 13. What is the portable standalone atomic-fence contract?

Atomic load/store/RMW operations already name the closed C11/Rust ordering
vocabulary and lower exactly on x86-64 and AArch64. Checked assembly separately
exposes target instructions such as x86 `lfence`, `sfence`, and `mfence` with
their actual instruction contracts. A portable language-level fence is still
unsettled: its semantics belong to the cross-activation atomic memory model,
not automatically to any same-named ISA instruction, and MMIO/DMA/device
ordering has different participants and scope.

Decide:

- whether the portable operation is an ordinary intrinsic core machine such as
  `Atomic::fence(order)`, a boundary-operator requirement, or another existing
  operation form, without adding a statement keyword;
- which orderings are legal (`Acquire`, `Release`, `AcqRel`, `SeqCst`, and
  whether `Relaxed` rejects rather than spelling a no-op);
- whether v1 has one process/system participation scope selected by the target
  policy, or exposes an explicit scope value, and how that scope enters
  normalized operation identity and TLA-style memory-model export;
- how the checker relates a fence to surrounding atomic observations so it can
  establish synchronization without pretending that a fence alone publishes
  arbitrary ordinary memory;
- which target policy selects the exact x86-64/AArch64 realization and evidence,
  including legal zero-instruction acquire/release cases, without source code
  naming target instructions; and
- whether compiler-only ordering barriers and device/MMIO/DMA visibility
  barriers remain separate core/provider operations with their own contracts,
  rather than modes of the atomic fence.

Recommendation: make the portable fence an ordinary compiler-known core atomic
operation over the existing `MemoryOrdering` data. Admit
`Acquire | Release | AcqRel | SeqCst`, reject `Relaxed`, and give v1 one
target-policy-selected cross-activation scope rather than forecasting a scope
hierarchy. Keep checked ISA fences available for target/provider code and keep
device/DMA visibility plus compiler-only barriers separate. The normalized
atomic operation records the portable order; target lowering supplies a
validated realization, including no emitted instruction where the target
memory model proves that sufficient.

## 14. How does a foreign contract declare retained data-pointer lifetime?

The extern model already distinguishes borrowed-out, borrowed-in, transferred,
and opaque-handle pointer relationships. Borrowed-out is intentionally
call-scoped: a checked adapter may lend a slice to one synchronous native call,
and the borrow ends when that call returns. Real APIs also retain pointers for
asynchronous work, registration, or later callbacks. The checked IR currently
has no normalized contract fact that distinguishes those APIs, so it cannot
reject a call-scoped pointer passed to a retaining leaf without guessing from
ABI shape, suspension, or a raw address.

This is the data-lifetime sibling of question 12, not the same decision. A
sealed external entry reference governs foreign control entering Omega;
retention governs foreign custody of Omega storage after an outbound call.
Some APIs use both and need two independently auditable contracts.

Decide:

- which existing boundary declaration owns the call-scoped-versus-retaining
  fact, and whether it is selected per pointer parameter, per return, or by a
  named registration protocol;
- how retained read, retained write, ownership transfer, and foreign allocation
  differ without turning one pointer annotation into a grab bag;
- which pinned `Extent` loan or transferred allocation must accompany a
  retained pointer, and how the foreign contract binds the exact range,
  polarity, lifetime, provenance, and permitted service reach;
- which linear receipt represents the foreign-held loan, how completion,
  cancellation, unregistration, or process teardown returns it, and which
  quiescence evidence is required before reuse;
- how a checked adapter proves that a call-scoped borrow cannot escape through
  a retaining contract, including indirect provider calls; and
- which residual claims are proved from checked providers versus accepted under
  a boundary receipt, with missing or opaque lifetime evidence failing closed.

Recommendation: keep pointer representation and ABI classification separate
from lifetime. Put a normalized, per-parameter foreign-use contract on the
ordinary boundary requirement, with call-scoped borrow as the strict default.
A retaining contract must consume or borrow an explicit pinned loan and return
a linear custody/registration receipt whose completion releases that exact
range. Reuse `Extent`, external-loan, provider-admission, and quiescence
machinery; do not infer retention from `suspends`, `blocks`, pointer shape, or
the fact that a native function happens to return later.

## 15. What is the public boundary write-frame clause spelling?

Omega already computes normalized body write frames and uses them to preserve
facts across calls. Boundary requirements need an authored frame because no
body exists from which to infer one. The semantics are settled: the clause
names the complete set of places the call may mutate; omission means an empty
frame; checked implementations must remain within it; and frame evidence is
part of the public contract rather than private proof detail.

The current guide spells this clause `stores`, but explicitly treats that word
as provisional. It now also conflicts with the authority-flow report verb
`Stores`, which means retaining authority beyond the call rather than mutating
a place.

Decide:

- whether the clause is named `writes`, `modifies`, `stores`, or another single
  verb, and whether the same spelling applies to requirements and explicit
  implementation refinements;
- the exact path-list grammar, including multiple paths, indexed/ranged places,
  parameter-relative paths, and whether braces or commas are used;
- how an explicitly empty frame is written when useful for documentation, while
  ordinary omission continues to mean no writes;
- whether whole-object entries subsume descendants during normalization and how
  diagnostics present that relationship; and
- whether any non-memory mutation belongs here, or remains represented only by
  service reach, operational ceilings, linear obligations, and postconditions.

Recommendation: rename the provisional clause to `writes` and keep it a plain
machine-contract clause with a comma-separated place list. It says exactly what
the checker needs and avoids colliding with authority retention. Keep effects,
resource consumption, foreign retention, and hardware state out of the write
frame; they already have independent contracts.

## 16. How does a domain owner delegate canonical qualification authority?

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

## 17. What is the normalized bounded-work plan and composition algebra?

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

## 18. What is the reusable hosted-FFI execution and gateway contract?

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

## 19. How are claim-content projections and backing authored?

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
