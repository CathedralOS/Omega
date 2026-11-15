# Layout evaluation implementation

The [layout specification](../../../../wiki/spec/layouts/plans.md) defines the
language. This page covers the implementation, not limits on the language.

## Ratified representation migration

The 2026-10-27 [representation-family contract](../../../../wiki/spec/layouts/plans.md#representation-applications)
replaced empty policy data, discovery of an attached `Policy::plan` and fake
generic-looking record shells with named `FixedLayout<T>` implementations and
the general `LayoutStorage<T, Policy>` application. Library families such as
CLayout use ordinary transparent generic aliases; aliases and named evidence
are resolved by their declaration identities, not their spelling. The compiler
supplies the authorized schema and target facts, checks compile-time
admission, evaluates and validates a plan, and keeps the representation for
every storage and ABI consumer. Build does not approve layouts one by one.

The Core `LayoutStorage<T, Policy>` constructor carries the semantic value and
the exact named implementation identity. Policies implement ordinary
`FixedLayout<T>::plan(schema, target)` members; the old attached plan machines,
empty policy data and record shells are gone. This gives explicit fixed
policies a working path. Transparent aliases, conversion families and
programmable case layouts are still separate tasks, `LIBRARY-REPRESENTATION-FAMILIES` and `PREFIX-CASE-IDENTITIES` in
[TASKS.md](../../../../TASKS.md).

## Ownership

The [domain access contract](../../../../wiki/spec/resources/access.md) uses
`AccessPolicy::plan(reflect::TypeSchema) -> AccessPlan`. The plan holds semantic
region rules, limits on who may establish, read and write, observation demands
and reach, but no layout. If the policy names no callers, any caller is
allowed; an explicit empty list denies everyone. Establishment joins the plan
to the separate layout and the actual backing storage. `DOMAIN-ACCESS-AUTHORITY`
in [TASKS.md](../../../../TASKS.md) covers moving core, compiler, source and
fixtures over, including removing the separate view shell and old domain
clauses. A layout field key is not interchangeable with a reflected member
identity.

`layout_plans/` reflects checked schemas, then evaluates and validates authored
plans. `plan_laid.rs` finds exact closed representation applications in the Psi
checking facade; `plan_laid/layout_installation.rs` installs the validated
result. Build supplies explicit deployment roots, selected providers and fixed
target facts. The facade keeps package selection authority while ordinary
compile-time admission checks every policy reached. An inactive target with no
application needs no layout facts. `access_views/`, `access_plans/` and
`wire_plans.rs` do the same probing and installing for their kinds. Omega passes
target inputs through these services; it never reinterprets policy or reopens
source initializers.

The [layout-plans foundation](../../foundation/layout-plans/src/layout_reports.rs)
holds normalized geometry and the conventional report types, including the path
depth limit for conventional sum reports. The interpreter's provider storage
applies those reports through the foundation's aggregate materialization.
Compact fingerprints only report compatibility; replay uses the exact schema
rows and complete layouts.

## Supported geometry and values

Fixed-layout normalization supports `At`, complete fragmented `Bits`, and
`IntegerAt`. Stored integers must be a whole number of bytes, up to 64 bits.
Source reflection covers records, cases, payload scopes and tombstones, but
programmable case and tag placement is not implemented. The source library's
fixed reflection capacities are private limits, not schema meaning.

Fixed arrays and fixed records reflect as one `Repeated` or `Nested` field.
An `At` over the whole extent keeps the interior layout the compiler derived;
an outer fixed array may instead use one `At` per element at a constant,
non-overlapping stride. Element order follows the sorted destination offsets.
Scalar fragments, stored-width integer placement and active field-access
decisions do not apply to these aggregate fields.

Nothing here turns typed owned values into bytes. The record, sum,
record-with-sum and record-with-nested-sum types
(`ValidatedConstMaterialization` and the `ValidatedConstRecordWith*` family),
their per-shape validators and shared replay helpers outlived the compiler's
tree execution; nothing called them, so they were deleted. The
[materialization rule](../../../../wiki/spec/language/constants.md#materialization)
still applies: a value becomes bytes only from admitted computation or a
checked realization, under a selected layout that fixes every observable bit.
The foundation's reports and the interpreter's provider storage carry the
aggregate path that exists.

## Runtime layout observations

Portable checking keeps layout queries open until selected, validated layouts
give them values, before Terminal Psi is produced. Exact integer addition,
subtraction and multiplication carry their bounds through ordinary value flow:
locals, copied fields and records, array elements, parameters, results, casts,
dispatch and state transfers. A copy keeps a snapshot of the value; a write
drops facts about the storage it changed. Each arithmetic operation must fit
its type on its own, so a later cancellation cannot excuse an intermediate
overflow.

Proofs follow checked calls into portable helpers, selected callbacks and
provider bodies for each applicable target, including authored bodies for
targets other than the one requested. A false or unresolved predicate or bound stops
production. These facts describe geometry only: they
grant no custody, backing or memory access, and do not turn an arbitrary
runtime argument into a layout constant.

[Record storage geometry](../../../../samples/cli/probes/record_geometry/README.md)
runs the supported value flow natively and in the Terminal interpreter.

## Target-family value policies

A plan-laid value policy may have a finite family of targets. Psi evaluates
every checked member with the same full Schema argument, validates every
returned plan and keeps the guarded alternatives. A bad member is refused even
when deployment asks for another target. Recasts and narrowed integer writes
must satisfy every relevant alternative; source checking never picks host
geometry.

Terminal Psi keeps the exact application, schema argument, family members and
their geometries. Omega selects a member through the ordinary target-family
receipt and gives native lowering and Terminal execution a fixed record-geometry
overlay. The receipt keeps the original artifact identity; selection neither
rewrites it nor adds proof authority. The artifact conservatively keeps all
policy bodies and their dependencies.

Schema ABI 4 keeps per-target geometry, complete member and retirement
identities, exact policy application custody and `FieldLayoutSource`. Scalar
geometry and explicitly selected `LayoutStorage` geometry remain distinct from
default aggregate/reference storage. Fixed arrays inherit their element source;
an explicit child stops that walk. Consumers rebuild the source from the exact
field type and selected child target, never from equal sizes or a policy's
display name. Source references use their source-visible geometry; the
engine's private loan descriptors do not change it.
Nested by-value representations are evaluated with their explicit target
inputs and lexical owners. References do not demand pointee placement.
Programmable case and tag placement and private-callback publication are
separate work; a reflected schema alone does not provide them.

The retired attached-policy surface let an author narrow the schema input.
Ordinary `FixedLayout` has the canonical `Schema` signature instead. None of
the fixtures that once covered either surface remain.

## Conventional sum materialization

Read-only conventional reports keep the fixed four-byte tag, ordinals in
authored order, the full payload overlay and total geometry; the foundation's
`CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT` caps the length of one record path.
Authors cannot write these as `LayoutPlan` tag placements. Outer record plans
treat sum fields and arrays as opaque whole `At` extents, and the interpreter's
provider storage applies the reports through the foundation's aggregate
materialization.

Depth is data in the report, not a family of Rust APIs. An earlier compiler
kept one `ValidatedConstRecordWithSum*` / `ValidatedConstRecordWithNestedSum*`
type and one `validate_const_materializable_*` function per nesting combination,
for a tree-execution route that no longer exists. None had a caller, so they
were deleted with the other materialization functions. They never supported
cycles, nesting through arrays, mixed common-field and case shapes, a direct
sum beside deeper sum paths in one record, or target-dependent sum geometry.
Deleting them widens no live path, and does not permit flattening children
into the outer schema or adding programmable tag placement.

## Other consumers

Plan-laid value and byte-region projection handle fixed scalars, nested fixed
records and arrays, and supported outer-array layouts with gaps, both natively
and in the interpreter. Widening to an equal-width semantic type is not the
same as decoding a foreign stored width. Recasts have a separate
[validation note](../checking/recasts.md).

Wire-plan evaluation supports bounded scalar repetition; symbolic
materialization uses sealed data and entry identities. Still separate work:
deriving source relocations, establishing provider keys, wider programmable
placements, independently verified generated codecs and historical
compatibility. Neither source API declarations nor normalized plan records
show that any of these paths works.

## Generated compact binary codecs

Generated `compact_binary` codecs support numbered scalars, repeated scalars in
exact arrays and `FixedVec`, trailing borrowed scalar encoding, a trailing
runtime-sized UTF-8 byte field and one level of nested scalar records. Arrays
use their exact count and `FixedVec` its bounded live length, never an invented
count field. Borrowed bytes use a zero-copy, length-prefixed path.

Borrowed scalar encoding measures, then writes the packed body without
allocating. Its plan records the live count, two scalar passes per element, and
room for the length prefix plus the exact body. Packed-varint decoding needs
owned or caller-provided mutable scalar storage; an owned `Vec` needs its
allocator contract.

Reports keep separate `Encode` and `StrictDecode` requirements, the plan, its
dynamic obligations and the generated origin, apart from compiler-admitted trust. The codec verifier independently checks the generated bodies: it compares their output byte for byte against a reference framing
written from the contract alone, round-trips the strict decoder, and requires
malformed frames to be rejected. That suite compares behavior on selected
members and malformed frames; the codec spec calls such probes regression
checks, not proof of agreement. So even a fully passing suite does not earn
`Derived`: every outcome keeps `Admitted` trust naming the compiler, with the
checks recorded as what they are. A field kind with no decoder (borrowed scalar
slices, owned text) is reported as a coverage gap, and a proven mismatch is a
compile diagnostic. `Derived` needs evidence that the public requirement holds
for every input, tied to the exact realization and schema; that is separate
work. The strict decoder cannot yet satisfy a preservation demand; a preserving
realization is also separate work.
The [codec specification](../../../../wiki/spec/layouts/codecs.md) owns byte
grammar, destination establishment, and failure behavior.
