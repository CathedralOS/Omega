# Design Brief: Authority Values And Boundary Evidence

Status: semantic direction updated 2026-07-30. The core `Extent` declaration,
owner-authored root requirement, state-local constrained-parameter evidence
boundary, Cathedral's first admitted `Granted` root, and ordinary interrupt
obligation carriers are live; further provider, carry, resource-frontier, and
artifact work remains staged in `TASKS.md`.

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
range must eventually be discharged. Its outstanding provenance cannot be
cast away; it must be consumed or transferred. Operations that release, split,
map, or otherwise consume authority require the relevant domain membership.

## Qualification evidence

A domain declares predicate obligations in `requires` and exact authorized
establishment requirements in its body. `Extent::Granted` names its boundary
root directly:

```omega
pub domain Extent::Granted {
    ExtentRootProvider::grant;
}
```

The body does not call `grant`; it authorizes that exact requirement to
originate the domain at its qualified return position. A selected provider
satisfies the requirement, and admission records its receipt. A third party
cannot create a look-alike trait or machine to establish `Granted`.

Predicate-only membership is established by proof, including proof guaranteed
by a checked validator or accepted under an admitted receipt. Routed
membership is established by an authorized checked conformance, propagation,
checked transformation, or admitted boundary conformance. When one domain has
both forms, its predicates are proved at the authorized route's return.

Core's first live authority root uses this exact shape:

```omega
pub boundary trait ExtentRootProvider {
    machine grant(root: Extent) -> Extent::Granted
    ensures
        result in Extent::Granted;
}
```

A checked adapter may realize the requirement, but its ordinary direct-call
surface does not mint `Granted`. Semantic checking consumes the boundary
requirement and admitted receipt first; only afterward does execution dispatch
rewrite the selected trait slot to that adapter.

The compiler therefore does not treat an unlisted `boundary machine` guarantee
as domain evidence merely because the machine is accepted. An admitted
membership guarantee must be inherited from a boundary requirement, must spell
the bare `result` as its subject, and must return the carrier targeted by the
domain. Checked proof facts retain both the authorizing boundary trait and the
exact requirement signature; the qualification-evidence artifact publishes
that signature with the selected provider-plan receipt.

This keeps crossing semantics on machines and requirements. Internal checked
operations may still transfer the same fact, subject to resource-frontier
validation.

## Evidence and guarantees

A qualified result type or `ensures` clause is an obligation on a checked
implementation. It becomes available to callers only when the implementation
does one of the following:

- proves the domain's predicate requirements from visible propositions;
- receives the fact from a guard, parameter, or checked callee guarantee;
- validates runtime input through an ordinary checked machine;
- transfers an existing resource claim through a checked transformation; or
- crosses an admitted boundary satisfying a requirement named by the domain
  whose receipt supplies the fact.

These evidence sources feed one membership judgment while retaining their own
validation rules. Arithmetic proof, resource transfer, and accepted provider
evidence are not interchangeable.

Domain predicates use ordinary declaration visibility. Public predicates let
consumers discharge their propositions directly. Predicates not visible
outside the package are established externally through checked validators and
guarantees. A routed qualification depends on its exact authorized
conformance, existing evidence, checked resource transformations, or admitted
boundary receipts.

## Root authority

Root authority bottoms out at an admitted crossing. A platform memory provider
may return `Extent::Granted` together with address-space and rights
facts through the memory requirement it satisfies. Omega cannot prove that
firmware, a hypervisor, or a host OS transferred those ranges; provider
admission supplies that evidence and records its scope.

The root value carries runtime geometry when the platform discovers that
geometry at runtime. Domain membership itself adds no runtime tag.

A content-bearing root receipt is denominated in the same normalized
compiler-owned algebra as the claim it establishes. Admission proves the
claim's projected content is contained in the receipt's backing. A receipt
cannot introduce an owner-defined geometry that the resource checker cannot
normalize or compose. The provider may still lie about external reality; that
is the explicit admitted trust seam. Checked code cannot enlarge the receipt.

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

Every establishment creates a fresh claim identity. Its current canonical path,
root lineage, projected content, permissions, and carry policy are independent
metadata. Moving a path moves its claim subtree; aggregate construction nests
claims under field/case/index paths; destructuring performs the inverse.
One-to-one and otherwise unambiguous outcome mappings infer. Ambiguous mappings
reject rather than guessing.

### Content-bearing claims

