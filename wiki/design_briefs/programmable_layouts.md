# Design Brief: Programmable Layouts

Current as of 2026-08-21. Layouts and codecs are library policies with
machine-checked contracts. The compiler owns a small placement vocabulary,
plan validator, and realization checker. Codec realizations may be authored or
generated from the same normalized plan.

## One data declaration

Omega's semantic declaration form is `data`. Stable `#N` identities and
`retired #N;` tombstones qualify ordinary fields and cases for policies that
need durable identity. Numbering is all-or-nothing within each record, sum, or
structured case-payload scope.

Layout/encoding choices attach at use sites or in type position. They do not
change the semantic data type and do not become compiler keywords for every
foreign format.

## Layout policies

A layout policy satisfies `Layout` and supplies a build-time `plan` machine:

```omega
trait Layout {
    machine plan(schema: Schema) -> Plan;
}

data CLayout;

machine CLayout::plan(schema: Schema) -> Plan
    satisfies Layout::plan
    terminates by schema.fields -> Slice::Length;
{
    ...
}
```

`offset` is byte-denominated and `stored_width` is bit-denominated. The first
normalized slice accepts positive whole-byte widths through 64 bits; wider or
sub-byte stored integers require an explicit later lowering capability.

`plan` must be build-time-admissible under the complete machine contract. It
returns a description composed from a closed compiler-known placement
vocabulary. The compiler validates the result before using it for type layout,
field projection, recasts, or ABI artifacts.

The normalized `Schema`/`Plan` ABI uses `u64` for opaque member keys, stable
member identities, and every nonnegative size, alignment, offset, bit
width/index, tag, and count. These are integer quantities, not addresses, so
they do not use `addr`. A schema member carries `Optional<u64>` identity rather
than reserving an integer sentinel. Build-time evaluation preserves all 64
value bits, and the normalized Rust plan uses the same unsigned geometry; host
`usize` conversion occurs only at a consuming allocation or slice boundary and
is checked there.

The live reflection now carries record fields, sum cases, structured-case
payload fields, and each scope's retired identities. Case array order remains
authored declaration order for policies that care about home layout, while
canonical schema reporting and plan normalization sort numbered scopes by
stable identity and therefore do not mistake `#N` for a runtime discriminant.
The resulting schema FNV is a compact report fingerprint, not schema authority;
typed reflection collision-checks it locally and later replay retains the exact
member/case rows and physical plan. Fixed-layout `At` and `Bits` placement
remains limited to the reflected common/record fields; tagged case placement
belongs to the next closed-vocabulary extension.

The compiler's existing conventional sum representation has a separate
read-only report for constant materialization. `omega-layout` projects its
fixed four-byte tag, authored-order ordinals, all-case payload overlay, and
total geometry; Psi revalidates that report before staging one active case.
This report is not a `LayoutPlan`, cannot be authored by a layout policy, and
does not settle the deferred tagged/untagged programmable placement vocabulary.
For direct nested constant materialization, `omega-layout` pairs that report
with a whole-field outer `LayoutPlanReport` projected from the same target
runtime layout for the complete nonempty authored-order set of direct pure-sum
fields in one closed `[copy]` record. A separate bounded rung admits the complete
nonempty authored-order set of direct nonzero literal fixed arrays of
conventional sums. Each compact row retains one outer field identity, count,
stride, and complete element layout while value custody remains distinct per
field occurrence and literal index. Singular surfaces remain exact-one
wrappers. Outer reports place both direct sums and each whole array only as
opaque aggregate `At` fields;
tag, case, and payload-overlay geometry remains exclusively in conventional
read-only reports. Psi rejoins and replays the exact reports. A separate first
one-level record-path report retains the complete nonempty authored-order set of
qualifying outer field occurrences, one shared outer whole-record plan, and one
inner whole-record plan plus complete direct conventional-sum rows per
occurrence, all projected from the same target plan. The outer carrier reuses
one validated inner direct-sum carrier per opaque field and reconstructs every
inner image plus the outer image before one atomic copy; it does not flatten
child placement into the outer schema or expose programmable tag placement.
Repeated uses of one inner type remain occurrence-distinct. Deeper paths,
zero-length or nested sum arrays, mixed common-field/case shapes, and target-
dependent sum geometry remain excluded. One further fixed-depth report retains
the outer whole-record plan once and composes each exact outer occurrence with
the unchanged plural one-level report, admitting the complete nonempty
authored-order set of `Outer -> Middle -> Leaf -> direct sums` chains. Its
consumer composes one existing plural one-level carrier per outer occurrence
and reconstructs every leaf and middle image plus the outer image before one
atomic copy. The earlier singular surfaces remain exact-one wrappers. Deeper
chains and every shallower, array-mediated, or enclosing direct-sum occurrence
remain excluded; no child placement is flattened or exposed as programmable
tag placement.

