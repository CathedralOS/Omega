# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-25.

## 1. What is the runtime and object-safety contract for `dyn Trait`?

Closed-world call-site specialization currently makes `&dyn Trait` parameters
execute correctly when every concrete receiver is known at its call site. It
cannot represent a runtime-varying trait value stored in data, passed across a
component boundary, or rebound to one of several satisfiers. The language guide
explicitly leaves the runtime representation and boundary legality open, while
the remaining task requires descriptors that preserve satisfier identity.

Decide:

- whether the stable value is a two-word `{instance, table}` pair whose table
  identity names the satisfier, or carries a separate sealed satisfier/contract
  identity (or component/endpoint handle);
- which trait signatures are object-safe, especially `Self` outside the
  receiver, unbound trait parameters, value returns, generic requirements,
  effects, capabilities, and boundary machines;
- whether `dyn Trait` may be owned/stored directly or only borrowed, and how
  lifetime, mutability, drop, migration, and hot-swap pinning travel with it;
- who emits, owns, versions, validates, and updates machine tables, including
  the ABI identity used across separately built components; and
- how named satisfier selection and third-party named-only conformances are
  encoded and checked at coercion.

Recommendation: use a sealed descriptor whose logical identity is
`{instance, satisfier_contract}` and let a validated target-specific table be a
private realization of that contract. Initially admit only borrowed receivers,
fully bound trait parameters, and requirements whose nonreceiver
parameters/results do not mention `Self`; require declared effect/capability
ceilings at every dynamic descriptor entry. This keeps the public model independent of raw
table addresses and leaves room for loader-controlled table replacement.

## 2. What is automatic cleanup's graph-edge and partial-value contract?

Omega already records affine StateExit events and rejects non-empty `drop`
bodies so cleanup cannot silently disappear. Executing those bodies is not just
an instruction-selection task: the language has graph states rather than
lexical scopes, while the current guide still labels exact cleanup syntax and
field order provisional.

Decide:

- which outgoing edges run automatic cleanup (explicit transition, terminal
  return, natural state completion, trap/failure, and synthesized call
  continuation), and exactly where cleanup occurs relative to argument moves,
  guard evaluation, result materialization, and the target handoff;
- the deterministic order for locals, by-value parameters, the owning value's
  `drop` machine, remaining fields, nested aggregates, and conditional sum
  payloads, including partially moved values;
- whether the reserved `Type::drop(&mut self)` body is inlined onto every edge,
  lowered as an ordinary state call with a continuation, or represented by a
  distinct checked cleanup plan, and how recursion/re-entry is constrained;
- how `requires`, `ensures`, effects, boundary reaches, and the settled
  infallible/non-suspending rule are checked and instantiated at each implicit
  cleanup site; and
- what proof artifact distinguishes a trivial affine discard from executed
  cleanup and demonstrates that every live cleanup obligation is transferred or
  discharged exactly once.

Recommendation: synthesize an explicit checked cleanup-edge plan before
backend selection. On each normal outgoing edge, move target arguments first in
the semantic plan, then clean the remaining live locals in reverse creation
order, by-value parameters in reverse declaration order, invoke the owner's
cleanup body, and finally clean remaining fields in reverse declaration order.
Reject cleanup on nuclear traps, fallible/suspending drop bodies, recursive drop
cycles, and any partially moved shape the plan cannot enumerate. Treat this as
one ownership subsystem rather than special-casing calls in instruction
selection.

## 3. How do resource frontiers transform across values?

Omega requires structural linearity: a record, live sum payload, array, or
generic container cannot erase a contained linear obligation. The current
whole-place checker can conserve one obligation through a composite, but it
deliberately rejects extracting one field from a multi-resource linear record.
Accepting that program requires a semantic decomposition rule, not merely
recording a field segment: two independently established fields must retain two
origins, and the remainder must stay live after either field moves.

The same machinery must validate nominal resource transformations. An admitted
root may establish an abstract authority fact on a value. Checked operations
then consume, split, retain, borrow, transfer, or discharge that claim across
different result shapes. A qualified result or `ensures` clause states the
obligation but does not prove that the transformation conserved it.

Decide:

- whether `[linear]` on a composite denotes one nominal claim, the frontier of
  its contained linear claims, or a nominal claim in addition to those
  contained claims;
- whether constructing a composite automatically merges field claims, merely
  nests them, or requires an explicit resource operation, and the inverse rule
  for field extraction/destructuring;
