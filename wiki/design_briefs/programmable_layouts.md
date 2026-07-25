# Design Brief: Programmable Layouts

Current as of 2026-07-18. Layouts and codecs are authored library policies with
machine-checked conformance laws. The compiler owns a small placement vocabulary
and plan validator; it does not generate arbitrary codecs or import C's type
system.

## One data declaration

Omega has one semantic declaration form: `data`. There is no `wire data`
species. Optional field identity numbers and `retired N;` tombstones belong to
plain data schemas for policies that need durable identity.

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
`AccessPlan` describes exact transfer width, read/write/atomic permission,
stable versus externally-changing observation, generic RMW permission, and the
statically pinned boundary reach. Layout and access plans are validated as a
pair when deriving a placed view. See
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md).

The normalized validator is live in `omega-access-plans`: entries are keyed by
layout field name; exact-width accesses must fit one fixed placement/container;
external access must pin reach; exported external RMW rejects; atomic and
ordinary permissions cannot be conflated; and operation authorization preserves
shared-read/exclusive-write polarity. Validation canonicalizes authored entry
order by field identity and assigns one deterministic plan identity over every
operation, observation, transfer-width, exposure, and service-reach fact.
Equivalent name-keyed policies therefore share identity even when their source
entry order differs. Validation now produces sealed,
offset-bearing field descriptors, and borrow-specific authorization produces
the only values primitive lowering may accept. Consuming one such value now
produces a normalized primitive request bound to plan identity, admitted
placed-view grant, field identity, exact address/width, observation,
loan-derived borrow polarity/lifetime, operation-specific atomic ordering, and
static reach. Invalid atomic load/store/compare-exchange orderings reject before
emission. Omega source records, source-level borrow-carrying access values, and
target-specific primitive emission remain. The normalized Extent join is live:
provider-admitted grants validate
space, provenance, open-set rights, loan size, and permitted static reaches;
operation polarity derives from the actual Extent loan rather than a caller
argument.

## Codecs are hand-written and proved

Encoding, decoding, and validation are ordinary library machines. Omega does
not derive their bodies from `Plan`.

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

The concrete conformance proves the trait's agreement laws. Asymmetric schema
evolution—defaults, retired fields, old-era imports—belongs to the evolution
layer rather than weakening the codec's law silently.

Validation/minting is exclusive: user code cannot construct a “valid” result
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
materialization minting the established value. Domain predicates over different
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
name or second slice carrier participates.

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
7. Hand-written codec conformances and roundtrip-law checking.
8. Symbolic materializer derivation and consumer-applicability validation.
9. Home-policy resolution and artifact reporting.
10. Remove `wire data` and legacy repr/format special cases.

Implementation status: steps 1-3 are live for primitive record schemas. Step
4's source shape is live as compiler-issued field keys copied into
`FieldEntry` values; the compiler normalizes those keys back to field names,
accepts repeated `Bits` placements, and rejects unknown/missing fields, mixed
whole/fragment placement, destination overlap/out-of-bounds ranges, and source
fragments that do not tile the logical field exactly. Ordinary plan-laid value
types continue to require one fixed `At` entry per field. A target-neutral
ordinary-scalar consumer now takes only named values and this validated plan:
there is no caller-supplied offset, every planned field must be supplied
exactly once, widths and fragments are rechecked, padding/reserved bits start
at zero, and the destination changes only after complete validation. A packed
x86-64 page-table word and a compiler-evaluated compact-bit policy pin the
foundation path. The first complete x86-64 table consumer now lives in
`omega-page-tables`: it maps admitted Extent rights to that entry vocabulary,
derives four-level 4 KiB table bytes from the normalized pending mappings, and
fails closed on unknown or unrepresentable policy facts. It remains a target
consumer of plans, not a second layout mechanism or a source-visible raw writer.
Hierarchy pointers and leaves both pass through the ordinary named-field scalar
materializer. The provider retains and compares the exact normalized x86
geometry: declaration order normalizes away, but shifted hardware bits reject;
the compact layout fingerprint remains report/cache identity and never
authorizes materialization.
The AArch64 sibling uses the same normalized mappings and construction
lifecycle while retaining its independent descriptor algebra: semantic memory
class selects `AttrIndx`/shareability and access rights select AP/nG/PXN/UXN.
No common "page flags" sum erases the target distinction.
The inverse scalar decoder consumes compiler-materialized
field widths and the same named geometry, reconstructs complete logical
fields, and rejects incomplete or overlapping source fragments. Decoding
establishes no domain, trust, or authority fact; source establishment and
imported-table scanner integration remain separate validators.
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
allocation is required. Lowering that program to target-machine code remains.
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
transactional restoration after writes begin. A sealed writer preparation now
owns the exact destination, its initial content, plan, and root set, binds them
to the installed code and deterministic fingerprints, and lowers to an
address-free generated machine carrier using only private source-slot indices.
Foreign entries,
placement/root drift, and invalid destination authority reject before lowering;
numeric entry addresses never become a public API. The packed private
`IDTWRIT1` context ABI is pinned as an R10-addressed destination pointer plus
dense u64 source slots; exact x86 emission/width and the RAX/RCX/RDX/R10/R11 plus
Flags footprint are live, with unsupported ABI/architecture/slot/geometry
combinations rejected before emission. A non-clonable opaque populated seal now
binds the exact destination word and once-resolved dense source words, exposes
only identity/fingerprint/geometry, and is required by lowering and
materialization. A validated private-pointer invocation plan now supplies the
exact register copied into R10; concrete provider insertion/execution remains.

## Still open

- final `Schema` reflection and `Plan` source types;
- exact source types for unions and runtime strides (the fixed-layout fragment
  slice uses compiler-issued field keys and `FieldEntry`);
- source-level symbolic relocation derivation and propagation of normalized
  placement constraints through linker/loader/provider artifacts;
- concrete Omega `AccessPlan` record/source spelling, extent-provenance
  agreement, and sealed placed-view accessor/lowering consumers (the normalized
  validator and diagnostics are live);
- recast syntax and diagnostics;
- schema-evolution law traits beyond strict roundtrip;
- policy selection through generics; and
- publish-time predecessor-plan compatibility checks.