The closed vocabulary includes only primitive placement concepts the backend
must understand: offsets/alignment, fixed and runtime strides, tagged/untagged
overlays, bit ranges, fragmented placement of one logical source across several
destination ranges, variable-length wire placements, and explicit endianness.
Entries are keyed by schema-field identity rather than by positional array
index, because fragments and overlays may contribute more than one placement
for a field. A new format is normally a library policy; a new placement
primitive requires a compiler release.

A native-only plan may also declare a typed private-materialization demand.
Its slot source is neither a semantic schema field nor an author-writable hole:

```text
SlotSource =
    SourceField(FieldId)
  | Constant(ConstantId)
  | PrivateMaterialization(MaterializationSlotId, MaterializationKind)
  | Padding
```

The first customer is a foreign callback field whose kind names one exact
callback requirement. Core supplies the empty compiler-known relationship. A
target package declares the stable typed slot as an ordinary named conformance,
then explicitly cites that exact evidence while building the native plan:

```omega
trait PrivateCallbackSlot<machine Requirement> {
}

WndClassWindowProcedureSlot:
    WndClassLayout satisfies
        PrivateCallbackSlot<WindowProcedure::call>;

// Conceptual closed-vocabulary plan operation. The selected conformance, not
// the offset, is the slot identity.
Plan::place_private<WndClassWindowProcedureSlot>(
    plan,
    window_procedure_offset
)
```

The conformance declaration is inert until a plan explicitly cites it. There
is no enumeration of conformances on `WndClassLayout`, unique-visible choice,
or owner-only exception to ordinary named-conformance rules. A third-party
declaration cannot inject a demand into an existing plan; an explicit citation
instead records the dependency and selected evidence normally. The conformance
subject makes the layout owner part of the typed declaration, and its static
requirement argument must resolve to one exact signature-free callback
requirement. An ambiguous overload rejects.

Plan evaluation resolves the conformance into a compiler-issued slot identity
and exact callback-requirement identity. The authoritative layout policy may
author or compute the physical offset, including from target semantics, but
that offset never identifies the slot and is never repeated in a calling or
binding plan. The normalized demand retains the stable slot declaration,
requirement declaration, target-closed placement, and complete layout-plan
identity separately. A target-neutral callback requirement may therefore keep
one identity while x86 and x64 plans place its slot differently.

The source-visible specification has no corresponding field. A layout
containing a private demand is incomplete for ordinary value materialization;
only a consuming plan that supplies every demand exactly once may use it. The
slot is absent from source projection, read, write, serialization, and runtime
value topology, and the compiler never exposes the materialized entry identity
as source-visible `addr` data. `Placed<P, T>` may carry staging or retained
native storage after validation, but it does not remove this plan entry: `T`
still has no semantic member from which the private demand could be derived.

This two-step declaration/citation is specific to a destination independently
owned by a layout. A callback occupying a whole foreign ABI parameter is
declared once, interleaved in the registrar requirement's ordered native
telescope. That native-only parameter has no semantic value; the requirement
itself authorizes the demand, and the calling policy only places its nominal
identity. It does not reuse `PrivateCallbackSlot` or permit a layout policy to
inject parameters into another declaration.

## Plan validation

The validator proves deterministic structural rules such as:

