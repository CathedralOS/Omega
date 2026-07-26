# Design Brief: Authority Values And Boundary Evidence

Status: design direction recorded 2026-07-25. The declaration surface and
resource-transformation checker are staged through the owner questions named
below.

## Purpose

Omega represents runtime authority with ordinary data plus compiler-tracked
qualification and provenance. The data carries the runtime information an
operation needs. Domain membership states what authority, validation,
interpretation, or historical fact has been established for that data.

For example, a concrete range authority has ordinary runtime geometry:

```omega
data Extent [linear] {
    base: addr;
    length: u64;
}
```

The fields describe a range. Membership in `Extent::Granted` states that the
range descends from a live admitted or checked authority claim. Reconstructing
the same fields creates another value but does not reproduce that membership.

Multiplicity belongs to the data type. `Extent` is linear because every owned
range must eventually be discharged. Its outstanding provenance survives
qualification forgetting. Operations that release, split, map, or otherwise
consume authority require the relevant domain membership, so an unqualified
value has no legal resource consumer.

## Qualification evidence

A domain may have a predicate body or be bodyless:

- a bodyful qualification is established by proof, including proof guaranteed
  by a checked validator or accepted under an admitted receipt;
- a bodyless qualification is established by an owner-authorized machine,
  propagation, checked transformation, or admitted receipt under an
  owner-authorized boundary requirement.

The source spelling for a bodyless fact is:

```omega
pub domain Reservation::Issued;
```

`Extent::Granted` uses the same declaration shape:

```omega
pub domain Extent::Granted;
```

External origination is authorized by the requirement being satisfied rather
than by a modifier on the domain. The domain owner publishes the boundary
requirement whose result names the exact qualified subject. A selected provider
satisfies that requirement, and admission records its receipt. A third party
cannot make its own declaration an implicit establishment route for someone
else's domain.

This keeps crossing semantics on machines and requirements. Internal checked
operations may still transfer the same fact, subject to resource-frontier
validation.

## Evidence and guarantees

A qualified result type or `ensures` clause is an obligation on a checked
implementation. It becomes available to callers only when the implementation
does one of the following:

- proves a nonempty domain body from visible propositions;
- receives the fact from a guard, parameter, or checked callee guarantee;
- validates runtime input through an ordinary checked machine;
- transfers an existing resource claim through a checked transformation; or
- crosses an admitted boundary satisfying an owner-authorized requirement
  whose receipt supplies the fact.

These evidence sources feed one membership judgment while retaining their own
validation rules. Arithmetic proof, resource transfer, and accepted provider
evidence are not interchangeable.

Domain bodies may use ordinary declaration visibility. A public body lets
consumers discharge its propositions directly. A body whose supporting
predicates are not visible outside the package is established externally
through the package's checked validators and guarantees. A bodyless qualification
has no structural derivation and therefore depends entirely on existing
evidence, checked resource transformations, or permitted boundary receipts.

## Root authority

Root authority bottoms out at an admitted crossing. A platform memory provider
may return `Extent in Extent::Granted` together with address-space and rights
facts through the memory requirement it satisfies. Omega cannot prove that
firmware, a hypervisor, or a host OS transferred those ranges; provider
admission supplies that evidence and records its scope.

The root value carries runtime geometry when the platform discovers that
geometry at runtime. Domain membership itself adds no runtime tag.

A provider-neutral grant may be useful when platform admission and a portable
library need a separate handoff. Such a handoff is a resource transformation,
not a prerequisite for the root model: the admitted provider may return the
qualified authority directly.

## Checked resource transformations

Internal operations conserve authority through a generic claim frontier.
A normalized transformation records:

- which input claims are consumed;
- which output claims inherit each origin;
- how claims decompose across result fields or cases;
- which claims are discharged; and
- which claims remain borrowed or retained.

The transformation machinery tracks claims and provenance generically.
Operation-specific postconditions define the subject relation. Extent split
proves an exact range partition; a loan proves containment and lifetime; a
mapping proves the relevant source/destination relation. Range geometry is an
Extent contract rather than a compiler-wide resource concept.

An implementation's qualified result does not authorize the transformation by
itself. Checked lowering must validate the outcome mapping and its
postconditions, while an accepted transformation must present a receipt.

## Extent profile

