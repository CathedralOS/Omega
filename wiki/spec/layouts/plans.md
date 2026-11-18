# Layout policies and plans

[Machine values](../language/machine_values.md) and [some packages](../language/some_packages.md)
have uniform descriptor/reference fields in the borrowed V1 representation.
A layout cannot inline an unknown witness, vary field offsets by that witness,
or reinterpret descriptor bits as type authority. New admissible call targets
or witnesses do not change a semantic global roster or the package layout;
ordinary code/state dependencies and boundary-layout contracts still apply.

Named decision: `library-authored-representation-families`, settled 2026-10-27.

Data declarations define semantic structure, never an ambient storage layout.
A storage declaration or parameter chooses a representation of that structure.
Library-authored policies compute its geometry; one general compiler mechanism
validates and applies the result. No format name receives special recognition.

Firmware structures follow the same rule: their target package supplies data
and layout policies, while the compiler validates and consumes the resulting
plans. [UEFI ownership](../build/uefi_entry.md#authored-firmware-definitions-and-adapters)
does not introduce a compiler-owned table schema or a separate layout mechanism.

## Home-layout realization

Named decision: `semantics-preserving-home-layout`, settled 2026-11-07.

Home-layout realization may use any representation that preserves the language's
semantics and applicable representation commitments. Field packing, tag
elimination and storage elimination are permitted examples, not an exhaustive
list. No additional source annotation or authored layout policy is required to
permit these transformations. Optional optimizations retain the ordinary
[selection and validation rules](../build/optimizations.md).

All readers, writers, projections and layout queries must agree with the selected
realization. Where home geometry is unconstrained, that realization may choose a
different size, alignment or encoding. Once a layout-dependent observation or
checked contract fixes a relationship, subsequent transformations must preserve
it. Explicit placements, ABI contracts, exposed bytes and addresses, compatible
views, loans, custody, cleanup and permitted survivor/recovery behavior constrain
the choice even when storage never crosses an ABI.

For example, a sum with a payload-free first case and a nonzero integer payload
in its second case may encode the first case as zero and the second as the
payload itself. The realization must establish that these encodings distinguish
every admitted observation and preserve zero's first-case meaning. This example
grants no fixed byte-size guarantee or general rule that a gated payload has a
niche. Failure to establish zero validity is insufficient to prove an unused
representation pattern.

[Invariant windows](../language/dependent_values.md#invariant-windows) retain
their existing source rules. A payload predicate may be temporarily broken
during repair. A compact realization must preserve legal case observations,
repair paths, cleanup and surviving storage, retaining case information where
needed or choosing another representation. Optimization cannot require an extra
window fence or per-write predicate obligation merely to fit its encoding.

A [same-storage recast](recasts.md#layout-optimization-and-recasts) constrains
both representations and every permitted aliasing observation. Jointly optimizing
them is allowed when the cast remains valid; equal resulting sizes alone are
insufficient. When a transformation cannot preserve an admitted use, it must
retain a compatible realization rather than reject that use because of the
optional optimization. Ordinary invalid layout or recast requests still reject.

The concrete representation and its relevant selection dependencies participate
in the validated plan and artifact identities consumed by layout queries,
recasts and ABI planning. The language specifies these preservation obligations;
eligibility algorithms, proof procedures and profitability heuristics belong to
the implementation and its versioned optimization policy.

## Policy evaluation and schema

A fixed-memory policy is a named implementation of the ordinary core trait
`FixedLayout<T>`. It is not a data type or a boundary service:

```omega
public trait FixedLayout<T> {
  machine plan(
    schema: Schema,
    target: LayoutTarget
  ) -> LayoutPlan
  reaches none
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
are documented under [runtime layout observations](../../../omega-rust/psi/compilation/build-time-evaluation/layouts.md#runtime-layout-observations).

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

[Typed runs](../resources/typed_runs.md) repeat an established placement for a
runtime count of owned cells. Their count does not require a runtime-sized inline
array type or settle arbitrary programmable runtime-stride syntax. The run's
cell footprint is retained with its validated layout; ordinary native placement
uses the same stride as a native array of `T`.

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

Published layout/type identity is normalizer-owned. Proof may enable a permitted
home-layout realization, whose concrete plan and selection dependencies retain
their identities. It cannot identify incompatible plans or change a committed
ABI. Canonical identity of a given plan application is independent of how its
validity was established.
Consumer applicability is derived: declaring a flag cannot make a symbolic
relocation decodable or a variable-length wire field safe for MMIO.

Native-only [private callback demands](../build/private_callbacks.md) are
typed materialization slots, not semantic fields or author-writable holes.
Their compiler-known slot sources distinguish semantic fields, constants,
private materializations, and padding. The consuming plan must discharge them;
ordinary source projection or serialization cannot expose their contents.

## Derived consumers

[Width-indexed integers](../language/numeric_values.md#width-indexed-integers)
separate semantic width from selected storage geometry. Ordinary home storage
for `u13` need not be a packed 13-bit field, and an array has its selected
element stride rather than an implicit bit packing. Multiword values have fixed
storage; their operations do not acquire hidden heap backing. Padding has no
universal zero/sign-extension promise and cannot affect numerical operations
or value-based hashing. Byte serialization and observation use explicit
representation contracts. A packed field unable to support an ordinary loan
uses the applicable accessor/codec path rather than inventing an aligned
reference. Recasts still prove validity, access and handback, not merely equal
stored sizes.

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

An [access plan](../resources/access.md) attaches to a domain and names semantic
regions through reflection, including whole-object and field rules. It contains
no `LayoutPlan`; a layout plan carries no access policy. An `AccessPolicy` consumes
the semantic `reflect::TypeSchema`, while layout policies consume the separately
target-resolved schema. Establishment joins their independently checked results
through exact semantic-member correspondence, then checks actual supply. One
field may map to several layout fragments without duplicating its authorization
rule. A separate provider receipt gives a resource profile standing as supply.
Neither layout compatibility nor an unrestricted caller list grants backing
authority, resident custody, or device correspondence.

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

## Layout demand and validation

Named decision: `demand-driven-layout-formation`, settled 2026-11-03.

Concrete target-layout evaluation is demand-driven. Do not pre-evaluate every
fixed-representation application in supplied library bodies merely because
those bodies are imported, resolved, typed or checked. A use reached by no entry,
no selected Build-row closure and no other checked consumer remains a deferred
layout application, not an accepted concrete target plan.

An entry or a target-keyed Build row demands the layout applications needed by
its complete closure, including helpers, callbacks and selected implementations.
Each row retains its own target applicability even when that target is outside
the requested output set. `--target` does not suppress validation of another
declared row's demanded layouts. Target keys remain on Build rows; source bodies
do not acquire target tags through this rule.

Storage, projections, ABI planning and compile-time operations that consume
concrete geometry must establish their exact layout demand before using it.
For example, evaluating `size_of<T>()` or a proof that needs a concrete field
offset cannot proceed with a placeholder or unvalidated plan. A required policy
still undergoes admission, evaluation and full validation with the exact schema,
representation application and authorized target facts. Layout-dependent
constants retain the ordinary multi-target agreement rules.

Every supplied implementation retains ordinary source checks, including invalid
code in unselected bodies. Those checks do not require speculative target-fact
layout formation for otherwise unused applications. A later selection or
consumer must validate the newly demanded application before it can establish
an operation's obligations, publish a successful realization or execute. Missing
facts, invalid plans, cycles and incompatible target geometry reject at that
point, never as fabricated zero sizes or successful deferred receipts.

Consequently, a package can contain a latent target-layout incompatibility that
is discovered only when a consumer demands that application. This is the
accepted diagnostic-timing trade-off, not evidence that the unused layout was
previously validated. Deferral changes when work is performed, not the validity,
authority or custody required by a consuming operation.

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