- whether a by-value whole-composite consumer discharges every live component,
  only a nominal claim, or must expose an outcome mapping for each component;
- how an outcome mapping names consumed inputs, inherited output origins,
  discharged claims, retained borrows, and claims transferred across different
  nominal carriers;
- how alternative sum payloads, repeated array elements, generic substitution,
  and partially moved records identify their live component set at joins; and
- which stable identity extends `PermissionProvenance` so multiple components
  established at the same state-entry or statement source cannot collapse into
  one apparent origin.

Recommendation: define a value's permission state as a path-indexed resource
frontier. A nominal linear leaf contributes one claim; a composite with linear
children carries those child claims at canonical field/index paths without
minting an extra claim unless the declaration explicitly opts into a distinct
nominal protocol. Whole-value moves preserve the frontier, field moves transfer
the selected subtree and leave siblings live, and whole-value consumers must
account for every live frontier entry. Give each establishment an event-local
origin identity rather than using source location alone. Validate an
operation's normalized outcome mapping against that frontier; keep
subject-specific relations such as range partition, containment, or equality
in ordinary postconditions. Defer dynamic-index owned extraction until the
index/disjointness proof can name a unique element.

## 4. How is quantified convergence packaged as a quotient relation?

The checked construction corpus now proves rational closeness transitivity and
its pointwise sequence form for arbitrary precision and indices. That is not
yet the proposition required by `data Real = CauchySeq %
converges_together`. A Cauchy certificate has the logical shape "there exists a
modulus such that, for every positive precision and every pair of later
indices, the samples are close"; heterogeneous convergence has the same
existential/universal shape. Current machine parameters quantify only across a
theorem declaration. They cannot package an existential static-machine witness
and its universal proof as a value or as the checked pure binary `bool`
relation the quotient validator requires.

Decide:

- whether the general source surface is a proof-only proposition/certificate
  type, explicit quantifiers, or an existential package of static machine
  witnesses plus checked theorem schemas;
- whether a sequence's modulus and Cauchy proof participate in
  `CauchySeq<...>` family identity, remain erased evidence attached to one
  representative, or use a separate normalized proposition identity;
- how `converges_together<A, B>(a, b)` binds or receives its joint modulus and
  proof while remaining the binary relation shape required by quotient
  formation;
- how reflexivity, symmetry, and transitivity compose existential witnesses
  without a compiler-known Cauchy rule, and how their certificates are exposed
  to the existing quotient equivalence checker; and
- which termination, universe, coherence, and separate-compilation rules keep
  quantified certificates ordinary checked Omega declarations rather than a
  hidden trusted logic.

Recommendation: add one general proof-only quantified-certificate mechanism,
not Real-specific syntax. It should existentially package erased static-machine
witnesses with checked universal theorem schemas, give the resulting
proposition a normalized identity, and let quotient relations consume that
proposition plus ordinary equivalence witnesses. Keep all moduli and proof
machines out of runtime layout. Do not admit an always-true executable relation,
an implicit compiler quantifier, or a boundary axiom as a temporary Real
implementation: each would change or assume the semantics the construction is
supposed to prove.

## 5. What are the semantic world and resource policy for compiler-run Omega code?

Build-time evaluation executes ordinary machines in constant positions and
compiler-owned generator sites. Eligibility can require a checked
`EventualTerminal` summary, but termination proves only that work eventually
finishes; it does not make brute-force proof search, layout generation, or a
dependency-supplied computation affordable. Wall-clock limits would be
nondeterministic, while an unbounded evaluator permits accidental or hostile
compile-time cost.

The evaluator process runs on the build host but must interpret the selected
target's Omega world. Target integer widths, overflow and float behavior,
layout, endianness, calling-plan inputs, and other admitted target facts must
therefore be explicit semantic inputs. Accidentally consulting host width,
layout, environment, clock, filesystem, or floating-point behavior is a
correctness bug, not an implementation detail; a cross-build must compute the
same value as an equivalent evaluator hosted on the target.

Decide:

- which target facts compiler-run code may observe, how they enter the
  evaluation context, and which host observations remain categorically
  unavailable;
- how target-world inputs and evaluator-semantics version enter cache and
  diagnostic identity so a host or target change cannot reuse a stale result;
- which deterministic work unit the evaluator charges (machine transitions,
  reduced terms, proof-engine steps, or a normalized weighted combination);
- whether budgets are per invocation, package, compilation, or a hierarchy of
  all three, and how parallel evaluation preserves deterministic accounting;
- how a root project raises a budget deliberately without allowing a dependency
  to raise or consume an unreviewed amount silently;