`Extent` is one linear range carrier. Address space, rights, mapping state, and
authority provenance are domain facts over that carrier.

Root providers may establish combinations such as:

```text
Extent in Granted & Physical & Readable
Extent in Granted & Virtual
Extent in Granted & Io
```

Checked operations preserve or attenuate those facts according to their
contracts. Split consumes its parent and produces disjoint descendants. Merge
rejoins compatible descendants of one conserved authority origin. Loans carry
the parent borrow and its polarity. Mapping consumes destination authority and
either consumes or borrows its source.

Every operation that finally discharges an owned range requires
`Extent::Granted`. Forgetting that fact leaves the linear value live but removes
its legal consumer, so the program cannot satisfy linear checking. Constructing
an unqualified Extent has the same outcome.

`ExtentSlot { Empty | Live(Extent) }` remains the debt-free boundary form for
optional storage. Zero-filled Extent storage is unestablished; a live qualified
extent originates through an admitted or checked establishment route.

## Boundary declarations

`boundary` marks the aspect of a declaration that participates in an admitted
crossing:

| Declaration | Crossing concern |
|---|---|
| boundary machine | control, calling, effects, and guarantees |
| boundary trait | service requirement and provider realization |
| boundary data | representation supplied at a crossing |

Direction comes from supply and use: a checked body, requirement, selected
provider, accepted declaration, parameter, or result. The keyword does not
encode inbound versus outbound traffic.

Evidence crosses in the contracts of boundary machines and requirements. A
receipt records the exact qualified subject and the owner-authorized
requirement that licensed the accepted assertion.

Proof-only abstract values such as `Real` continue to use boundary data when
their representation is supplied by the admitted proof boundary. Runtime
authority values use ordinary data declarations whose layouts are derived from
their fields.

## Identity and reporting

Public data shape contributes its ordinary package/type identity. Domain
identity records its body, semantic contributions, and establishment
relationships. Private proof steps and resource-checker witnesses remain
implementation evidence.

Artifacts record each fact origin as checked, transferred, validated, or
accepted. Accepted origins include the domain, subject type, boundary machine,
selected provider, and receipt. Authority-flow reports continue to record
which packages accept, derive, retain, return, release, or acquire qualified
values.

## Carry of resource claims

Accepted resource claims originate with a strict four-axis carry policy. Their
result contracts may establish the positive compiler-owned permissions
`Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, and
`Carry::MovableAddress`. `Carry::Portable` is the transparent conjunction of
all four.

The carry entry belongs to the undischarged permission provenance rather than
the current predicate-fact set. Forgetting an authority qualification therefore
retains the claim's demand. Freshly constructed unqualified data has no claim
entry and follows its structural/type-wide carry policy.

Checked-internal claims derive from the provenance and storage they inherit.
Claim transfer and conserved decomposition preserve permissions; combined
origins select the most restrictive demand per axis. The checker infers the
unique mapping for ordinary root, transfer, split, loan, and aggregate shapes
and rejects a transformation whose provenance assignment is ambiguous.

## Implementation dependencies

Two general facilities complete this model:

1. The domain surface must support bodyless declarations, transparent aliases,
   owner-authorized establishment routes, and receipt-backed guarantees on
   boundary requirements.
2. The permission checker must preserve path-indexed claim frontiers and
   validate inferred resource-transformation outcome mappings together with
   their inherited carry permissions.

The current runtime authority declarations remain staged on their existing
boundary-data forms until those facilities land. Migration changes the source
declarations and compiler metadata while preserving the authority contracts.

## Acceptance

- reconstructing an authority carrier does not reproduce its domain facts;
- an accepted provider cannot originate membership without satisfying an
  owner-authorized requirement that names the exact qualified subject;
- every accepted fact origin appears with its provider receipt;
- a checked qualified result is rejected unless proof, existing evidence, or a
  validated resource transformation establishes it;
- split conserves one parent claim into exact child claims without duplicating
  its origin;
- accepted resource origins default to strict carry, explicit positive
  permissions relax individual axes, and inherited claims preserve them;
- a fabricated or dequalified linear Extent has no legal consuming path;
- qualification facts and runtime authority carriers add no implicit runtime tag;
  and
- proof-only boundary data remains representation-free unless its boundary
  contract supplies a representation.
