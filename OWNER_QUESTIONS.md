# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-07-30.

## 1. What does contained execution failure do to outstanding obligations?

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

## 2. How are modular concurrency environment premises authored and discharged?

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

## 3. What is the public float-conversion requirement family?

The float record settles conversion semantics but not the public names or
signatures for policy-bearing conversions. `FloatSemantics` already defines
format conversion, integer-to-float rounding, and
exact/trapping/saturating float-to-integer results. Exact
denotation-preserving coercion belongs to `as`; directed rounding and every
lossy, trapping, saturating, or checked-result choice require separately
visible operations. Publishing guessed names now would freeze a core API that
the owning brief never chose.

Decide:

- which non-exact cases use destination-qualified operations such as
  `F32::from_f64` and `I32::from_f64`, and whether one generic conversion
  requirement instead carries source and destination types;
- whether exact, trapping, and saturating float-to-integer behavior is selected
  solely from the destination qualification or appears in distinct requirement
  identities;
- the separately named toward-zero/toward-positive/toward-negative format and
  integer-to-float variants, without introducing a runtime rounding-mode
  parameter;
- how source-visible primitive carrier requirements cite the proof-only
  `FloatSemantics` conversion functions and integer meaning;
- whether same-format policy conversion is a real operation or absent from the
  requirement family; and
- which diagnostics distinguish rejected exact `as` from the available
  policy-bearing operations.

Recommendation: use destination-qualified, statically typed requirement
identities; let the destination arithmetic-policy qualification select
exact/trapping/saturating result adapters; keep directed rounding as separate
operation names; and omit same-format conversion. This follows the settled
operand-driven provider model without carrying type or policy tags at runtime.

## 4. What is the source-visible placed-storage admission surface?

The normalized semantics are settled: a qualified `Extent` yields a bounded
shared or exclusive loan; a selected provider binds one offset-keyed
`ResourceProfile` and receipt to the range; `admit<P, T>` consumes that exact
loan and either returns it on rejection or produces a single-use
`PlacementAdmission<P, T>`; and `place` consumes the accepted token into
`Placed<P, T>`. The Rust foundation enforces this model. The source language
does not yet define how any of the opaque evidence-bearing values are obtained.

The missing surface is security-relevant. An ordinary public constructor would
let source spell a lookalike profile or admission. Encoding shared/exclusive
polarity as a runtime case would contradict the rule that a mutable reborrow
cannot upgrade the source loan. Hiding the whole operation inside a
package-specific provider would make normalized compiler admission and its
diagnostics unavailable to generic code.

Decide:

- the exact operations that borrow a qualified `Extent` into a bounded shared
  or exclusive `ExtentLoan`, including offset/length arguments, lifetime
  linkage, failure shape, and whether the two polarities share one nominal
  source type;
- how an admitted provider publishes a `ResourceProfile` for one exact range
  and binds address space, rights, provenance, mapping era, reach, and receipt
  without allowing an ordinary record literal to become supply evidence;
- whether `admit<P, T>` is a compiler-derived generic operation, a boundary
  requirement implemented by the selected provider, or a composition of a
  provider admission and a pure compiler check;
- the source representation and multiplicity of `ExtentLoan`,
  `PlacementAdmission<P, T>`, rejection diagnostics, and `Placed<P, T>`,
  including which lifetime parameters are explicit or compiler-erased;
- how a package-specific convenience operation can combine admission and
  placement without hiding the exact provider receipt or weakening rejection's
  return of the moved loan; and
- which normalized admission facts survive calls, storage, component
  crossings, and artifacts so target lowering consumes evidence rather than a
  numeric base and author-supplied offset.

Recommendation: keep the carriers opaque and compiler-known. Use distinct
shared and exclusive borrow operations whose result lifetime is derived from
the qualified `Extent`; let the selected provider establish a sealed
range-specific profile receipt; keep the demand/profile compatibility check in
one compiler-derived `admit<P, T>` that returns the exact loan on failure; and
make `place` the sole consuming constructor for `Placed<P, T>`. Package
wrappers may compose those operations but cannot mint or erase their evidence.

## 5. What is the generic atomic accessor requirement family?

Placed atomic fields already derive unique opaque accessors and direct atomic
syntax is gated per load, store, swap, compare-exchange, and fetch operation.
Chapter 20 requires helpers to accept one granular accessor without receiving
the entire placed view, but it does not define public requirement identities or
signatures for those atomic families. Guessing names would freeze a core API in
the same way as the unresolved float-conversion family.

Decide:

- whether each exact operation is its own requirement (`AtomicLoad<T>`,
  `AtomicFetchAdd<T>`, and so on) or operations are grouped into a smaller
  family with associated capabilities;
- whether ordering is an ordinary parameter to every requirement and which
  settled source ordering type it uses while the current implementation still
  carries transitional names;