A content-bearing qualified claim kind publishes one normalized projection from
its subject into a compiler-owned partial composition algebra. The projection
states what authority the claim covers; it does not add runtime metadata.
Unqualified carrier data has no claim and therefore no projected authority.

The closed algebra vocabulary begins with:

- `Indivisible`: the claim has one atomic unit and supports no owned
  decomposition; and
- `Interval<Scalar>`, for one-dimensional ordered ranges.

`CountedQuantity<Scalar>` is the first customer-driven extension. A bounded
bump/arena region projects residual capacity; allocation consumes normalized
payload size, alignment padding, and allocator metadata, while split and
return conserve the scalar quantity. Count alone does not prove placement in a
fragmented heap, which remains fallible or requires an exact free-extent or
reservation theorem.

Packages may author projections into that vocabulary. The compiler owns
normalization, containment, equality, and partial separated composition. New
algebra kinds require a compiler release and a concrete customer; arbitrary
owner-defined composition is not authority evidence.

Owner question #2 governs how a source declaration marks a claim as
content-bearing and selects this algebra, including whether an explicit content
clause may omit `Indivisible`. Ordinary linear claims never default into the
content algebra merely because they are linear.

An interval normalizes a coordinate-space identity plus half-open ordered bounds
`[start, end)`. Separated composition requires the same coordinate space and
nonoverlap. Its exact equality with a parent rejects omitted gaps; equal numbers
in different spaces never compose. Root lineage remains an additional
authority-family check rather than part of numeric geometry.

One projection is load-bearing in four places:

1. establishment proves `content(value)` is within checked or admitted backing;
2. every authority-bearing access proves its touched footprint is within
   `content(value)`;
3. transformations conserve the separated composition of all consumed and
   produced content; and
4. retirement accounts for every remainder through an authorized route.

Only an authorized establishment route introduces new content, and only an
authorized retirement route removes it. An owner-originated resource may expose
machines that establish fresh claims under the owner's policy. An externally
rooted conduit may establish roots only from algebra-denominated admitted
backing; its ordinary checked machines must conserve existing content.

An underapproximating projection is safe but restricts access. An
overapproximating projection rejects at checked establishment when the supplied
backing does not cover it. An admitted provider can still misstate external
backing, and the receipt records that accepted claim.

### N-ary conservation

Conservation is irreducibly n-ary. Per-output containment and scalar measures
are insufficient: two children may each lie inside a parent and have lengths
that sum to the parent while overlapping completely. A qualified result or an
owner-written `partitions` postcondition therefore does not license authority
duplication.

For every content-bearing claim kind, checked transformation proves:

```text
separate(content(consumed claims))
    =
separate(content(produced claims), content(authorized retirement))
```

Separated composition is partial: overlapping or otherwise incompatible
content has no composition. Exact equality rejects gaps unless an authorized
retirement accounts for them. Split and merge are the same theorem in opposite
dataflow directions. Binary operations are a common library shape, not the
semantic limit; the frontier theorem is general n-to-m conservation.

Root lineage remains distinct from geometry. Equal or adjacent content from
unrelated admitted roots cannot merge merely because the algebraic ranges fit.
Fragments created by different transformations may merge when they retain
compatible common lineage and their content composes exactly; literal
siblinghood is not required.

### Independent and related content

When one value carries several independent content-bearing claim kinds, every
algebra is conserved independently. When correspondence between quantities
carries authority meaning, independent projection is insufficient and the
correspondence must be represented by one joint content algebra.

For example, independently conserving a virtual interval and a set of physical
pages would permit children to exchange which page backs which interval. A
future virtual-to-physical mapping algebra must instead conserve symbolic
`virtual range -> physical backing` associations. It must use a compact
canonical closed form, such as normalized mapping runs, rather than enumerating
every page, and must keep containment, restriction, equality, and separated
composition decidable. Until such an algebra is specified, owned decomposition
of correspondence-bearing virtual/physical claims rejects.

### Orthogonal permissions

Content answers *which resource*. Permissions answer *which operations*.
Weakening read-write to read-only preserves content and irreversibly discards
the write permission; merge must not join permissions and silently recreate it.
If authority is scarce, exclusive, recoverable, or must return later, it is a
separate claim or loan rather than a freely discardable permission.

Domain facets retain their established predicate/semantic meaning. Multiplicity
governs copy/discard obligations, content governs decomposition, permissions
govern allowed operations, carry governs mobility, and root lineage governs
which authority family descendants may recombine.

