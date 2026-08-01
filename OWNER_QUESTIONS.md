# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-07-31.

## 1. How are modular concurrency environment premises authored and discharged?

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

## 2. What is the public float-conversion requirement family?

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

## 3. What is the source-visible placed-storage admission surface?

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

## 4. What is the generic atomic accessor requirement family?

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

## 5. What is the v1 canonical portable IR contract?

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

## 6. How does a boundary requirement author algebra-denominated backing?

The semantic rule is settled: an admitted content-bearing root must receive a
per-invocation backing receipt in the same compiler-owned algebra as its
owner-selected `Content<A>` projection, and establishment proves projected
content is contained in that backing. Current provider plans retain the
requirement schema, selected realization, and receipt identity, but no source
or typed-tree value denotes the receipt's backing. The design briefs use
`content(receipt)` schematically; `receipt` is not a bindable contract subject,
and provider-plan rows contain no dynamic algebra value.

Decide:

- the source form by which a boundary requirement declares backing and relates
  it to parameters/result through an ordinary postcondition;
- whether the contract receives a compiler-provided erased receipt binder, a
  sealed algebra-valued expression, or another non-forgeable subject;
- how runtime-dependent geometry is captured per invocation while the static
  provider-plan fingerprint commits to the declaration rather than one value;
- how checked adapters prove the same relation and admitted leaves accept it
  without letting an ordinary record literal become backing evidence;
- how the compiler selects and validates the exact `Interval` or
  `CountedQuantity` identity, rejects algebra mismatch, and retains normalized
  containment in checked/debug artifacts; and
- whether a provider whose returned projection exceeds its backing rejects the
  invocation, returns a source-visible failure value, or constitutes an
  admitted contract violation at the boundary.

Recommendation: introduce a compiler-issued, proof-only receipt binder on the
boundary requirement. Let the requirement give that binder one closed
compiler-owned algebra expression over its parameters/result and state the
ordinary containment postcondition against it. Checked adapters prove the
relation; admitted leaves accept it under the selected provider receipt. The
binder erases at runtime, cannot be constructed in ordinary source, and the
normalized algebra expression plus containment theorem survive beside the
receipt identity.

## 7. How are content-conservation theorems authored in contracts?

The n-ary law and its closed algebras are settled, and checked claim outcome
maps already identify which input claim feeds each result path. The design
briefs say that ordinary postconditions relate projections, but their
`content(result)` and `content(old(buffer))` examples are schematic. Core
declares neither operation, typed proof expressions have no distinguished
pre-state snapshot, and no source form identifies an authorized retirement as
the remainder of the same separated equation. Inferring equality from field
names, constructor shape, or the outcome map alone would silently authorize
content duplication.

Decide:

- the proof-only source expression that applies the owner-selected
  `Content<A>` projection to a qualified claim, including how an author selects
  one exact qualification when a carrier has multiple independent claims;
- the spelling and binding rules for pre-state content of consumed or mutated
  parameters, and whether snapshots apply to arbitrary values or only
  compiler-normalized proof projections;
- the source representation of partial separated composition, exact equality,
  and an authorized-retirement term in one n-to-m theorem;
- how result field/case/index paths and input paths in the checked outcome map
  bind to theorem subjects without relying on parameter order or presentation
  names;
- which unambiguous transformations the compiler may infer directly and which
  require an authored postcondition, especially direct constructors, one-to-one
  returns, splits, merges, and consuming failure outcomes; and
- how independently conserved algebras produce distinct witnesses while a
  joint correspondence algebra prevents an author from splitting related
  authority into unrelated equations.

Recommendation: add compiler-resolved proof intrinsics for exact-qualified
`content(value)` and its entry snapshot, plus one closed `separate(...)`
relation whose terms are output claims or route-authorized retirement. Require
an explicit qualification selector whenever projection choice is not unique.
Permit inference only when normalized input and output projections are
definitionally identical after the checked outcome-map substitution; require
an authored theorem for every other n-to-m transformation. Erase the intrinsics
after checking while retaining the normalized equation and its proof result in
checked/debug artifacts.