- all referenced schema fields exist exactly where the policy permits;
- offsets, sizes, and strides are in range;
- alignments are valid;
- non-overlay fields do not overlap;
- bit ranges fit their storage slots;
- repeated fragments tile the declared source bits exactly, without source or
  destination gaps/overlap except where an explicit overlay permits it;
- overlay/tag rules are internally consistent;
- dynamic extents are bounded by the enclosing carrier; and
- every private-materialization demand is consumed exactly once by a compatible
  enclosing plan, with no duplicate, overlapping, or unresolved supply; and
- the plan normalizes to one stable identity.

Published layout/type identity is normalizer-owned. Prover strength may accept
or reject a policy conformance but never change the canonical plan or ABI key.

## One plan, several derived consumers

The same normalized geometry may feed different compiler-owned consumers:

- a codec plan for bytes in buffers the program owns;
- direct field projection for ordinary plan-laid values;
- shared byte-region record views containing a plan-laid subrecord (implemented
  for fixed scalar fields, recursively nested fixed arrays composed of
  supported primitive elements or fixed checked-shape records, and fixed record
  fields recursively composed from those shapes in
  both native and interpreter execution, including ordinary semantic widening
  after an equal-width stored scalar has been projected on x86-64 and AArch64;
  this is not width-varying foreign storage). Such an array reflects as one
  `Repeated` field with one whole-extent `At` placement; a fixed record likewise
  reflects as one `Nested` field whose enclosing placement is one whole `At`
  extent and whose interior offsets remain compiler-derived. Scalar `Bits`,
  `IntegerAt`, and active access decisions remain rejected for either aggregate;
- mutable byte-region record views for recursively fact-free fixed records
  (implemented with nested plan-laid field write-through, including stacked
  fixed indexing and mutation below recursively nested primitive-array or
  fixed-record-array fields through their whole `At` extent, plus two runtime
  indices through a plan-laid gapped outer fixed array of recursively fixed
  arrays while retaining distinct plan-derived outer and compiler-derived inner
  strides, and a gapped outer fixed array of fixed records that retains an
  intervening member offset before the inner fixed-array index, in both native
  and interpreter execution, plus x86-64/AArch64 compile rails);
- placed-view projection over an authorized external extent; or
- ordinary scalar materialization into fixed dictated structures. The
  normalized foundation also admits atomic aggregate materialization when the
  compiler supplies each field's exact physical extent, including multiple
  independently keyed whole aggregate fields in one plan. A field may use one
  whole-value `At`; an outer fixed array may instead use exactly one `At` per
  compiler-sized element at one nonoverlapping constant destination stride.
  Authored entry order is presentation: sorted destination offsets select
  semantic element order. Incomplete values, wrong element counts, irregular
  strides, scalar fragments, overlap, and out-of-bounds placement reject before
  destination mutation. The typed source-owned bridge derives complete field
  bytes and extents from a checked structured value plus its exact typed-tree
  schema for fixed records and arrays in the supported fixed subset. Psi
  retains the semantic value and
  shape; the Omega realization seam supplies target byte order. Erased bindings
  remain required semantic terms but contribute no bytes or initialization
  work; an owned record whose fields are all erased therefore validates its
  complete semantic value while materializing only zeroed plan storage. A
  relevant nested field with that erased-only shape, including a fixed array of
  such records, likewise remains mandatory semantically but receives no
  physical plan entry. This
  does not create a by-value public ABI carrier. Schema/type mismatch, duplicate
  or missing fields, out-of-range scalars,
  sum/unresolved-generic/reference shapes, and unsupported recursion reject
  before destination mutation. A fully specialized generic record is no longer
  generic at this seam: the synthesized concrete `CheckedShape` symbol and its
  substituted member type references provide the exact recursive schema.
  Angle-bracket spelling is descriptive only and cannot select layout identity.
  An
  admitted zero-argument source machine may supply that structured value through
  Psi's checked interpreter, so source construction reaches the same writer
  without exposing physical field bytes; or
- a materializer that resolves symbolic data/entry identities into an artifact
  or post-load structure.

