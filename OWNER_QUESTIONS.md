# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Question numbers remain
stable when a resolved entry is removed, so gaps are intentional.

Last pruned: 2026-07-26.

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

The structural frontier is settled. A claim is an identity-bearing entity; its
canonical path is its current location, and its origin/root lineage is separate
metadata. Every establishment creates a fresh identity. Moving a path moves
its claim subtree and leaves siblings live. Construction nests contributed
claims at field paths; destructuring performs the inverse. Variant claims are
guarded by the active case. Statically named array indices participate like
field paths; dynamic-index owned extraction remains a monotone acceptance
restriction until the checker can prove which element moved.

An explicitly `[linear]` declaration contributes one nominal root claim. A
transparent composite that merely contains affine/linear fields derives their
child claims without adding another nominal root. Whole-value consumers account
for every live frontier entry. One-to-one moves, borrows, containment, and
unambiguous aggregate mappings infer automatically; ambiguous mappings reject.
Carry inheritance follows each output's mapped origins rather than every
consumed argument.

The unresolved question is conserved decomposition and recomposition of
externally rooted claims. A qualified result and geometric postcondition do not
alone prove that one authority was neither duplicated nor enlarged. Per-output
relations are insufficient because conservation is irreducibly n-ary.

Decide:

- how a resource owner maps a claim to a compiler-understood footprint without
  teaching the compiler customer names such as `Extent`, `base`, or `split`;
- whether the first closed footprint vocabulary is
  `Indivisible | Interval<Scalar>`, with later counted/set algebras added only
  for real customers;
- how the checked rule proves parent footprint equals the separated
  composition of all children for split and the inverse for merge;
- how attenuation composes with footprint conservation without restoring
  discarded rights;
- which symbolic root-lineage identity survives ordinary calls and storage
  when fragments must later recombine; and
- how generic or runtime-sized collections expose separated claims once
  quantified array reasoning exists.

Recommendation: keep indivisible as the default. Let an owner select a
compiler-owned footprint algebra and provide a checked projection from its
carrier into that algebra. The ordinary prover establishes subject arithmetic;
the resource checker owns separated composition and frontier conservation.
Owner-originated resources may establish fresh claims under owner-authorized
machines; admitted/conduit resources may only root through receipts or conserve
existing origins. Layout plans and ordinary allocator bookkeeping remain
borrowed geometry and do not require owned split. Defer this algebra until a
subrange genuinely crosses an ownership boundary.

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
  lifetime, thread, and reentrancy contracts; and
- which local descriptor machinery from language-guide chapter 14 may be shared
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