- whether approaching a budget emits warnings, whether exhaustion is always a
  hard error, and how diagnostics render the expensive call chain and cache
  misses;
- how target-dependent operation costs interact with target-world semantic
  evaluation without turning the limit into target-specific wall time; and
- which results and certificates are Merkle-cached, and how cache identity
  includes target facts, evaluator semantics, and the granted budget where it
  can affect strategy.

Recommendation: require an available `EventualTerminal` guarantee for every
compiler-run invocation, including a local checked summary for an acyclic body;
this is admission, not budgeting. Charge a deterministic semantic-work counter,
apply conservative per-invocation and aggregate project ceilings, and let only
the root project grant explicit named increases. Exhaustion reports a
build-resource limit, never divergence or a failed termination proof. Cache
repeatable results by semantic inputs. Encourage expensive searches to emit
compact certificates consumed by cheaper checked verifiers, while still
allowing an owner-approved high budget for deliberate brute-force work.

## 6. Which keyword acknowledges a suspension-capable direct call?

Omega intentionally avoids `async machine` and `Future<T>`: one ordinary
machine can run in the current activation or be supplied to `runtime.start<M>`
for a distinct activation. That does not require possible parking to be hidden
at an ordinary-looking direct call. Suspension changes latency, cancellation
timing, continuation retention, and which loans and linear values cross a
scheduler boundary. Hiding those facts behind a local-call-shaped API conflicts
with Omega's bias toward explicit high-consequence behavior.

The direction is deliberate: Omega will require a source keyword at a direct
call whose selected contract may suspend. This makes latency-bearing calls
searchable, keeps suspension visible in code review, and prevents an ordinary-
looking API from hiding scheduler and continuation consequences. The previous
no-call-site-marker ruling was flawed because it optimized away exactly the
explicitness Omega normally requires. What remains deferred is the keyword and
its precise treatment of blocking, expression position, and generated code,
not whether suspension should be acknowledged.

Decide:

- whether possible blocking requires its own acknowledgement, shares one
  latency-bearing-call marker with suspension, or remains visible only through
  the contract and tooling; a shared marker is terser but hides whether the
  runtime may park the activation or occupy its worker;
- the spelling (`suspend`, `await`, or another term), especially since the call
  may complete immediately and returns an ordinary value rather than a future;
- how the checker derives the requirement from the normalized selected
  contract, so a concrete checked non-suspending refinement needs no marker
  while a call through a suspension-capable requirement does;
- how calls in expressions, transition subjects and arguments, generated code,
  proof/build-time evaluation, cleanup, and boundary adapters spell or forbid
  the acknowledgement;
- how artifacts record the marker's source acknowledgement while it affects
  only source legality and diagnostics, with no new machine identity, ABI,
  return type, activation, or lowering semantics; and
- how task start is distinguished: `runtime.start<M>` acknowledges creation of
  a distinct activation, while the call to `start` itself needs the suspension
  marker only when `start` may park the current activation.

Recommendation: treat the keyword as an audibility check over the normalized
suspension contract, not an execution operator: it does not force a park,
create a future, or change synchronous/direct invocation.
Calling `M(args)` still runs `M` in the current activation; `runtime.start<M>`
still creates another activation. A genuinely non-suspending API must expose a
narrower checked contract, commonly through a `try_` operation, rather than
promising that one invocation of a suspension-capable requirement happens not to park.
This follows the distributed-systems lesson that local and latency-bearing
operations should not be made syntactically indistinguishable; the closest
classic reference is Waldo et al., *A Note on Distributed Computing*.

## 9. How does a task-runtime provider publish checked behavior?

The normalized activation/runtime join and receipt qualification exist, but no
source or checked-plan carrier can currently supply the runtime side of that
join. `TaskRuntime` has ordinary runtime fields plus attached boundary machines;
canonical provider selection owns requirement realizations derived from
`satisfies` closures. No existing declaration or derived contract states a
runtime's continuation capacity/alignment, preemption granularity, CPU/thread
migration, continuation movement, cancellation support, or inline-completion
behavior. Those facts cannot be inferred from `suspends`, `blocks`, calling
conventions, target identity, or a provider plan's callable rows.

Decide:

- whether `TaskRuntime` becomes or is paired with an ordinary boundary-trait
  requirement realization, or whether provider plans gain a general nominal boundary-data
  requirement without introducing a task-only selection mechanism;
- which checked provider declarations/evidence derive each behavior field, and
  which fields may remain opaque claims requiring a root grant;