Consumer applicability is derived and validated. A policy cannot claim that a
symbolic relocation is decodable, or that a variable-length wire placement is
a valid MMIO projection, merely by setting a flag.

Access behavior is deliberately not part of `LayoutPlan`. A separate normalized
`AccessPlan` describes consumer demand per field: inaccessible, stable,
external, or individually atomic operations plus exposure. A nominal
`PlacementPlan` combines the selected layout and access plans with the
boundary reach required by that interpretation. Provider supply remains
separate as an admitted, offset-keyed `ResourceProfile`. Placement checks the
pair against one exact borrow of `Extent in Granted` and derives
`Placed<P, T>`. `ResourceProfile` is ordinary data; only the selected
provider's range-bound receipt gives one standing as supply.

Dormant owned content uses the core qualification
`Extent in Granted & Resident<P, T>`. `Resident` covers the exact placement
range and carries the complete represented and non-runtime custody of one
`T`; it is mutually exclusive with `Vacant`, cannot be weakened away, and
rejects ordinary Extent split or merge. Borrowed placed views loan that exact
claim, while owned views temporarily carry it and resident-preserving
retirement returns the same occurrence. Address, mapping, revision, and
occurrence identity remain evidence rather than type arguments.

The access policy receives this validated `LayoutPlan`, so it can decide which
laid fields admit primitive access without copying offsets or transfer widths
into source. It addresses the reflected schema with compiler-issued field keys
and starts from an all-inaccessible plan. The evaluated plan has exactly one
decision per runtime-relevant reflected schema field; omission is denial, and
declaration reorder cannot silently reassign permissions. An `[erased]` binding
remains in semantic/type identity but has no physical field key or access
decision. Each field key retains a crate-sealed, domain-separated commitment to
the complete canonical layout that issued it. The adjacent compact layout
fingerprint is report compatibility only, so holding it equal cannot move a key
between exact layouts during mutation, lookup, authorization, or projection.

Placed projection is pure and yields borrow-carrying accessors rather than
lvalues. Stable access derives ordinary mutation only when both the active
borrow and source borrow are exclusive. External access is exactly once at an
admitted whole-container width and never synthesizes generic RMW. Atomic fields
expose only admitted operation families and orderings. Each operation carries
both its logical field extent and physical effect footprint; destructive reads
and stable RMW conflict over the whole affected transfer container even when
their logical bitfields are disjoint. Boundary reach belongs to the placement,
not individual fields, and runtime provenance proves that the selected reach
may touch the supplied range.

Admission and content establishment are distinct. Admission proves that the
backing supports the requested interpretation. Stable storage may view exact
resident content, initialize vacant storage, or validate existing contents. A
non-resident range cannot be viewed or validated as a `T` with represented
non-copy fields: bytes and proof do not establish custody. External content is
opened by a provider-specific wrapper that first establishes its existing
qualification and then uses the appropriate view operation; there is no
generic adopt or cast registry.
Encoding, decoding, representability, and legal transfer derivation are checked
per field and operation. The compiler never invents a fitting domain, emits a
generic External RMW, assembles an External field from several reads, or hides
an unbounded atomic retry behind `.write`.

Compatibility does not prove that a schema describes the physical device. A
separate admitted, provenance-bearing correspondence ties the nominal policy to
one provider/device identity and may be conditional on a runtime revision check
bound to the same stable device instance.

