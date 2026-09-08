# Layout policies and plans

Layouts are library policies over ordinary `data` declarations. They choose
physical representation without replacing the semantic fields or introducing
a language keyword for each foreign format. This contract describes the
intended language; the [implementation boundary](../../../omega-rust/psi/semantics/build-time-evaluation/layouts.md)
identifies the supported subset.

## Policy evaluation and schema

A policy satisfies `Layout` through a build-time-admissible machine
`plan(schema: Schema) -> Plan`. `Plan` is the source spelling of `LayoutPlan`
in `omega::language::core::layout`. The complete machine contract governs
evaluation. A returned plan must validate before layout, projection, recast,
codec, or ABI consumers use it.

Reflection retains fields, sum cases, structured-case payload fields, and
retired identities for each scope. Ordinary fields and cases may have stable
`#N` identities; `retired #N;` reserves removed identities. Numbering is
all-or-nothing within each record, sum, or structured payload scope. Authored
case order remains available to home-layout policies. Canonical numbered
schemas and plans order members by stable identity; these identities are not
runtime discriminants.

Stable identities are nonnegative `u64` values, unique within their member
scope and disjoint from its retired identities. A retirement declaration also
activates numbered mode. Renaming a member preserves its identity; numbering
does not itself choose source matching order or physical placement.
An erased numbered field retains semantic and historical schema identity,
including compatibility and retirement history, without codec placement, tag,
or bytes. Decode establishes its erased term by checked elaboration; erasure
never renumbers other members.

Opaque member keys, stable identities, sizes, alignment, offsets, bit widths
and indices, tags, and counts use `u64`, not `addr`. An absent stable identity
is `Optional<u64>`, not an integer sentinel. Evaluation preserves all 64 bits;
host-size conversion is checked only at the consuming allocation or slice
boundary. Compact schema/layout fingerprints are report coordinates, not
authority: consumers retain and replay the exact schema and validated plan.

## Placement vocabulary

The compiler owns a closed placement vocabulary; policies select and compose
its primitives. A new format normally needs a library policy, whereas a new
primitive needs compiler support. Entries use compiler-issued schema-field
keys, not array positions. Fragmentation or overlays may require multiple
entries for one field.

The intended vocabulary covers byte offsets and alignment, fixed/runtime
strides, tagged/untagged overlays, bit ranges and fragmented placement,
variable-length wire placement, and explicit endianness. Exact source forms
for programmable unions and runtime strides remain unspecified; conventional
sum-layout reports do not define those forms.

| Placement | Meaning |
| --- | --- |
| `At(offset)` | Whole-field placement at a byte offset. |
| `Bits(container, container_width, destination_lsb, source_lsb, width)` | Place a specified source-bit fragment in a transfer container. |
| `IntegerAt(offset, stored_width, interpretation)` | Store an integer at a byte offset using a bit-denominated width and `Signed` or `Unsigned` interpretation. |

For `Bits`, fragments tile the declaration's representation width exactly.
`bool` contributes one bit; a nonnegative constant integer range contributes
the bits needed by its maximum; unconstrained or negative-capable integers
retain their carrier width. Checked type facts determine this width. A policy
cannot omit representable bits or turn packing into truncation.

`IntegerAt` preserves the portable semantic carrier. Reads load the exact
stored width and sign- or zero-extend. Decode is total when the stored range
fits the semantic carrier. Mutable access requires either that every admitted
semantic value fits or that the concrete write proves fit, together with the
consumer's transfer and observation permission. The compiler never truncates
or invents a fitting qualification. More complex normalization may use an
ordinary checked target adapter.

## Validation and identity

A validated plan establishes:

- referenced fields belong to the schema and have the required coverage;
- sizes, offsets, strides, alignments, and bit containers are valid and bounded;
- non-overlay destinations do not overlap;
- fragments tile source bits exactly, with no unpermitted source or destination
  gaps/overlap;
- overlay/tag rules agree, and dynamic extents fit their enclosing carrier;
- private materialization demands receive exactly one compatible supply, with
  no duplicate, overlapping, or unresolved demand; and
- normalization yields one stable identity.

Published layout/type identity is normalizer-owned. Prover strength may change
whether a conformance is accepted, never the canonical plan or ABI key.
Consumer applicability is derived: declaring a flag cannot make a symbolic
relocation decodable or a variable-length wire field safe for MMIO.

Native-only [private callback demands](../build/private_callbacks.md) are
typed materialization slots, not semantic fields or author-writable holes.
Their compiler-known slot sources distinguish semantic fields, constants,
private materializations, and padding. The consuming plan must discharge them;
ordinary source projection or serialization cannot expose their contents.

## Derived consumers

One normalized geometry may feed codecs over owned byte buffers, direct
plan-laid field projection, checked shared/mutable byte-region views, placed
access over an authorized extent, ordinary value materialization, or symbolic
data/entry materialization. Each consumer checks its own applicability without
reauthoring geometry. Validation/materialization rejects incomplete or invalid
values before mutating the destination.

By-value ABI classification uses each integer field's physical stored width
and alignment, not its wider semantic carrier. Named scalar materialization
checks signed or unsigned fit before changing destination bytes. An unresolved
narrowed symbolic value rejects unless an admitted post-handoff writer retains
the exact fit constraint and checks the resolved value before writing or
publishing its context. Resolution does not license truncation.

Erased bindings remain required semantic terms and part of type identity but
have no physical field key, access decision, or initialization bytes. A
complete erased-only value can therefore materialize zeroed plan storage
without inventing a runtime field or discarding its semantic custody.

Access behavior is not layout geometry. [Placement and access](../resources/placed_access.md)
combine layout, access demand, and boundary reach; a separate provider receipt
gives a resource profile standing as supply. Layout compatibility alone grants
neither backing authority, resident custody, nor device correspondence.

## Type-position layout

A fully static policy may be attached to a data declaration:

```omega
data Descriptor in CLayout {
    kind: u32;
    address: u64;
}
```

Fields remain semantic Omega fields; `CLayout` controls placement. Packed
layout is a policy, not a `[packed]` escape hatch. Bit-addressable fields use
ordinary integer carriers with range contracts, not additional `u3`/`u17`
primitive types. `OmegaLayout` is the default policy family for Omega-native
numbered schemas; platform ABIs and foreign formats are sibling policies.
