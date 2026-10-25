# Layout evaluation implementation

The [layout specification](../../../../wiki/spec/layouts/plans.md) owns language
meaning. These are implementation boundaries, not limits on the language.

## Ownership

`layout_plans/` reflects checked schemas, evaluates authored plans, and
validates them. `plan_laid/` owns pre-resolution desugaring and post-typing
installation. `placed_views/`, `access_plans/` and `wire_plans.rs` own concrete
view probing, placed-access policy evaluation and wire-plan installation. Omega
schedules these target-neutral services and supplies target realization inputs;
it does not reinterpret their semantics.

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

An ordinary policy whose inputs need target geometry retains that same checked
body with an independent context roster. Executed entry bindings and authored
target bodies supply exact roots; their dependency closure associates contexts
with applications before source formation. Equal result bytes never remove a
context or its policy contract. Terminal encoding distinguishes these body
applications from authored families, retains the producing body in the callable
catalog, and joins its checked declaration during codec admission. Selection
uses the exact body and context under the original artifact identity. Adapter
profiles need explicit physical deployment geometry; source preparation does
not derive it from the compiler host.

The current family Schema ABI supports records with at most 32 nonzero live
fields of scalar or fixed nested-record shape. Active cases, retired identities,
nested target-dependent schemas and nested policy geometry that has not yet been
materialized reject explicitly. Private callback layout demands also require a
separate target-closed publication route. An unresolved family is never treated
as ordinary packing by legacy typed-layout consumers.

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