The ordinary source records now live in
`omega::language::core::layout`; its existing `Plan` record remains the current
source spelling of `LayoutPlan`. Source access policies construct their exact
schema cardinality from `AccessPlan::inaccessible(schema)` and keyed functional
replacement. The compiler evaluates `Access::plan` against a reified validated
layout, derives transfer widths from that geometry, and evaluates
`Placement::plan` into one normalized layout/access/reach identity. The
target-neutral service lives in `psi-build-time-evaluation`; Omega schedules it
and consumes its sealed Psi plan carriers for target realization but does not
own those language semantics. Plan-laid type desugaring and `Placed<P, T>`
probe/evaluation/exact-accessor synthesis live in that service as paired
pre-resolution and post-typing passes as well. The
`psi-access-plans` bootstrap validates geometry, exact widths,
observation/operation compatibility, borrow polarity, atomic orderings, exact
internal loan facts, and sealed lowering requests. Stable, External, and Atomic
consumers narrow those requests without reauthoring geometry and return the
original authority-bearing request unchanged when specialization rejects. Its
normalized `PlacementPlan` owns the complete layout/access pairing and one
normalized boundary reach, which admission checks once and lowering retains.
Before content establishment, a borrowed admission may be withdrawn to recover
the exact original loan without claiming vacancy or destruction. The Rust
bootstrap consumes an internal exact-loan carrier; source instead admits
ordinary `&`/`&mut`
projections of `Extent in Granted`, with range, lifetime, and polarity supplied
by the borrow checker. Current view-borrow and retained source-borrow polarity
are checked independently.
Exposure uses the settled `BindingPrivate` spelling; stable compound mutation
is derived from read+write and exclusivity, destructive external reads remain
distinct, external compound mutation is unavailable, and atomic permissions
distinguish exact operation families. Provider supply now enters through
receipt-bound, normalized offset-keyed resource profiles. Placement/profile
compatibility restricts profiles to exact subrange loans, checks requested
observation, operations, widths, reach, and rights, derives the static base
congruence, and discharges that congruence against the concrete loan base at
the placement compatibility judgment. The normalized foundation carrier also
separates pure field
projection from its event: `project`/`project_mut` return borrow-carrying
accessors whose named read, destructive-take, write, stable-compound, and
atomic-family methods are the only routes to a sealed primitive request. See
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md)
for the full `AccessPlan`, `ResourceProfile`, compatibility, and `Placed<P, T>`
model. Establishment uses distinct borrowed and owned entry points for the
view, initialize, and validate families. Non-runtime Type custody travels in
an ordinary authored data record, and an explicitly selected named
`PlacementCustody<P, T>` conformance checks that record against the exact
evaluated plan. Borrowed rejection releases the loan and returns custody;
owned rejection returns both the `Extent in Granted` and custody through the
authored `PlacementReturn` carrier. Providers establish external
qualifications before viewing, and proof results remain in the separate `;`
output lane. Source compilation now derives unique opaque
stable/external accessors
for concrete `Placed<P, T>` spellings and omits inaccessible or unauthorized
operations. Atomic fields now derive exact `bool`/`u32`/`u64` operation-family
accessors, and binding-private operations are restricted to the nominal policy
package for direct naming and issuance; possession delegates their public
operation requirements to generic code. Qualified-borrow placement
compatibility is settled and remains implementation work. Generic atomics use
the settled sealed
per-operation requirement family shared with ordinary core atomics;
target-specific lowering remains implementation work.

## Codecs are ordinary checked requirements

Encoding, decoding, and validation are ordinary library-machine requirements.
A realization may be authored or generated from a validated `Plan`.

```omega
trait Codec<Policy, Value> {
    machine encode(
        value: &Value,
        out: &write [u8],
        written: &mut count
    );

    machine decode(bytes: &[u8], out: &mut Value) -> DecodeResult;

    machine roundtrip(value: Value)
        ensures decode(encode(value)) == value;
}
```

The concrete conformance proves the trait's agreement requirements. Historical
schema migration belongs to the format lineage and composes outside the
current-shape codec law.

Realization origin and trust class are orthogonal. An authored or generated
body independently checked against the public requirement is derived. A
generator accepted as correct by construction is admitted with the compiler as
the named trusted party. Opaque foreign realizations are admitted with their
provider. Artifacts retain the normalized plan, requirement identity, origin,
trust class, and evidence.

Validation/establishment is exclusive: user code cannot construct a “valid” result
whose payload claims a domain it has not proven. A validator may establish the
domain because its checked contract proves the predicates on the returned view.

## Type-position layout

A fully static policy may lay out values directly:

```omega
data Descriptor in CLayout {
    kind: u32;
    address: u64;
}
```

The semantic fields remain Omega fields; the policy controls physical
placement. Packed layout is a policy/plan, not a `[packed]` escape hatch.
Bit-addressable fields use ordinary integer carriers plus range contracts and
bit placements; Omega does not need `u3`/`u17` primitive types.

