# Design Brief: Programmable Layouts

Current as of 2026-07-28. Layouts and codecs are library policies with
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
canonical schema and plan identity sort numbered scopes by stable identity and
therefore do not mistake `#N` for a runtime discriminant. Fixed-layout `At` and
`Bits` placement remains limited to the reflected common/record fields; tagged
case placement belongs to the next closed-vocabulary extension.

The closed vocabulary includes only primitive placement concepts the backend
must understand: offsets/alignment, fixed and runtime strides, tagged/untagged
overlays, bit ranges, fragmented placement of one logical source across several
destination ranges, variable-length wire placements, and explicit endianness.
Entries are keyed by schema-field identity rather than by positional array
index, because fragments and overlays may contribute more than one placement
for a field. A new format is normally a library policy; a new placement
primitive requires a compiler release.

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
- the plan normalizes to one stable identity.

Published layout/type identity is normalizer-owned. Prover strength may accept
or reject a policy conformance but never change the canonical plan or ABI key.

## One plan, several derived consumers

The same normalized geometry may feed different compiler-owned consumers:

- a codec plan for bytes in buffers the program owns;
- direct field projection for ordinary plan-laid values;
- shared byte-region record views containing a plan-laid subrecord (implemented
  for fixed scalar records in both native and interpreter execution, including
  stored integer widening from projected fields on x86-64 and AArch64);
- mutable byte-region record views for recursively fact-free fixed records
  (implemented with nested plan-laid field write-through in both native and
  interpreter execution, plus x86-64/AArch64 compile rails);
- placed-view projection over an authorized external extent; or
- ordinary scalar materialization into fixed dictated structures; or
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
pair against one exact Extent loan and derives `Placed<P, T>`.

The access policy receives this validated `LayoutPlan`, so it can decide which
laid fields admit primitive access without copying offsets or transfer widths
into source. It addresses the reflected schema with compiler-issued field keys
and starts from an all-inaccessible plan. The evaluated plan has exactly one
decision per schema field; omission is denial, and declaration reorder cannot
silently reassign permissions.

Placed projection is pure and yields borrow-carrying accessors rather than
lvalues. Stable access derives ordinary mutation only when both the active
borrow and source loan are exclusive. External access is exactly once at an
admitted whole-container width and never synthesizes generic RMW. Atomic fields
expose only admitted operation families and orderings. Boundary reach belongs
to the placement, not individual fields, and runtime provenance proves that the
selected reach may touch the supplied range.

The ordinary source records now live in
`omega::language::core::layout`; its existing `Plan` record remains the current
source spelling of `LayoutPlan`. Source access policies construct their exact
schema cardinality from `AccessPlan::inaccessible(schema)` and keyed functional
replacement. The compiler evaluates `Access::plan` against a reified validated
layout, derives transfer widths from that geometry, and evaluates
`Placement::plan` into one normalized layout/access/reach identity. The
`omega-access-plans` bootstrap validates geometry, exact widths,
observation/operation compatibility, borrow polarity, atomic orderings, exact
loan facts, and sealed lowering requests. Its normalized `PlacementPlan` owns
the complete layout/access pairing and one normalized boundary reach, which
admission checks once and lowering retains. Admission consumes the exact
Extent loan, rejection returns it, and `place` consumes the accepted token.
Current view-borrow and retained source-loan polarity are checked independently.
Exposure uses the settled `BindingPrivate` spelling; stable compound mutation
is derived from read+write and exclusivity, destructive external reads remain
distinct, external compound mutation is unavailable, and atomic permissions
distinguish exact operation families. Provider supply now enters through
receipt-bound, normalized offset-keyed resource profiles. Placement/profile
compatibility restricts profiles to exact subrange loans, checks requested
observation, operations, widths, reach, and rights, derives the static base
congruence, and discharges that congruence against the concrete loan base at
admission. The normalized foundation carrier also separates pure field
projection from its event: `project`/`project_mut` return borrow-carrying
accessors whose named read, destructive-take, write, stable-compound, and
atomic-family methods are the only routes to a sealed primitive request. See
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md)
for the full `AccessPlan`, `ResourceProfile`, admission, and `Placed<P, T>`
model. Source compilation now derives unique opaque stable/external accessors
for concrete `Placed<P, T>` spellings and omits inaccessible or unauthorized
operations. Atomic fields now derive exact `bool`/`u32`/`u64` operation-family
accessors, and binding-private operations are restricted to the nominal policy
package. The source-visible loan/profile admission surface and public generic
atomic requirements remain blocked on `OWNER_QUESTIONS.md` #4 and #5;
target-specific lowering remains implementation work.

## Codecs are ordinary checked requirements

Encoding, decoding, and validation are ordinary library-machine requirements.
A realization may be authored or generated from a validated `Plan`.

