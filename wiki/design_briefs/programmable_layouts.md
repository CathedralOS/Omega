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
  for fixed scalar records in both native and interpreter execution);
- placed-view projection over an authorized external extent; or
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
shared-read/exclusive-write polarity. Validation now produces sealed,
offset-bearing field descriptors, and borrow-specific authorization produces
the only values primitive lowering may accept. Omega source records,
source-level borrow-carrying access values, and the exact primitive lowering
remain. The normalized Extent join is live: provider-admitted grants validate
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

## Recast views

A checked recast borrows the same bytes under another stated shape when the
normalized plans prove representation compatibility:

```omega
let raw: &GdtRaw = &gdt recast GdtRaw;
```

Exact spelling remains provisional. The operation is representation-identity,
cannot strengthen semantic facts, preserves provenance/lifetime, and is never
an unchecked transmute. Foreign validation or executable conversion remains an
ordinary contracted machine.

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
types continue to require one fixed `At` entry per field. Step 8 now has a
normalized foundation: sealed `Data(DataSymbolId) | Entry(EntryStubId)` source
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
once, stages every write, and leaves the destination unchanged on failure.
Lowering that program to target-machine code remains.
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
normalized writer validates its destination, resolves each target once, stages
all fragments, and publishes atomically. Foreign entries and data symbols fail
without changing the destination; numeric entry addresses never become a
public API. Target-machine emission of the writer remains.

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
