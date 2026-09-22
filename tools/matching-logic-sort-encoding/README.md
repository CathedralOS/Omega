# Matching-logic typed-to-one-sorted encoding

Deterministic encoder for the "typed-to-one-sorted encoding" leg of the
matching-logic bounded comparison ([source
draft](../../wiki/drafts/reference/matching_logic_sort_encoding.md), motivated by
[matching_logic.md](../../wiki/drafts/reference/matching_logic.md)). It turns a typed
subject schema (`cases/*.json`) into the one-sorted finitary basic fragment's
clause inventory: membership predicates and range refinements, disjointness
and definedness preconditions, pair-constructor injectivity, revision
(refinement-not-invalidation) rules, and loan/multiplicity clauses.

The encoding relocates trust, it does not discharge it: every emitted clause
is an axiom admission, listed verbatim under `admissions` in the report. No
clause here is admitted authority over an Omega proof; the output is
comparison input for the bounded-slice harness, not a checked translation.

## Commands

Python 3.9+, no third-party packages, same invocation on every host:

```sh
python3 tools/matching-logic-sort-encoding/sort_encoding.py encode \
    tools/matching-logic-sort-encoding/cases/reference.json
python3 tools/matching-logic-sort-encoding/sort_encoding.py check \
    tools/matching-logic-sort-encoding/cases/reference.json
```

- `encode` prints the clause inventory plus evidence record as JSON and
  always exits 0 on a well-formed case.
- `check` prints the same report and exits 1 when any consistency rule fails,
  so a case pins an expected violation by its exit code and diagnostic rule.

## Case schema (`omega-sort-encoding-case/1`)

A case is one JSON object:

- `types.integers`: `signed_widths`/`unsigned_widths` emit `Int{w}`/`UInt{w}`
  range refinements over the shared `Integer` membership; `ranged` emits
  `name(x) == Integer(x) & low <= x & x <= high`; `addresses` emits `Addr`;
  `slices` emit the pointee-quantifier pattern; `sums` emit the tag
  disjunction plus pairwise tag disjointness.
- `operations`: partial operations; each must carry `definedness`.
- `pair_constructor_injective`: required when slices or sums are present.
- `inhabitants`: witness count per membership class in the intended model —
  the junk-model guard. A quantified membership with zero inhabitants is a
  vacuous universal, reported as `vacuous-membership`.
- `revisions`: `prior_lower`/`prior_upper` vs `tightened_*`; a bound that
  widens is `revision-not-refinement`.
- `loans`: `kind` in `shared`/`mut`/`write` over a `region`, optional
  `parent`. Exclusive kinds may not coexist with any other live loan over a
  region; `parent` must name a live loan element or declared region.
- `fixpoints`: each entry needs `certificate`; the fragment admits none
  otherwise.

## Diagnostic rules

`missing-definedness-precondition`, `vacuous-membership`,
`revision-not-refinement`, `exclusive-loan-duplicated`,
`reborrow-without-lineage`, `pair-constructor-not-injective`,
`sum-tags-not-disjoint`, `unknown-payload-membership`,
`unknown-slice-element-membership`, `unguarded-fixpoint`.

## Evidence record

`omega-sort-encoding-record/1` reports the fields
`matching_logic_sort_encoding.md` requires: logical fragment, rule version,
semantics version (sha256 of the source draft), exact subject (name + case
digest), target capsule, observation profile, bridge graph (Omega type →
membership predicate), admissions, and diagnostics.

## Pinned cases

`cases/reference.json` is the positive case (the doc's table). Every other
file in `cases/` is a pinned negative that must produce exactly its named
diagnostic.

## Tests

```sh
python3 tools/tests/test_matching_logic_sort_encoding.py -v
```
