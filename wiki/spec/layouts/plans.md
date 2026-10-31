# Layout policies and plans

Named decision: `library-authored-representation-families`, settled 2026-10-27.

Data declarations define semantic structure, never an ambient storage layout.
A storage declaration or parameter chooses a representation of that structure.
Library-authored policies compute its geometry; one general compiler mechanism
validates and applies the result. No format name receives special recognition.
This contract describes the accepted language, not completed implementation;
the [implementation boundary](../../../omega-rust/psi/compilation/compile-time/layouts.md)
identifies the supported subset and legacy migration.

Firmware structures follow the same rule: their target package supplies data
and layout policies, while the compiler validates and consumes the resulting
plans. [UEFI ownership](../build/uefi_entry.md#authored-firmware-definitions-and-adapters)
does not introduce a compiler-owned table schema or a separate layout mechanism.

## Policy evaluation and schema

A fixed-memory policy is a named implementation of the ordinary core trait
`FixedLayout<T>`. It is not a data type or a boundary service:

```omega
public trait FixedLayout<T> {
  machine plan(
    schema: Schema,
    target: LayoutTarget
  ) -> LayoutPlan
  reaches
  terminates;
}
```

The empty reach ceiling and termination guarantee apply to the complete
selected execution closure. Ordinary checked helpers are permitted; hidden
host calls, runtime-only assumed bindings, and imported effects are not.
`LayoutTarget` supplies immutable target representation facts, including scalar
geometry, alignment, endianness and applicable ABI facts. It grants no access
to host services, private source structure, or compiler storage.

`LayoutPlan` is the core representation-plan type in
`omega::language::core::layout`. It is ordinary data, not a certificate that its
contents are valid. A returned plan must validate before storage, projection,
recast, or ABI consumers use it. A package-defined implementation has the same
rights and obligations as one shipped in core or std.

For example, an intentionally sparse Darwin view may select:

```omega
public DarwinStatLayout satisfies FixedLayout<DarwinStatRecord> {
  machine plan(
    schema: Schema,
    target: LayoutTarget
  ) -> LayoutPlan {
    darwin_stat_plan(schema, target)
  }
}
```

Here `darwin_stat_plan` is an ordinary package helper returning the authored
placements. There is no empty policy data declaration, data-owned machine,
implicit receiver, or attached-`plan` naming convention. Selection follows
[named implementation rules](../language/conformances.md#declaration-and-selection).

This target-resolved `layout::Schema` is not the target-neutral
[`reflect::TypeSchema`](../language/reflection.md#schema-and-layout). Their checked
join retains exact owner, application, member, relevance, and schema revision.
Semantic graph indices are not physical field keys or byte offsets. Reflection
descriptions do not inherit the pilot layout carrier's fixed capacities; they
remain complete or report resource/production failure. Layout consumers still
obey the actual validated plan and its supported placement constraints.

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

## Size and alignment queries

The compiler-known core queries `size_of<T>()` and `align_of<T>()` return `u64`
byte counts for the exact selected target and validated layout of `T`. These
are core names, not keywords, casts, or permission to replace the layout policy.

```omega
const descriptor_bytes: u64 = size_of<CLayout<Descriptor>>();
const descriptor_alignment: u64 = align_of<CLayout<Descriptor>>();
```

`size_of` requires a fixed complete extent and includes the plan's padding.
It measures that carrier's own storage, not recursively owned allocations or
the pointee of a reference. `align_of` requires a known validated alignment;
it may be available even when the value's encoded length is dynamic. A dynamic
wire layout needs an ordinary value-specific sizing machine for its byte
length, not a fabricated fixed `size_of`. Erased fields contribute no physical
storage; a valid zero-sized layout can report zero, but a proof-only carrier
with no runtime layout cannot receive a fabricated size or alignment.

An explicit representation application identifies the geometry being measured.
A bare `size_of<T>()` or `align_of<T>()` uses the ordinary default-storage rule
for that occurrence; it does not attach a layout to the declaration of T.
Queries consume the same exact schema, target and layout-policy application as
placement and ABI. They run under ordinary semantic-evaluation admission once
those inputs are available. Interpretation reports the selected target's
layout, never the host interpreter object's allocation size. Unresolved or
circular layout dependencies reject. A query exposes geometry only: it neither
reveals private fields nor establishes backing, a valid resident value, access
rights, or recast compatibility.

These forms take data types. Executable machine entries and their late code
regions instead use [address and code-layout queries](../language/counts_and_addresses.md#native-code-layout-queries).
Neither `usize` nor a dedicated alignment carrier is introduced.

The separate value form `align_of(value)` observes a live storage location,
following a reference to its referent and a nonempty slice to its first element.
It may establish stronger alignment than a type's layout minimum; it does not
measure a reference descriptor or create access authority. Its runtime and
interpretation obligations are specified under
[location alignment](../language/counts_and_addresses.md#location-alignment).

The Rust reference's supported query flow and retained arithmetic obligations
are documented under [runtime layout observations](../../../omega-rust/psi/compilation/compile-time/layouts.md#runtime-layout-observations).

## Placement vocabulary

The compiler owns a closed placement vocabulary; policies select and compose
its primitives. A new format normally needs a library policy, whereas a new
primitive needs compiler support. Entries use compiler-issued schema-field
keys, not array positions. Fragmentation or overlays may require multiple
entries for one field. Each `FieldEntry` contains its schema `key` and
`representation: FieldPlan`; representation includes encoding and fragmentation,
not only a byte position.

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
| `Varint(tag)` | Variable-length wire placement under a schema tag. |
| `LengthPrefixed(tag)` | Length-prefixed wire placement under a schema tag. |

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

An [access plan](../resources/access.md) contains a `LayoutPlan`, per-field
operation/exposure decisions, and required boundary reach. A layout plan carries
no access policy. Ordinary layout policies remain reusable without access views;
an `AccessPolicy` composes them through ordinary checked calls, not a separate
composition trait. Their returned data is independently validated before use.
A separate provider receipt gives a resource profile standing as supply. Layout
compatibility alone grants neither backing authority, resident custody, nor
device correspondence.

## Representation applications

`LayoutStorage<T, Policy>` is the general compiler-supported core constructor
for fixed representation storage. T is the complete semantic type application;
Policy is an explicitly selected named implementation satisfying
`FixedLayout<T>`, not a data subject or an inferred visible conformance.
Its normalized application retains both selections, validated geometry, target
dependencies, ownership, qualifications and lifetime obligations. It adds no
wrapper header or separately stored T. Field projection uses T's authorized
members through the selected geometry, not an ordinary `value: T` wrapper.

Library families use [transparent generic aliases](../language/generics.md#transparent-type-aliases):

```omega
public data CLayout<T> = LayoutStorage<T, CAbiPolicy<T>>;
public data PackedLayout<T> = LayoutStorage<T, PackedPolicy<T>>;

data Descriptor {
  kind: u32;
  address: u64;
}

machine consume(descriptor: &CLayout<Descriptor>) {
  inspect_descriptor(descriptor);
}
```

`CAbiPolicy<T>` and `PackedPolicy<T>` denote ordinary generic named
implementations. Their package names and bodies are not compiler syntax.
Core or std may supply C layout; community packages may supply other policies
without changing the compiler while using its existing placement vocabulary.
Nested representation choices are explicit parts of policy composition;
an outer policy cannot accidentally inherit an unrelated native child layout.

`data T in Policy`, layout selection through domain qualification, and a
layout-specific `@` operator are not source forms. Ordinary data declarations
remain layout-neutral. An unqualified storage declaration or parameter uses
the language's default storage rules for that occurrence, not a layout ambiently
attached to T. Distinct representations do not implicitly coerce, repack, copy
linear content, or become interchangeable references. Recasts retain their
existing compatibility and custody rules; conversions are explicit
[representation conversions](codecs.md#representation-conversions).

For a complete C record, the reusable C policy computes placement from its
declared members and the target ABI. A sparse foreign view instead needs an
explicit custom policy. For example, Darwin's timestamp members include both
seconds and nanoseconds; a record listing only seconds cannot infer the omitted
foreign members or the full native extent. Layout validation establishes
internal geometry, not agreement with a foreign header or external behavior.

Packed layout is a policy, not a `[packed]` escape hatch. Bit-addressable fields
use ordinary integer carriers with range contracts, not extra integer primitives.
The default Omega storage policy, C ABI policies and custom fixed policies use
the same mechanism. Variable-length wire formats do not thereby acquire fixed
offsets, addressable fields or by-value C ABI eligibility.

## Compile-time journey and Build

The using representation application selects the exact implementation. The
compiler supplies its authorized schema and target facts, checks compile-time
admission, evaluates the policy, validates the complete returned plan, then
creates the representation application consumed by storage, projections and ABI
planning. Reflection and a plan record do not supply their own validation
authority. Equivalent applications share canonical identities under the
normalizer; a fabricated name, digest or successful helper call is not a plan
receipt. Cyclic representation dependencies reject.

The package's `build.omg` declares ordinary dependencies and target/configuration
inputs. Separate foreign calls retain their own provider and binding selection.
There is no per-layout registration, Build blessing, manual invocation of
`plan`, or layout-specific host grant. Merely importing a package does not
instantiate its layouts. Normal package review and compile-time execution
admission still apply.

Applying a layout to existing storage cannot enlarge its authorized footprint
or widen its access mode, exclusive loan, lifetime or content custody. New
storage follows ordinary allocation and storage-admission rules; a plan supplies
none of that authority. Owning mutable bytes permits writes to those bytes under
the existing rules; a layout introduces no new authority system. Encoding
address bits or reconstructing an inert pointer is not access authority. Typed
observations must establish the values and qualifications they use, and byte
writes obey existing invariant-window and restoration rules.
The actual foreign API, representation, calling plan and admitted external
contract remain explicit audit points. No geometry check proves an external
implementation honest or supplies authority over addresses encoded in storage.