For `Bits`, exact source tiling is measured against the declaration's
representation width, not blindly against the carrier's byte width. `bool`
therefore contributes one bit; a non-negative constant integer range
contributes the bits required by its maximum value; unconstrained and
negative-capable integers retain their full carrier width. Omitting a
representable bit remains an invalid plan. This is packing, not truncation:
the validator derives the width from checked type facts, while the policy only
chooses where those bits go.

Width-varying foreign integer fields use a distinct closed placement, not
`At` or partial `Bits` tiling. Conceptually:

```text
IntegerAt {
    offset,
    stored_width,
    interpretation: Signed | Unsigned,
}
```

The semantic field retains its portable integer carrier. A read loads exactly
the stored width and sign- or zero-extends according to the placement. This is
a total decode when the stored integer range fits the semantic carrier. A
mutable view is derived only when every admitted semantic value encodes at the
stored width, or the concrete write carries a proof that it fits, and the
consumer's ordinary transfer/observation rules authorize that store. The
compiler never truncates or invents a fitting qualification. Target-owned
checked adapters remain appropriate for normalization more complex than one
integer encoding, but are not required merely because two targets store the
same semantic field at different widths.

## Recast views

A checked recast borrows the same bytes under another stated shape when the
normalized plans prove representation compatibility:

```omega
let raw: &GdtRaw = &gdt as &GdtRaw;
let writable: &mut u32 = &mut float_bits as &mut u32;
```

The operation is representation-identity, preserves provenance/lifetime, and
is never an unchecked transmute. A shared view requires source facts to imply
target facts. A mutable view requires implication in both directions, because
every value writable through the target must leave the source valid when the
loan ends. Foreign validation or executable conversion remains an ordinary
contracted machine.

This judgment applies to values and ordinary storage, not to `Placed<P, T>` or
its accessors. Reshaping a placed view could expose a field its source access
plan made inaccessible even if the target retained the same observation class.
The first placed-view slice therefore rejects view-to-view recast. Detached
snapshot bits may still be recast as ordinary values, and a caller retaining
the underlying extent loan may request another placement through admission.

Current recast implementation supports shared scalar, bounded byte-region,
and recursively nested record/array views, plus mutable aliases whose complete
representation sets are equal. Range-bearing integer leaves normalize to exact
two's-complement bit sets; float ranges use same-carrier interval inclusion for
shared views and equality for mutable views. Typed record aliases require equal
layout geometry and equivalent leaf representations.

One direct erased-lifetime application around an otherwise eligible synthesized
record may use that exact instance layout when its lifetime arity is exact, it
has no residual runtime arguments, and no stored field recursively carries a
lifetime application. This exception applies only at the recast target root;
arrays and nested record fields do not erase additional lifetime shells into
layout authority.

Raw storage never establishes typed facts. Mutable raw-byte views require
existing total-write or Psi-proved fit evidence, while established typed views
may retain or weaken facts according to the recast judgment. `Placed<P, T>` is
excluded because its access plan adds authority not captured by representation
identity.

Unsized slices consume the complete source representation and derive length by
exact tiling. Zero-sized elements, remainders, and runtime offsets without
statically proved multi-byte congruence reject. Native and interpreter paths
preserve backing-address identity through projection and state forwarding.

## Policy selection

Bare codec calls may use the exact codec policy already named by the
destination's declared policy domain:

```omega
let save: [u8; 256] in Protobuf<Level>;
encode(level, &mut save);
```

Otherwise the call names the policy explicitly. Candidate meaning never changes
because an unrelated import adds a conformance. Third-party conformances remain
callable by name; no visible-conformance search participates.

`OmegaLayout` is the default policy family for Omega-native numbered schemas.
Foreign formats such as Protobuf or a platform ABI are sibling library
policies, not modes of the core type.

## Durability

Durability/self-description is a property of the normalized plan and the API
contract consuming it, not a semantic domain attached to arbitrary bytes. A
durable-storage API can require a policy whose plan carries stable identity and
reader-tolerance guarantees. Dropping identity for an explicitly ephemeral
cache is a different policy choice made at that boundary.