- how capacity and alignment claims bind to provider storage/arena plans rather
  than unaudited integer literals;
- whether `start` and `try_start` share one runtime behavior contract or may
  select distinct contracts, especially for inline completion and transactional
  rejection;
- how the behavior statement, provider-plan identity, selected realization, opaque
  runtime representation plan, and executable dispatch identity compose into
  one receipt without circularly treating a claim as its own proof; and
- how a selected runtime value carries that admitted provider provenance to
  `Task<T>` while preventing arbitrary opaque values from borrowing another
  provider's admission.

Recommendation: make task runtime an ordinary selected provider realization and add a
normalized provider-behavior evidence record to the common provider plan. Derive
capacity/alignment from an admitted storage plan and operational behavior from
checked provider contracts where possible; require the normal trust receipt for
opaque residual claims. One admitted runtime contract should cover both start
operations, with `try_start` additionally required to prove transactional
argument/lease return. Keep the current provider-independent activation demand
and join unchanged. Do not infer behavior from target names, manufacture a
compiler-only default provider, or add a parallel task-specific selection or
grant table.

## 10. What requirement family supplies primitive float operations?

`FloatFormat::BINARY32` and `FloatFormat::BINARY64` now state the permanent
semantic identities of `f32` and `f64`. The remaining F7 migration says target
packages provide checked conformances whose selected provider plans replace the
compiler's hardcoded IEEE instruction lowering. The corpus does not yet define
the requirements those conformances satisfy, however, or how primitive
spellings such as `+`, `-`, `*`, and `/` resolve to them.

This is public language architecture rather than an encoding task. One
requirement per concrete format and operation gives explicit identities but
duplicates the surface. One generic format-policy requirement is compact, but
must still distinguish each operation's contract, domain policy, result format,
and target availability without turning the format record into a dispatch tag.
Declaring the current built-in arithmetic path to be the requirement implicitly
would leave no browsable source contract for provider derivation.

Decide:

- whether primitive float arithmetic is governed by named boundary operators,
  a boundary trait family, or another existing requirement form;
- whether the format is selected by concrete carrier (`f32`, `f64`), a static
  format-policy parameter, or a requirement family derived from the carrier;
- which operations belong to the first contract family (arithmetic,
  comparisons, conversions, fused operations, and classification);
- how arithmetic-policy domains such as `Trapping` and `Saturating` refine the
  selected requirement without ambient modes or provider-dependent source
  meaning;
- which part of the result is the public semantic promise and provider-plan
  identity, versus accepted hardware evidence or proven software evidence; and
- how checked `asm` catalog entries satisfy the requirements while preserving
  the interpreter's exact-format semantics and allowing a software provider on
  targets without matching hardware.

Recommendation: use ordinary named boundary-operator requirements in
`omega::core`, selected by the complete static operand carrier/domain tuple.
Keep the semantic contract tied to the carrier's permanent `FloatFormat`
constant; derive provider plans from explicit target-package satisfiers.
Checked target assembly realizes those requirements, while checked software
machines may satisfy the same requirements. Begin with binary arithmetic and
comparisons; keep conversions, classification, and fused operations separate
named requirements rather than one open-ended `FloatOperations` grab bag.

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
  same sealed satisfier-reference family as dynamic dispatch while retaining a
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
  lifetime, thread, and reentrancy contracts; and
- which object-safety/descriptor machinery may be shared with question 1
  without making an external callback merely a `dyn Trait` descriptor entry or
  erasing the internal-versus-external calling distinction.

Recommendation: derive an opaque, non-forgeable entry reference only from an
admitted selected `satisfies` edge whose requirement pins the evaluated
`CallPlan + StatePlan`. Keep the artifact/entry value reusable and immutable;
have the registration boundary consume separate scoped authority and return a
linear registration receipt that owns the foreign-held root until explicit
revocation and quiescence. Materialize a native code pointer only inside that
admitted binding. Reuse sealed satisfier identity and root-ledger machinery,
but do not add raw machine addresses, integer conversion, an arbitrary
function-pointer type, or a Win32-specific callback construct.

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

## 16. What is the authored domain-policy surface?

Predicate and semantic facets are independent. Predicate membership may come
from checked proof, checked transformation, validation, or permitted boundary
evidence. Semantic meaning comes from an authorized binding-site commitment.
Weakening requires checked agreement, and open operator families need
deterministic ownership. The normalized compiler model carries the facet pair,
but the source declarations that author it remain incomplete.