```omega
trait Codec<Policy, Value> {
    machine encode(
        value: &Value,
        out: &mut [u8],
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

Implementation status (2026-07-24): shared scalar, bounded interior-byte, and
nested plan-laid record reads are live. Fact-free mutable scalar views are live
end to end over equal-width scalar places and bounded byte-region offsets.
Mutable integer views may also retain constant range facts when both sides
normalize to the exact same two's-complement bit-pattern set; this admits, for
example, `i32 [0..=100]` and `u32 [0..=100]`, while shifted equal-cardinality
ranges reject. Canonicalization merges adjacent or overlapping intervals, so
`i8 [-128..=127]` is correctly equivalent to unconstrained `u8`.
Range-refined reference binding stores a reference rather than re-establishing
the referee and is checked through this recast judgment.
Recursively fact-free mutable record views are also live over bounded byte
regions: nested ordinary/plan-laid scalar projections preserve exact offsets
and bit patterns in both native backends and the interpreter. Literal-length
fixed-array fields, including arrays of nested records and runtime-indexed
element projections, participate in the same live view. Typed record-to-record
mutable aliases are also live when total size/alignment, leaf offsets/sizes, and
every leaf representation set are equivalent. Both sides are already
established values, so the alias retains facts rather than creating them; the
live canary covers range-bearing signed/unsigned leaves and `bool` in native and
interpreter execution. Raw bytes are different. `recast` never establishes
record facts from unchecked storage; that path remains gated on validation or
materialization establishing the typed value. Domain predicates over different
carriers remain fenced until their representation sets can be proved rather
than guessed. Float ranges compose by numeric interval inclusion on the same
float carrier, with exact interval equality required for mutable aliases. The
same leaf judgment composes through typed record views. A shared view may
forget the interval into an unconstrained equal-width bit carrier; it never
justifies cross-carrier mutable equivalence.

Top-level structural targets are live on the same full-type-reference spine.
Literal-length fixed arrays apply the recursive element judgment directly.
Unsized slices consume the complete source representation and derive their
descriptor length by exact tiling:

```text
element_count = source_byte_count / target_element_byte_size
```

A zero-sized element or nonzero remainder rejects. Raw storage may target only
recursively fact-free elements; an already-typed shared view may weaken facts,
and an already-typed mutable view requires implication in both directions.
Native lowering and the interpreter preserve the backing address through
indexed reads, writes, and state-parameter forwarding. No generated semantic
name or second slice carrier participates. Aggregate elements use the same
recursive leaf judgment: a typed fixed array may be viewed as an unsized slice
of a differently named record when every repeated element preserves layout
geometry and facts. Shared range weakening and mutable exact equivalence are
live through padded element strides on both native targets and the interpreter;
a single mismatched nested leaf representation set rejects. Interior unsized
slices consume every byte after a proven start. A runtime offset may establish
a byte-element tail, while multi-byte and aggregate elements require an exact
offset so divisibility is static; an upper bound alone does not prove
congruence. Native descriptors compute the dynamic tail length from the
declared byte-region capacity and preserve mutable address identity through
state forwarding.

## Policy selection

Bare codec calls may use the destination's declared policy domain when it
selects one home conformance unambiguously:

```omega
let save: [u8; 256] in Protobuf<Level>;
encode(level, &mut save);
```

Otherwise the call names the policy explicitly. Candidate meaning never changes
because an unrelated import adds a conformance. Third-party conformances remain
callable by name; implicit selection consults only the coherent home surface.

`OmegaLayout` is the default policy family for Omega-native numbered schemas.
Foreign formats such as Protobuf or a platform ABI are sibling library
policies, not modes of the core type.

## Durability

Durability/self-description is a property of the normalized plan and the API
contract consuming it, not a semantic domain attached to arbitrary bytes. A
durable-storage API can require a policy whose plan carries stable identity and
reader-tolerance guarantees. Dropping identity for an explicitly ephemeral
cache is a different policy choice made at that boundary.

## Engineering order

1. Build-time evaluation for `Layout::plan`.
2. `Schema`, `Plan`, and the closed placement vocabulary.
3. Deterministic plan validation and normalized identity.
4. Name-keyed fragment placement and exact source/destination tiling.
5. Plan-laid type layout and field projection.
6. Representation-compatible recast checking.
7. Authored and plan-generated codec realizations with roundtrip-contract
   checking and trust classification.
8. Symbolic materializer derivation and consumer-applicability validation.
9. Home-policy resolution and artifact reporting.
10. Converge legacy repr/format paths on normalized policy plans.

Implementation status: steps 1-3 are live for primitive record schemas. Step
4's source shape is live as compiler-issued field keys copied into
`FieldEntry` values; the compiler normalizes those keys back to field names,
accepts repeated `Bits` placements, and rejects unknown/missing fields, mixed
whole/fragment placement, destination overlap/out-of-bounds ranges, and source
fragments that do not tile the logical field exactly. Ordinary plan-laid value
types accept either one fixed `At` placement or a complete set of fixed `Bits`
placements for each primitive scalar field. Direct reads assemble the logical
value from one or more fragments, and immediate writes use masked
read-modify-write operations that preserve neighboring bits; both paths are
live on x86-64 and AArch64. A target-neutral ordinary-scalar consumer takes
only named values and this validated plan: there is no caller-supplied offset,
every planned field must be supplied exactly once, widths and fragments are
rechecked, padding/reserved bits start at zero, and the destination changes
only after complete validation. A compiler-evaluated compact-bit policy pins
this generic path without naming a target subsystem. Target and OS packages
consume plans; the compiler does not own their table hierarchy, flags, or
lifecycle. The inverse scalar decoder consumes compiler-materialized field
widths and the same named geometry, reconstructs complete logical fields, and
rejects incomplete or overlapping source fragments. Decoding establishes no
domain, trust, or authority fact. Source establishment remains separate work.
The admitted `compact_binary` realization now derives bounded repeated framing
from carrier semantics: `[T; N]` contributes exactly `N` elements and
`FixedVec<T, N>` contributes its intrinsic live length up to `N`; the retired
array-plus-synthetic-count convention is gone. Borrowed byte slices use the
existing zero-copy length-delimited path. General borrowed scalar slices now
encode through a normalized runtime obligation row: descriptor element count,
two scalar passes per element, and exact packed-payload output capacity. The
generated native operation measures before emitting and allocates no staging
buffer. Packed scalar decode still needs owned or caller-provided mutable
storage, and `Vec<T>` awaits its allocator contract.
Step 8 now also has a normalized symbolic foundation: sealed
`Data(DataSymbolId) | Entry(EntryStubId)` source
identities derive resolved writes, native whole-pointer relocations, or
post-handoff writer records from the same validated plan. Loader-consumed
unresolved fragments reject, while fixed addresses may constant-fold through
the identical write path. The plan now also carries normalized permitted-range,
effective-alignment, build/load/post-handoff phase, machine-regime identity,
and artifact-installation-scope constraints; the concrete-site validator checks
all five and joins policy alignment with layout alignment. Source-level
symbolic-value derivation and final artifact propagation remain.
Post-handoff actions also derive a provider-consumable writer program. It
validates the concrete placement, resolves repeated fragments of one target
once, validates all writes before mutation, and writes directly into the
exclusive unpublished destination. A failed fill produces no publication
claim and the destination remains unpublishable; no full-table staging
allocation is required. The same normalized program now derives one
address-free reusable fragment shape plus separate invocation evidence. Dense
private source slots are assigned by first symbolic-target occurrence, so
repeated fragments resolve once without putting target identity or numeric
content into fragment identity. Exact checked encoders are live on x86-64 and
AArch64; each revalidates complete fragment/container/context geometry,
publishes its exact register/state footprint, and fingerprints emitted bytes
separately from the target-neutral fragment plan. Provider preparation now pairs
an already-lowered AOT fragment and exact footprint with an opaque once-resolved
invocation context, rejects target/installed-artifact drift, and checks that
both halves bind the same normalized fragment. It never generates host code
after installation. Context slots follow symbolic target identity, not numeric
equality, so distinct admitted entries that select one address still retain
the fragment's exact dense-slot ABI.
The object/image substrate no longer assumes relocation sites are text:
section-qualified generic `Absolute64` relocations can patch initialized data,
including PE base-rebase records. Materialized-data origin/provenance must get
an honest record rather than borrowing instruction-index sentinels; that
tagged `Instruction | Materialization` origin and native-action lowering are
now implemented. Selected-artifact entry integration now reaches canonical
executable-container v2, which carries one required entry-set section. It
validates unique compiler-issued entry identities and in-code offsets, binds
the entry-set identity through artifact admission, and allows an admitted
artifact to yield only a sealed target present in that set. Source-level data
identity derivation and final artifact propagation remain. For entry targets,
the exact `InstalledCode` state now supplies the private resolver while the
normalized writer validates its destination and every source before mutation,
resolves each target once, then writes the exclusive unpublished destination
directly. Failure produces no publication claim; it does not promise
transactional restoration after writes begin. The compiler does not synthesize
a table-specific machine carrier or own a table lifecycle. Reusable
post-handoff fragments now lower from generic writer geometry on both target
families, with normalized plan identity and emitted-byte identity explicitly
separate from invocation evidence such as exact placement, resolver, roots,
and content. A single checked provider-preparation seam binds the fragment bytes,
exact footprint, architecture, installed entry resolver, and opaque packed
context without returning numeric entry or destination addresses from the
preparation gate. Generated writer bytes are inline AOT fragments, not
independently callable runtime helpers: they deliberately carry no return
sequence, and provider preparation consumes rather than generates them.
Carrying the immutable fragment, exact footprint, and symbolic invocation plan
through final artifacts remains L6c work. Connecting that final placed fragment
to source provider code then depends on P1's authority-value/provider-key
evidence and L4/L5 materialization establishment. A consumer package may
use this machinery to build an IDT or another hardware-consumed table, but its
preparation, population, validation, and installation states remain consumer
code rather than compiler types.

## Still open

- extend the live fixed-layout `Schema` reflection and `Plan` vocabulary beyond
  the current primitive-field slice;
- exact source types for unions and runtime strides (the fixed-layout fragment
  slice uses compiler-issued field keys and `FieldEntry`);
- source-level symbolic relocation derivation and propagation of normalized
  placement constraints through linker/loader/provider artifacts;
- finish `Placed<P, T>` projection (generic atomic-family helper contracts and
  admitted-loan construction) and target-specific accessor lowering over the
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
