# Matching-logic external proof import

Checked importer for the "external proof import" leg of the matching-logic
bounded comparison ([source draft](../../wiki/drafts/matching_logic.md),
[design record](../../wiki/drafts/matching_logic_external_proof_import.md)).
It consumes a foreign flat-step proof export
(`external-arithmetic-proof/1`), translates each step into the bounded
slice checker's tree certificate
(`omega-matching-logic-certificate/1`), and re-checks the result through
`tools/matching-logic-slice/slice_checker.py` — the imported statement is
never trusted; only its replayed derivation is.

The importer's own gates fire before the recheck:

- every `AXIOM` step must name a case axiom, and the cited set must equal
  the case's declared `axiom_closure` exactly — understated hides a
  dependency, overstated is flagged as `closure-overstated`;
- step ids are unique and premises cite earlier ids only
  (`premise-order`, `malformed-step`);
- foreign rules outside the bridge (`unknown-external-rule`), hypothesis
  carriers (`unsupported-external-rule`), fixpoint rules
  (`fragment-escape`), and classical principles (`classical-rule-import`)
  reject by name.

The goal is rebuilt from the case's declared `obligation` — a producer
cannot substitute a weaker question (`goal`).

## Commands

Python 3.9+, no third-party packages, same invocation on every host:

```sh
python3 tools/matching-logic-external-proof-import/external_proof_import.py \
    check tools/matching-logic-external-proof-import/cases/reference.json
python3 tools/matching-logic-external-proof-import/external_proof_import.py \
    record
```

- `check` prints a JSON verdict and exits 1 when the outcome diverges from
  the case's `expect`.
- `record` runs every pinned case and writes `record.json`, the evidence
  record `matching_logic_external_proof_import.md` defines.

## Case schema (`omega-external-proof-import-case/1`)

A case reuses the slice checker's `symbols`/`refinements`/`axioms`
inventory and adds:

- `axiom_closure`: exact list of source axiom names the export may cite.
- `obligation.kind == "statement"` with `pattern`: the exact conclusion
  the translated certificate must derive. `refinement_after_transition`
  also works and delegates to the slice checker's goal builder.
- `export`: the foreign `external-arithmetic-proof/1` object — a flat
  `steps` list (premises cited by step id) plus `conclusion`.

## External rules (`external-arithmetic-proof/1`)

`AXIOM`, `MP`, `ANDI`, `ANDL`, `ANDR`, `ORIL`, `ORIR`, `EI`, `EE`, `UI`,
`REFL`, `SYM`, `TRANS`, `SUBST`, `MEMBER`, `DEF_CLOSED`, `DEF_MEMBER`
bridge to their `omega-matching-logic-certificate/1` counterparts.
`HYP`/`IMPI`/`GEN`/`ORE` carry open hypotheses and are outside this
bounded importer; `mu`/`nu`/`fixpoint` and `lem`/`dne`/`classical_choice`
reject outright.

## Diagnostic rules

`malformed-step`, `premise-order`, `unknown-external-rule`,
`unsupported-external-rule`, `fragment-escape`, `classical-rule-import`,
`undeclared-axiom`, `axiom-outside-closure`, `closure-overstated`,
`translation-recheck`, `goal`, `certificate-bound`, `schema`.

## Pinned cases

`cases/reference.json` imports a Peano-style proof of `add(2,1)=3` —
foreign flat steps, exact closure `{add_z, add_s}`, rechecked by the slice
checker. Every other file in `cases/` is a pinned negative naming its
expected diagnostic rule.

## Tests

```sh
python3 tools/tests/test_matching_logic_external_proof_import.py -v
```