- the receiver polarity and return signatures for load, store, fetch, swap,
  and compare-exchange, including the expected-value/update result shape;
- whether integer-only fetch operations are unavailable by missing conformance
  or expressed through an additional arithmetic carrier bound;
- how a generic helper's requirement set preserves the exact normalized
  operation subset when specialized to a placed accessor; and
- whether ordinary core atomic types conform to the same requirements or the
  family is specific to placed accessors.

Recommendation: publish one requirement per primitive operation, with ordering
as an explicit parameter and compare-exchange keeping separate success and
failure orderings. Derive only the conformances admitted by the normalized
placement, and let ordinary atomic types conform to the same operation
requirements so generic protocol code does not need a placed-only abstraction.

## 6. What is the v1 canonical portable IR contract?

The architecture requires one versioned, distributable, interpreter-defined IR
whose semantics are independent from mutable optimizer representations and
whose identity is independent from its fuel schedule. No current document
chooses the v1 representation. The reference interpreter executes TypedTrees
today, while later compiler stages already have state graph, control-flow,
abstract-operation, and target-operation forms. Declaring any one of those
canonical would freeze an artifact and proof boundary that it was not designed
to carry.

This is not merely a serializer choice. The canonical form determines what a
consumer verifies, what portable execution means, where ownership/effect facts
become executable obligations, which operations receive stable fuel charges,
and which future compiler changes preserve semantic identity.

Decide:

- the abstraction level and complete v1 type, value, operation, call, block,
  transition, and terminal vocabulary;
- where checked ownership, multiplicity, reach, trust, suspension/blocking,
  failure, and termination obligations appear in the executable artifact
  versus separately verified evidence;
- how target-semantic primitives, selected conformances/providers, layouts,
  boundary calls, and opaque admitted operations are represented without
  embedding a particular native ABI or optimizer choice;
- the canonical ordering, numbering, normalization, serialization, and
  fingerprint rules, including which debug/source/proof material is excluded
  from semantic identity;
- the verifier boundary and the lowering that proves a checked Omega program
  produced this IR, rather than accepting a hand-authored lookalike as checked;
- how the reference interpreter, restricted fixed-work checker, native block
  meter, and proof-carrying-code verifier consume the same instruction/block
  identities; and
- semantic-version compatibility: which changes require a new version, whether
  artifacts may carry several versions, and how old versions remain
  executable or explicitly retire.

Recommendation: introduce a new immutable normalized execution IR after
checked language semantics and before target-specific lowering. Use explicit
typed values, basic blocks, calls, transitions, and closed semantic operations;
keep target provider identities as explicit admitted operands rather than
native encodings. Define a deterministic binary serialization and fingerprint
over semantic content only, with debug maps and private proof evidence in
separate sections. Make the reference interpreter execute this form, then
derive the separately versioned fuel schedule over its stable operation/block
identities. Do not canonize TypedTrees or a mutable backend representation by
accident.

## 7. What is the authored unit-theory surface?

The normalized quantity model is settled as structural dimension, nominal
kind, rational scale, and non-semantic presentation. Exact `as` must derive the
same scale relation used by mixed-unit operators, prove divisibility and
representability, and invoke no authored conversion machine. The source
language does not yet define how a package authors any of those unit-theory
records.

Today a declared domain receives one opaque semantic identity, and the compiler
temporarily treats the presence of a domain-owned operator as its
denotation/dimension contribution. There is no source form for a dimension
vector, nominal kind, rational scale, presentation alias, or canonical unit.
Consequently `Km` and `Metres` can be distinct semantic domains, but the
compiler has no authored fact from which it can derive their exact ratio or
prove that they denote the same quantity kind. Inferring that relationship
from operator bodies or names would make normalization depend on executable
code or spelling.

Decide:

- the declarations that introduce base dimensions and structural dimension
  products, including integer exponents and canonical ordering;
- how a nominal kind cites one dimension while keeping equal-dimension kinds
  such as Energy and Torque distinct;
- how a domain contributes a kind and an exact rational scale, including the
  numerator/denominator orientation and the canonical reference unit;
- whether derived unit products and quotients need authored names or may remain
  normalized anonymous theories until presentation requests an alias;
- the source form and identity effect of non-semantic presentation aliases;
- how mixed-scale operators and exact `as` share one derived conversion plan,
  including integer divisibility and representability obligations; and
- which normalized dimension, kind, scale, and presentation records survive
  generics, separate compilation, interface fingerprints, and checked
  artifacts.

Recommendation: introduce explicit nominal base-dimension and kind
declarations, then let a domain attach one kind plus a reduced rational scale
relative to that kind's canonical unit. Keep presentation aliases in a separate
non-semantic field. Normalize dimension vectors and rational products at
declaration lowering, and make both operator resolution and exact `as` consume
that record; neither may inspect an operator body or invoke a conversion
machine to discover unit meaning.
