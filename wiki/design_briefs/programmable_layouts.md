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
overlays, bit ranges, variable-length wire placements, and explicit endianness.
A new format is normally a library policy; a new placement primitive requires a
compiler release.

## Plan validation

The validator proves deterministic structural rules such as:

- all referenced schema fields exist exactly where the policy permits;
- offsets, sizes, and strides are in range;
- alignments are valid;
- non-overlay fields do not overlap;
- bit ranges fit their storage slots;
- overlay/tag rules are internally consistent;
- dynamic extents are bounded by the enclosing carrier; and
- the plan normalizes to one stable identity.

Published layout/type identity is normalizer-owned. Prover strength may accept
or reject a policy conformance but never change the canonical plan or ABI key.

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
4. Plan-laid type layout and field projection.
5. Representation-compatible recast checking.
6. Hand-written codec conformances and roundtrip-law checking.
7. Home-policy resolution and artifact reporting.
8. Remove `wire data` and legacy repr/format special cases.

## Still open

- final `Schema` reflection and `Plan` source types;
- exact placement vocabulary for unions, runtime strides, and bitfields;
- recast syntax and diagnostics;
- schema-evolution law traits beyond strict roundtrip;
- policy selection through generics; and
- publish-time predecessor-plan compatibility checks.