## Implementation boundary

The live slice covers normalized primitive-record plans; whole, fragmented,
and stored-width scalar placement; typed/interpreter/native projection;
representation-compatible structural recasts; bounded scalar wire repetition;
and sealed symbolic data/entry materialization. Validation is atomic with
respect to the destination. Decoding establishes no domain, trust, authority,
or device-correspondence fact. Target and OS table lifecycle remains package
work.

The settled core vocabulary is now source-visible: opaque `Placed<P, T>`, the
`Vacant` and invariant indexed `Resident<P, T>` Extent domains,
`PlacementOutcome`, `PlacementReturn`, and the empty ordinary
`PlacementCustody<P, T>` trait. This shape milestone adds no placement
operation, admission value, occurrence identity, or source-visible authority.

The first checked `PlacementCustody<P, T>` agreement is bounded to a concrete
named conformance whose exact concrete policy/schema pair already owns a
source-derived placed-view plan. Direct erased record fields absent from that
physical plan must match the custody record by canonical path, exact normalized
type, and multiplicity, while represented fields are forbidden there.
One represented acyclic, non-generic, case-free checked-record field may also
project its direct erased leaves through an authored nested custody record.
The checker preserves each complete root-to-leaf path and rejects represented
nested siblings using the enclosing field's exact plan entry. Diagnostics
retain the exact `Policy::plan` machine and represented offset/width decision.
One further represented acyclic, non-generic, case-free record may now occur on
that spine when its canonical fixed representation is nonzero. Its authored
projection preserves both enclosing field identities before the direct erased
leaves and reuses the exact root plan entry for represented-sibling diagnostics.
Third, fourth, and fifth represented record levels are also live under the same
restrictions. Their projections preserve every enclosing identity, and
bounded, memoized descendant replay fails closed when an unsupported deeper
shape could hide erased custody. This is ordinary conformance checking only; a
sixth represented record level, zero-layout wrappers, arrays, generic,
case-dependent, planless,
and establishment-operation custody remain open.

Remaining compiler and language work:

- extend the live fixed-layout `Schema` reflection and `Plan` vocabulary beyond
  the current primitive-field slice;
- exact source types for unions and runtime strides (the fixed-layout fragment
  slice uses compiler-issued field keys and `FieldEntry`);
- source-level symbolic relocation derivation and propagation of normalized
  placement constraints through linker/loader/provider artifacts, including
  provider-key establishment;
- finish `Placed<P, T>` and `Resident<P, T>` establishment/projection
  (canonical non-runtime input paths, per-claim occurrence lineage, outcome
  dispositions, generic atomic-family helper contracts, and qualified-borrow
  compatibility) and target-specific accessor lowering over
  live normalized access/resource validator; direct atomic operation-family
  gating is live for exact `bool`/`u32`/`u64` placed accessors, and
  binding-private access is enforced against the nominal policy package;
- recast syntax and diagnostics;
- independent generated-codec verification against public requirements and
  preserving-codec realizations for unknown members (artifacts already keep
  generated origin separate from compiler-admitted trust, and the standard
  package exposes the `Relayed<T>` preservation carrier);
- policy selection through generics; and
- channel/store compatibility-demand checking over published schemas, codec
  plans, historical shapes, and migrations.

The first checked non-runtime input-path carrier is bounded to direct concrete
state references to `Placed<P, T>`, including entry and subordinate states. It
retains the exact state and parameter coordinate, reference/binding mode,
synthesized view, policy, producing plan machine, schema, and complete validated
placement. Terminal retains the same closure coordinate through hermetic
declaration identities, a canonical policy/schema-derived view identity, and
the placement plan's domain-separated canonical layout/access/reach commitment;
codec and verifier
replay reject missing-machine, duplicate, reordered, malformed, or
zero-commitment rows, while canonical artifact identity binds every retained
identity string. The carrier grants no runtime or access authority. Value-form
inputs and per-outcome dispositions remain fail-closed.
