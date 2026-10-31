# Layout evaluation implementation

The [layout specification](../../../../wiki/spec/layouts/plans.md) owns language
meaning. These are implementation boundaries, not limits on the language.

## Ratified representation migration

The 2026-10-27 [representation-family contract](../../../../wiki/spec/layouts/plans.md#representation-applications)
replaces empty policy data, attached `Policy::plan` discovery and synthetic
generic-looking record shells with named `FixedLayout<T>` implementations and
the general `LayoutStorage<T, Policy>` application. Library families such as
CLayout use ordinary transparent generic aliases; aliases and named evidence
must be resolved by their actual declaration identities, not source spelling.
The compiler supplies authorized schema/target facts, checks compile-time
admission, evaluates and validates a plan, and retains the representation for
all storage and ABI consumers. Build grants no per-layout blessing.

The selected Core `LayoutStorage<T, Policy>` constructor carries the semantic
value and exact named implementation identity. Policies implement ordinary
`FixedLayout<T>::plan(schema, target)` members; attached plan machines, empty
policy data and synthetic record shells are retired. This establishes the
explicit fixed-policy application path. Transparent aliases, conversion
families and programmable case layouts retain their separate task acceptance
under `LIBRARY-REPRESENTATION-FAMILIES`, `PREFIX-CASE-IDENTITIES` and
`WIRE-CODEC-LIBRARY` in [TASKS.md](../../../../TASKS.md).

## Ownership

The access contract uses `AccessPolicy::plan(schema) -> AccessPlan`, containing
`LayoutPlan`, field decisions and required reach. Layout helpers return data;
the final validator independently checks complete schema coverage and permitted
operations. Seeding inaccessible decisions deduplicates keys across fragments,
not obligations across fields. `ACCESS-POLICY-VOCABULARY` owns the coordinated
core, consumer, internal-identifier and fixture migration in [TASKS.md](../../../../TASKS.md).

`layout_plans/` reflects checked schemas, evaluates authored plans and validates
them. `plan_laid.rs` discovers exact closed representation applications in the
existing Psi checking facade; `plan_laid/layout_installation.rs` installs the
validated result. Build supplies explicit deployment roots, selected providers
and immutable target facts. The facade preserves package selection authority
while ordinary compile-time admission checks the complete policy closure.
An inactive target scope with no application needs no layout facts.
`placed_views/`, `access_plans/` and `wire_plans.rs` own their respective probes
and installations. Omega supplies target inputs through those services rather
than reinterpreting policy semantics or reopening source initializers.

The [layout-plans foundation](../../foundation/layout-plans/src/layout_reports/mod.rs) holds
normalized geometry and the conventional report carriers, including the
conventional sum reports' path depth limit. The interpreter's provider storage
applies those reports through the same foundation's aggregate materialization.
Compact fingerprints are compatibility reports; exact schema rows and complete
layouts govern replay.

## Supported geometry and values

Fixed-layout normalization supports `At`, complete fragmented `Bits`, and
`IntegerAt`. Stored integers currently require positive whole-byte widths
through 64 bits. Source reflection includes records, cases, payload scopes, and
tombstones; that does not make programmable case/tag placement implemented.
The source library's fixed reflection capacities are private implementation
bounds, not schema semantics.

Fixed arrays and fixed records reflect as one `Repeated` or `Nested` field.
Whole-extent `At` preserves compiler-derived interior layout; an outer fixed
array may instead use one `At` per element at a constant nonoverlapping stride.
Sorted destination offsets determine semantic element order. Scalar fragments,
stored-width integer placement, and active field-access decisions do not apply
to these aggregate fields.

Typed-owned byte materialization has no entrance here. The record, sum,
record-with-sum and record-with-nested-sum carriers
(`ValidatedConstMaterialization` and the `ValidatedConstRecordWith*` family),
their per-shape validators and their shared replay helpers outlived tree
execution inside the compiler: no compile path, reader or consumer called them,
so they were deleted. The language's
[materialization rule](../../../../wiki/spec/language/constants.md#materialization)
still governs: a value materializes only from admitted computation or a checked
realization, under a selected layout that determines every observable bit, and
the foundation's reports plus the interpreter's provider storage carry the
aggregate path that does exist.

## Runtime layout observations

Portable checking retains layout queries until selected, validated layouts
close their values before Terminal production. Exact integer addition,
subtraction and multiplication retain bounds recipes through ordinary value
flow: locals, copied fields and records, array elements, parameters, results,
casts, dispatch and state transfers. Copies keep value snapshots; writes retire
facts about the changed storage. Each arithmetic operation retains its own
representability obligation, so later cancellation cannot excuse an overflowing
intermediate.

Proof applications follow checked invocation edges into portable helpers,
selected callbacks and provider bodies under each applicable target, including
authored bodies outside the requested output target. False or unresolved
predicates and arithmetic bounds refuse production. These facts describe
geometry only; they grant no custody, backing or memory-access authority.
They do not turn an arbitrary runtime argument into a layout constant.

[Record storage geometry](../../../../samples/cli/layouts/record_geometry/README.md)
exercises the supported value flow through native and Terminal execution;
`tests/omega/fail/layouts/` pins overflow, overwritten values and unselected
target applications.

## Target-family value policies

A plan-laid value policy may retain a finite target family. Psi evaluates every
exact checked member with the same complete invariant Schema argument, validates
every returned plan, and retains the guarded alternatives. A bad member rejects
even when deployment requests another target. Recasts and narrowed integer writes
must satisfy every relevant alternative; source checking never chooses host
geometry.

Terminal Psi retains the exact application, schema argument, family roster and
member geometries. Omega selects a member through the ordinary target-family
receipt and supplies an immutable record-geometry overlay to native lowering and
Terminal execution. The receipt keeps the original canonical artifact identity;
selection neither rewrites that artifact nor supplies new proof authority. The
complete artifact conservatively retains the policy bodies and their dependencies.

Schema ABI 3 retains per-target scalar geometry, complete member and retirement
identities, and exact policy application custody. Source references use their
source-visible geometry; engine-private loan descriptors do not change it.
Nested by-value representations are evaluated with their explicit target
inputs and lexical owners. References do not demand pointee placement.
Programmable case/tag placement and private-callback publication retain their
separate acceptance; a reflected schema alone does not establish those paths.

The retired attached-policy surface allowed an authored narrower schema input.
Ordinary `FixedLayout` has the canonical `Schema` signature instead. The former
`schema_identity_payload_*` fixtures therefore do not survive as custom
host-payload predicate coverage. Their replacement
`canonical_schema_stable_identity_*` pair checks exact `Optional<u64>` stable
identity transport and ordinary plan validation. General authored case-payload
predicates remain covered separately by `data/closed_literal_nested_predicates`
and `data/closed_literal_selected_case_false`, and canonical const arguments by
`generics/const_index_active_case_predicates_exit` and
`generics/const_index_active_case_false`.

## Conventional sum materialization

Read-only conventional reports retain the fixed four-byte tag, authored-order
ordinals, complete payload overlay, and total geometry, with the foundation's
`CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT` bounding one record path. They are not
authorable `LayoutPlan` tag placements. Outer record plans retain sum fields and
arrays as opaque whole `At` extents, and the interpreter's provider storage
applies the reports through the foundation's aggregate materialization.

Depth is report data, not a family of Rust APIs. An earlier compiler revision
kept one `ValidatedConstRecordWithSum*` / `ValidatedConstRecordWithNestedSum*`
carrier and one `validate_const_materializable_*` entrance per nesting
combination for a tree-execution route the compiler no longer has; none had a
caller, so that family is deleted with the rest of the materialization
entrances. Cycles, array-mediated nested paths, mixed common-field/case shapes,
direct sums coexisting with deeper sum paths in one record, and target-dependent
sum geometry stayed unsupported by it; deleting it widens no live path and does
not authorize flattening children into the outer schema or adding programmable
tag placement.

## Other consumers

Plan-laid value and byte-region projection cover fixed scalars, recursive fixed
records/arrays, and supported gapped outer-array layouts in native and
interpreter paths. Equal-width semantic widening is distinct from foreign
stored-width decode. Recasts have a separate
[validation note](../checking/recasts.md).

Wire-plan evaluation supports bounded scalar repetition; symbolic
materialization uses sealed data/entry identities. Source relocation derivation,
provider-key establishment, wider programmable placements, independently
verified generated codecs, and historical compatibility remain separate work.
Source API declarations or normalized plan records alone prove none of these
consumer paths complete.

## Generated compact binary codecs

The generated `compact_binary` slice supports numbered scalars, exact arrays
and `FixedVec` repeated scalars, trailing borrowed scalar encoding, a trailing
runtime-sized UTF-8 byte field, and one level of nested scalar records. Arrays
use their exact count and `FixedVec` its bounded live length, not an invented
sibling count field. Borrowed bytes use a zero-copy length-delimited path.

Borrowed scalar encoding measures then emits the packed body without allocation.
Its normalized plan retains live count, two scalar passes per element, and
capacity for the canonical length prefix plus exact body. Packed-varint decode
requires owned or caller-provided mutable scalar storage; owned `Vec` needs its
allocator contract.

Reports retain distinct normalized `Encode` and `StrictDecode` requirements,
the normalized plan and dynamic obligations, and generated origin independently
of compiler-admitted trust. `checked_interpreter::verify_wire_schema_codec`
independently checks the generated bodies -- it byte-compares canonical
emissions against a reference framing written from the contract alone,
round-trips the strict decoder, and requires malformed frames to be rejected.
That suite is differential evidence over selected members and malformed frames:
the codec spec calls such probes regression checks, not a general agreement
argument, so a fully exercised suite still does not earn `Derived` -- every
outcome keeps `Admitted` trust naming the compiler, with the exercised checks
recorded as the evidence they are. A field kind with no decode-side realization
(borrowed scalar slices, owned text) is named as a coverage gap, and a proven
divergence is a compile diagnostic. `Derived` needs agreement evidence
establishing the public requirement for every input, tied to the exact
realization and schema; that machinery remains separate work. The current
strict decoder cannot satisfy a preservation demand; preserving realization
remains separate work.
The [codec specification](../../../../wiki/spec/layouts/codecs.md) owns byte
grammar, destination establishment, and failure behavior.
