# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-24.

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
ceilings at every dynamic slot. This keeps the public model independent of raw
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

## 3. What is a composite linear value's resource frontier?

Omega requires structural linearity: a record, live sum payload, array, or
generic container cannot erase a contained linear obligation. The current
whole-place checker can conserve one obligation through a composite, but it
deliberately rejects extracting one field from a multi-resource linear record.
Accepting that program requires a semantic decomposition rule, not merely
recording a field segment: two independently established fields must retain two
origins, and the remainder must stay live after either field moves.

Decide:

- whether `[linear]` on a composite denotes one nominal claim, the frontier of
  its contained linear claims, or a nominal claim in addition to those
  contained claims;
- whether constructing a composite automatically merges field claims, merely
  nests them, or requires an explicit resource operation, and the inverse rule
  for field extraction/destructuring;
- whether a by-value whole-composite consumer discharges every live component,
  only a nominal claim, or must expose an outcome mapping for each component;
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
origin identity rather than using source location alone. Defer dynamic-index
owned extraction until the index/disjointness proof can name a unique element.

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
promising that one invocation of a suspension-capable slot happens not to park.
This follows the distributed-systems lesson that local and latency-bearing
operations should not be made syntactically indistinguishable; the closest
classic reference is Waldo et al., *A Note on Distributed Computing*.

## 7. How are bootstrap provider helpers staged before their runtime seals exist?

The checked IDT writer and `lidt` encoders are implemented, but their only
compiler entry points currently consume `PopulatedIdtWriter` and
`PreparedIdtLoad`. Those values are established after an executable artifact is
admitted, placed, and bound to concrete root and destination identities. The
first Cathedral image must already contain the helper code that performs those
transitions. It therefore cannot derive that code solely from the later runtime
seal without a bootstrap cycle.

The emitted bytes depend only on a normalized static shape: invocation calling
plan, writer fragment geometry, byte order, private context ABI, and source-slot
count. The exact installed-code, destination, content, ledger, and control
identities are invocation facts. They must remain sealed and audited, but they
do not select the helper's instructions.

Decide:

- whether one admitted artifact contains reusable compiler-generated helper
  templates which later borrow invocation-specific sealed contexts, or whether
  every concrete invocation requires a separately installed specialized helper
  artifact;
- how a target package selects the compiler-provided implementation of an
  ordinary helper requirement without an IDT-specific source intrinsic,
  user-spellable deriver operation, or freely callable raw-pointer surface;
- which normalized facts form template identity and static footprint evidence,
  versus which facts enter only the runtime invocation/install receipts;
- how the initial boot artifact records an uninvoked template without falsely
  claiming that an IDT, roots, destination, or `IdtControl` already exists; and
- how the eventual invocation proves that its opaque context matches the
  template shape before control enters the generated code.

Recommendation: emit reusable compiler-provided helper implementations selected
through ordinary provider requirements. Static template identity covers the
validated plan and exact generated bytes; a later linear invocation carrier
binds installed artifact, destination, resolved private context, roots/ledger,
content, and control. The runtime receipt names both identities. Do not put
runtime instance IDs into the prebuilt code template, synthesize executable
bytes after handoff, or expose a source operation that accepts an address.

## 8. How does opaque runtime `boundary data` acquire a representation?

`boundary data` correctly has no public fields or constructor. It currently also
has no backend layout at all: layout planning skips every
`DataSupplyMode::BoundaryOpaque` declaration. That is sufficient for proof-only
symbols such as `Real`, but not for runtime provider-minted values such as
`Extent`, `Ptr<T>`, `TaskRuntime`, `InterruptMaskGuard`, or
`InterruptAcknowledgement`. Those values must cross calls, occupy storage, obey
linearity/carry rules, and preserve a provider-owned identity without exposing
forgeable representation.

The normalized Rust `omega-extents` carrier and the opaque Omega `Extent`
declaration therefore do not yet meet. Cathedral cannot replace its temporary
plain extent record honestly until the source/runtime boundary says what value
the provider returns and how that value is stored and passed.

Decide:

- how a declaration distinguishes a proof-only opaque symbol from a runtime
  opaque carrier without inferring semantics from special names;
- whether runtime opacity always lowers to a compiler-owned sealed handle, or
  whether a provider may select among a closed validated representation
  vocabulary such as handle, native pointer, word token, or fat descriptor;
- how the representation enters `LayoutPlan`, `Calling<C>`, artifact identity,
  provider admission, and separate-component ABI compatibility without making
  private payload fields public;
- where the provider-owned backing state lives, how a source value resolves to
  it, and how stale handles, target width, provenance, and generation are
  validated;
- how moves, borrows, linear consumption, carry policy, suspension, and
  destruction operate on an opaque carrier, especially when the provider state
  is larger than the source representation; and
- whether `Ptr<T>` is an admitted special native-pointer representation or an
  ordinary instance of the same representation-plan mechanism, while ensuring
  no raw numeric address becomes authority.

Recommendation: keep `boundary data` as the opacity/trust surface, but require a
runtime use to be backed by an admitted, normalized representation plan. The
closed plan vocabulary should include an erased proof-only case and a
compiler-managed sealed-handle case first; add native word/pointer/descriptor
cases only where their operation contracts and target ABI validate them. The
provider plan owns minting, backing storage, lookup, and consumption, while the
source carrier exposes no constructor or representation fields. Do not silently
assign every opaque type zero size, make every carrier pointer-sized, infer a
representation from its name, or let a package declare arbitrary layout bytes
as authority.

## 9. How does a task-runtime provider publish checked behavior and own its slot?

The normalized activation/runtime join and receipt qualification exist, but no
source or checked-plan carrier can currently supply the runtime side of that
join. `TaskRuntime` is opaque `boundary data` with attached boundary machines;
canonical provider selection, by contrast, owns boundary-trait slots derived
from `satisfies` closures. No existing declaration or derived contract states a
runtime's continuation capacity/alignment, preemption granularity, CPU/thread
migration, continuation movement, cancellation support, or inline-completion
behavior. Those facts cannot be inferred from `suspends`, `blocks`, calling
conventions, target identity, or a provider plan's callable rows.

Decide:

- whether `TaskRuntime` becomes or is paired with an ordinary boundary-trait
  slot, or whether provider plans gain a general nominal boundary-data
  requirement without introducing a task-only selection mechanism;
- which checked provider declarations/evidence derive each behavior field, and
  which fields may remain opaque claims requiring a root grant;
- how capacity and alignment claims bind to provider storage/arena plans rather
  than unaudited integer literals;
- whether `start` and `try_start` share one runtime behavior contract or may
  select distinct contracts, especially for inline completion and transactional
  rejection;
- how the behavior statement, provider-plan identity, selected slot, opaque
  runtime representation plan, and executable dispatch identity compose into
  one receipt without circularly treating a claim as its own proof; and
- how a selected runtime value carries that admitted provider provenance to
  `Task<T>` while preventing arbitrary opaque values from borrowing another
  provider's admission.

Recommendation: make task runtime an ordinary selected provider slot and add a
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
