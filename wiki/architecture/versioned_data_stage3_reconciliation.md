# Versioning-Magic Retirement And Target Representation

Chapter 22 is the language authority. The filename is retained because compiler
comments and historical task records link here.

## Ruling

The implemented stage-3 model—`version` blocks, synthesized historical types,
`Versioned<T>`, an era field, version-match lowering, and migration-chain
reports—is retired language machinery.

The target language represents the same useful programs with existing semantic
categories:

| Need | Target representation |
|---|---|
| Stable members in a fluid external format | Optional field identity numbers, explicit discriminants, and tombstones on ordinary `data` |
| Byte grammar | Authored codec/layout policy and deterministic normalized plan |
| Breaking historical shape | A separately named ordinary `data` declaration |
| Decode result covering known eras | An ordinary user-declared sum |
| Era selection | Ordinary decode/framing logic followed by ordinary case dispatch |
| Migration | Ordinary machine, optionally satisfying a user/library trait |
| Validated boundary provenance | Domain evidence established by decoder contracts |
| Live replacement | Package/runtime protocol over capabilities, linear tokens, contracts, and component providers |

No semantic information may survive only inside a “versioning” flag after this
migration. Schema identity metadata belongs to schema/layout representations;
sum cases belong to ordinary type representations; migration contracts belong
to machines; component compatibility belongs to artifacts and import slots.

## Front-End Removal

Remove the dedicated surface and its derived nodes:

- parsing and validation of `version vN { ... }` members;
- lowering historical members to root types named `Type::vN`;
- reservation and synthesis of `Versioned<T>`;
- the read-only `.era` pseudo-field;
- version-specific transition-arm grammar and membership nodes;
- version-specific exhaustiveness claims; and
- version-scoped machine paths and migration-name discovery.

Ordinary record/sum patterns and ordinary exhaustiveness must cover the useful
matching behavior. Protocol packages choose explicit era names and framing.

## Semantic Representation Cleanup

Delete or generalize version-only representation fields rather than leaving a
dead parallel taxonomy:

- syntax/symbol/typed-tree version definition records;
- versioned container naming helpers and hidden payload-field names;
- version membership and interior-write special cases;
- version-chain artifact summaries; and
- schema “current era” fields that exist only to feed the old container.

Keep stable identity metadata in the ordinary data/schema representation. Keep
layout-plan identity and compatibility results in layout artifacts. If a future
lineage package requests a route graph, represent it as an ordinary normalized
plan/certificate, not by restoring version nodes to every compiler stage.

## Wire Bridge Migration

The current hard-coded wire bridge prefixes every top-level message with an era
value and compares version blocks. The target codec does neither implicitly:

- an unversioned numbered schema starts with its codec's first real field or
  framing element, not a compiler-invented era zero;
- an external format that carries an era declares it through ordinary schema
  metadata or codec framing;
- breaking eras decode into explicitly named ordinary shapes; and
- compatibility checks compare typed schema/plan artifacts and authored codec
  laws, not compiler-generated current/history tables.

Existing byte-exact tests must be deliberately re-baselined. The legacy prefix
must never be described as a stable published Omega format during the
transition.

## Corpus Migration

The `canaries/{pass,fail}/versioning` corpus is invalid as a language-surface
suite. Preserve only the general behaviors it incidentally tests:

- historical-shape type checking becomes ordinary data type checking;
- historical payload dispatch becomes ordinary sum dispatch;
- coverage failures become ordinary sum exhaustiveness failures;
- migration examples become ordinary machine/trait examples;
- boundary-only construction becomes a domain-evidence test where provenance
  matters; and
- wire era tests become explicit codec/envelope tests.

Tests whose only purpose is protecting `Versioned<T>`, version blocks, `.era`,
or special arms should be deleted with the implementation.

## Component Replacement Boundary

Do not replace one special data feature with a special deployment DSL. The
source-level `replace ... quiesce ...` plan is also retired. Cathedral is the
only planned first consumer and owns the proving slice for the orchestration
framework.

Omega still owes general substrate useful independently of Cathedral:

- core multiplicity/linearity and generic machines/traits;
- normalized machine-contract and component-artifact identities;
- separate compilation, relocatable artifacts, and provider admission;
- boundary operations for loading, dispatch installation, and pin accounting;
  and
- effects, authority, resource, progress, and failure contracts.

Only an irreducible requirement exposed by the Cathedral prototype may return
as core language/runtime work. Package verbosity is not enough.

## Completion Gate

The retirement is complete when:

1. source grammar and every semantic IR contain no version-only category;
2. ordinary sums/machines cover the migrated useful canaries;
3. the default wire bridge emits no implicit era prefix;
4. compatibility artifacts use typed schema/plan identities and predecessor
   relations;
5. the language guide contains no normative `Versioned<T>`, `Type::vN`,
   version block, or `replace` syntax; and
6. component replacement work is tracked as general substrate plus a Cathedral
   consumer slice, not as “versioned data stage 4.”