### Borrowed geometry is not owned decomposition

Layout fields, placed views, subrange loans, borrow-backed Arenas, and ordinary
allocator free-list entries remain views or private geometry under one owned
root. They do not transform the root into independently owned claims and
therefore need no content split. Owned decomposition is required only when a
subresource genuinely leaves the parent's ownership domain.

Runtime-indexed owned extraction remains a monotone acceptance restriction
until the frontier and prover can name the unique moved element. Static field,
case, and array-index paths participate in the ordinary path-indexed frontier.

## Extent profile

`Extent` is one linear range carrier. Address space, permissions, mapping state,
and authority provenance are domain facts over that carrier.

Root providers may establish combinations such as:

```text
Extent in Granted & Physical & Readable
Extent in Granted & Virtual
Extent in Granted & Io
```

Checked operations preserve or attenuate those facts according to their
contracts. A content-bearing `Granted` claim projects to its address-space
interval. Split consumes its parent and proves the parent content equals the
separated composition of its descendants. Merge proves the same equation in
reverse over compatible common lineage. Loans carry the parent borrow and its
polarity. Mapping consumes destination authority and either consumes or borrows
its source; correspondence-bearing owned decomposition remains unavailable
until a symbolic joint mapping algebra exists.

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
| boundary machine | control, calling, reach, and guarantees |
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
values. Content-bearing reports additionally retain the normalized projection,
receipt backing, root lineage, outcome mapping, and n-ary conservation witness.

For hardware-entered provider slots, the selected service schema records a
linear routed parameter qualification as a structured `accepts` row. The row
uses the carrier-aware semantic-domain identity, begins with the strict
compiler carry policy, participates in provider-plan identity, and survives the
external-root selection bridge. This is the static admission contract bound by
the selected-plan receipt. A concrete source membership fact still requires the
matching installed-root invocation receipt; reconstructing the parameter's
ordinary fields or merely naming the selected plan establishes nothing.

## Carry of resource claims

Accepted resource claims originate with a strict four-axis carry policy. Their
result contracts may establish the positive compiler-owned permissions
`Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, and
`Carry::MovableAddress`. `Carry::Portable` is the transparent conjunction of
all four.

The carry entry belongs to the undischarged permission provenance rather than
the current predicate-fact set. Authority casts cannot discard that entry;
only consumption or transfer changes it. Freshly constructed unqualified data
has no claim entry and follows its structural/type-wide carry policy.

Checked-internal claims derive from the provenance and storage they inherit.
Claim transfer and conserved decomposition preserve permissions; combined
origins select the most restrictive demand per axis. The checker infers the
unique mapping for ordinary root, transfer, split, loan, and aggregate shapes
and rejects a transformation whose provenance assignment is ambiguous.

## Implementation dependencies

The implementation requires:

1. Migrate the domain surface to predicate `requires` plus exact requirement
   routes, remove ambient package-owner establishment, and retain route-backed
   claims in selected boundary-entry `accepts` rows and qualification
   artifacts. Concrete invocation receipts must remain connected throughout
   source qualification and the remaining authority-flow consumers.
2. The permission checker must preserve path-indexed claim frontiers and
   validate inferred resource-transformation outcome mappings together with
   their inherited carry permissions.
3. Qualified claim metadata must select and normalize
   `Indivisible | Interval<Scalar> | CountedQuantity<Scalar>`, and admitted
   receipts must carry backing in the same algebra.
4. The prover and resource checker must connect subject arithmetic and access
   footprints to compiler-owned containment and separated composition without
   teaching either system names such as `Extent`, `base`, `split`, or `merge`.

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
- split conserves one parent claim into exact separated child content without
  overlap, gaps, duplicated origin, or unrelated-root merge;
- admitted backing and projected content use the same normalized algebra;
- every authority-bearing access stays within projected content;
- every independent content-bearing qualification is conserved, while related
  quantities require one joint symbolic projection;
- permission attenuation cannot be reversed by merge, and recoverable
  authority uses a claim or loan;
- accepted resource origins default to strict carry, explicit positive
  permissions relax individual axes, and inherited claims preserve them;
- a fabricated or dequalified linear Extent has no legal consuming path;
- qualification facts and runtime authority carriers add no implicit runtime tag;
  and
- proof-only boundary data remains representation-free unless its boundary
  contract supplies a representation.