Decide:

- how a domain declares `predicate`, `semantic`, or both facets without
  inventing a parallel attribute system or silently inferring public semantics
  from its body;
- the bodyless predicate spelling for abstract facts whose membership cannot be
  unfolded from carrier data;
- the `boundary domain` form that permits admission receipts to originate
  predicate membership, including its additive interaction with checked
  derivation and its normalized trust identity;
- the declaration spelling for sealed-by-default versus open semantic
  introduction, and how an exported `MintAuthority<D>` is accepted at an
  explicit qualification site;
- how an open operator family names its designated dispatch-owner position and
  how packages opt into that family without ambient candidate search;
- the source shape of a `weakens_to` certificate, including the denotation and
  operation-agreement obligations it must prove; and
- which of these declarations contribute to normalized semantic identity,
  sealed-theory identity, trust reports, and package coherence.

Recommendation: use ordinary domain declarations for every facet. The leading
bodyless predicate form is `pub domain T::Fact;`; adding `boundary` permits
receipt-backed roots without changing internal checked derivation. Require
semantic content to be authored explicitly, keep semantic introduction
owner-controlled by omission with one explicit open clause, and pass
`MintAuthority<D>` as an ordinary capability value at the semantic
qualification operation. Name one owner position in an open operator-family
declaration, and make weakening an ordinary checked certificate block whose
normalized promise, but not private proof steps, enters theory identity.

## 17. What is the Omega-authored `AccessPlan` policy surface?

The OS foundation deliberately separates layout geometry from access behavior.
`LayoutPlan` says where bits live. The normalized `AccessPlan` says how a
placed field may be observed or changed: exact transfer width, stable versus
externally-changing or atomic observation, read/write/atomic permissions,
public versus provider-private exposure, and statically pinned service reach.
The Rust validator, sealed field-authorization seam, and plan-pair checks are
live, but the Omega source record and policy-machine contract remain open.

Decide:

- which ordinary `omega::core` data records and closed case families represent
  access entries, observation, operation permission, transfer width, exposure,
  and pinned reach;
- which trait or requirement a package-authored policy machine satisfies and
  which schema/layout facts it receives when producing an `AccessPlan`;
- whether one policy produces a complete plan for a layout, or whether access
  families compose through an explicit normalized merge;
- how provider-private primitive access is made available only to the declaring
  device package while public derived accessors expose contracted operations
  such as W1C without a generic RMW escape;
- how a placed-view derivation cites both evaluated plan identities and checks
  them against the exact `Extent` loan/provenance before minting field tokens;
  and
- which plan changes alter public accessor identity versus only provider
  realization evidence.

Recommendation: copy the programmable-layout pattern. Define ordinary closed
`AccessPlan` data in `omega::core`; have an explicitly selected policy machine
compute one complete name-keyed plan from the reflected schema and validated
layout; normalize and validate the result in the compiler; and derive all
public field tokens/accessors from the accepted pair. Keep device-specific
operations as checked package machines over provider-private sealed access.

## 18. What fail-closed carry contract applies to boundary-origin authority?

Ordinary data is suspension-safe and affinity-free unless its fields or
explicit carry contract say otherwise. Runtime authority now uses ordinary data
plus domain evidence, so opacity no longer identifies the values that formerly
received a strict carry default. A forgotten provider/type annotation must not
make a boundary-origin token movable, migratable, or suspension-safe by
accident.

The carry restriction also cannot live only in a droppable predicate fact.
Forgetting `Granted` from an Extent may strand its legal resource consumer, but
it must not erase an independent CPU, thread, suspension, or address-stability
demand.

Decide:

- whether every receipt-originated authority value receives a born-strict
  per-value carry claim independent of its domain membership;
- how a boundary provider's admitted result contract grants narrower
  suspension/CPU/thread/address permissions without rewriting the type-wide
  structural floor;
- how checked internal issuers request the same fail-closed treatment when
  their abstract authority never crosses a boundary;
- how the carry claim follows moves, borrows, qualification forgetting,
  resource transformations, serialization, and component crossings; and
- which normalized promise enters type/contract identity versus which
  provider receipt remains realization evidence.

Recommendation: boundary evidence creates a maximally strict per-value carry
claim in permission provenance. The selected provider may grant axis-specific
permissions through its admitted result contract; omission therefore rejects
crossings. Carry remains independent of the predicate domain, so forgetting
membership cannot weaken the claim. Ordinary checked issuers use the existing
explicit type-wide or per-mint carry contract.
