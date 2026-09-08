# Layout evaluation implementation

The [layout specification](../../../../wiki/spec/layouts/plans.md) owns language
meaning. These are implementation boundaries, not limits on the language.

## Ownership

`layout_plans/` reflects checked schemas, evaluates authored plans, validates
them, and derives complete structured materialization values. `plan_laid/`
owns pre-resolution desugaring and post-typing installation. `placed_views/`
owns concrete view probing, evaluation, and exact accessor synthesis. Omega
schedules these target-neutral services and supplies target realization inputs;
it does not reinterpret their semantics.

The [layout-plans foundation](../../foundation/layout-plans/src/lib.rs) holds
normalized geometry/report carriers. The [target layout owner](../../../omega/backend/layout/src/sum_materialization.rs)
projects conventional sum reports from its runtime layout. Compact fingerprints
are compatibility reports; exact schema rows and complete layouts govern replay.

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

Structured materialization checks complete semantic values and exact typed
schemas before deriving bytes. Erased fields remain required but emit no bytes;
erased-only nested records and arrays have no physical plan entry. Specialized
generic records use their concrete checked symbols and substituted member
types, never display names. Unsupported recursion, unresolved generics,
references, or mismatched/incomplete values reject before destination mutation.
A checked zero-argument source machine may supply the structured value.

## Conventional sum materialization

Read-only conventional reports retain the fixed four-byte tag, authored-order
ordinals, complete payload overlay, and total geometry. They are not authorable
`LayoutPlan` tag placements. Outer record plans retain sum fields and arrays as
opaque whole `At` extents. Psi rejoins exact reports and occurrence-distinct
values, reconstructing zero-padded images before one atomic outer copy.

Direct pure-sum fields, direct nonzero literal arrays of sums, and exact-depth
record paths have separate validating entrances. The nested path APIs extend
through depth 23 using shared recursive carriers and a bounded memoized walk;
each public wrapper still admits only its named depth and identity domain.
Repeated nominal types retain separate field/index occurrences. Deeper,
recursive, array-mediated nested paths, mixed common-field/case shapes,
coexisting shallower/direct-sum paths, and target-dependent sum geometry remain
unsupported by these exact-depth entrances. These bounds do not authorize
flattening children into the outer schema or adding programmable tag placement.

## Other consumers

Plan-laid value and byte-region projection cover fixed scalars, recursive fixed
records/arrays, and supported gapped outer-array layouts in native and
interpreter paths. Equal-width semantic widening is distinct from foreign
stored-width decode. Recasts have a separate
[validation note](../validation/recasts.md).

Wire-plan evaluation supports bounded scalar repetition; symbolic
materialization uses sealed data/entry identities. Source relocation derivation,
provider-key establishment, wider programmable placements, independently
verified generated codecs, and historical compatibility remain separate work.
Source API declarations or normalized plan records alone prove none of these
consumer paths complete.
