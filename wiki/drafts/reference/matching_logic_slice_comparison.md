# Matching-logic slice comparison — bounded record

Status: both columns measured; the metrics aggregation axis stays pending on `MATCHING-LOGIC-COMPARISON-METRICS` (tools/matching-logic-metrics).
Method and checklist: [matching_logic.md](matching_logic.md). Regenerate: `python3 tools/matching-logic-slice-comparison/compare.py`.
Delete this record once the metrics aggregation column lands and the `bag_view` divergence resolves.

## Provenance

- revision: `7176821bc6b9c7404d9cda02235ce486000cf1d3`
- toolchain: `nightly-2026-09-04`
- host: Linux x86_64
- checker binary: `target/debug/omega`

## Route sizes (physical lines of Rust)

| component | role | files | lines |
| --- | --- | --- | --- |
| `receiver_terminal_verifier` | receiver: Terminal Psi artifact verification | 153 | 52221 |
| `receiver_pcc_codec` | receiver: proof sidecar / bundle / trust-graph checking | 16 | 11738 |
| `admission_kernel` | admission kernel + enforced rule classifier | 21 | 5873 |
| `theory_surface` | mathematical theory the kernel checks against | 50 | 32632 |
| `derivation_support` | producer: obligations, checker, derivation store | 31 | 13849 |
| `trusted_check_total` | receiver-side total (what a receiver must run) | - | 63959 |

Translation surface on the current route: `0` — semantic products are checked natively; there is no theory translation layer to measure.

## Accepted proof-rule inventory (enforced classifier)

Source: `proof-admission/src/classicality.rs` — the `foundation` match is exhaustive over `AcceptedProofRule`, so this table is the audited inventory, not a sampled one.

| foundation | rules |
| --- | --- |
| Constructive | 14 (`Assumption`, `ConjunctionElimination`, `ConjunctionIntroduction`, `DisjunctionElimination`, `DisjunctionIntroduction`, `EqualitySymmetry`, `EqualityTransitivity`, `ImplicationElimination`, `ImplicationIntroduction`, `IntegerLessOrEqualTransitivity`, `IntegerOrderSubstitution`, `IntegerOrderWeakening`, `IntegerStrictOrderTransitivity`, `ValueEqualityTransport`) |
| ConstructiveDecidable | 8 (`IntegerAffineBound`, `IntegerCastBound`, `IntegerCorrelatedForbiddenRoots`, `IntegerExactAddDefinitionBound`, `IntegerOrderDiscreteness`, `IntegerSubtractOrder`, `PredicateDenotation`, `Primitive`) |
| TrustedAdmission | 1 (`SemanticAxiom`) |

Declared rule variants: 23; unclassified: none.

## Identical pinned positive/negative cases

Every negative declares its positive twin in its header comment; both sides exercise the same proposition. Contract: positive checks (exit 0), negative rejects emitting the corpus `expected.txt` fragment.

| subject | positive ms | negative ms | fragment observed |
| --- | --- | --- | --- |
| `add_comm` | 90.9 | 14.5 | yes |
| `bag_view` | 15.9 | 13.7 | yes |
| `congruence` | 14.7 | 11.4 | yes |
| `constant_equation` | 12.8 | 11.6 | yes |
| `climbing_sum_step` | 30.4 | 12.8 | yes |
| `climbing_sum_unbounded` | 27.2 | 15.8 | yes |
| `gauss_sum` | 20.8 | 14.3 | yes |
| `gauss_sum_step` | 19.9 | 13.4 | yes |
| `kernel_theorem_equality` | skipped | skipped | heavy pair, `--include-heavy` |
| `linear_range_sum` | 14.7 | 10.6 | yes |
| `nonlinear_square_range` | 14.0 | 10.9 | yes |
| `order_antisymmetry` | 12.9 | 11.2 | yes |
| `order_asymmetry` | 13.1 | 9.5 | yes |
| `order_transitivity` | 13.7 | 10.2 | yes |
| `ranked_accumulator` | skipped | skipped | heavy pair, `--include-heavy` |
| `remainder_range` | 14.7 | 10.1 | yes |

All 14 run positives accepted and negatives rejected: NO; expected fragments observed: yes. Skipped heavy pairs: ['kernel_theorem_equality', 'ranked_accumulator'].

### Divergences observed on the current route

- `bag_view` — positive rejected:
  `error: authored Call declaration selection occurrence 0 remained unresolved after successful checking (CheckedCall)`

## Certificate artifact

Bare-file `--check` runs emit no artifact (`wrote_output=false`), so certificate bytes are measured from a produced `.proof` / `.psi.proof` sidecar via `--native-sidecar` and currently recorded as: `pending: no --native-sidecar supplied`.

## Candidate side

The bounded slice checker is landed and measured — `tools/matching-logic-slice/slice_checker.py` (553 physical lines, sha256 `424c6c8b10aef2fa`, 22 rules). Theory: per-case axiom inventory; every clause is an admission. Fragment: `one-sorted finitary basic matching logic, no fixpoint symbols`; record schema `omega-matching-logic-slice-record/1`. Trusted bridge: none — every consumed clause is an axiom admission.

8 pinned case(s): 1 positive, 7 negative; divergences: none.

| case | expect | verdict | check ms | certificate bytes | admissions |
| --- | --- | --- | --- | --- | --- |
| `eq_subst_side` | reject | reject | 0.024 | — | 0 |
| `membership_gap` | reject | reject | 0.017 | — | 0 |
| `quantifier_escape` | reject | reject | 0.007 | — | 0 |
| `reference` | accept | accept | 0.041 | 1256 | 6 |
| `transition_reversed` | reject | reject | 0.018 | — | 0 |
| `undeclared_axiom` | reject | reject | 0.008 | — | 0 |
| `undefined_witness` | reject | reject | 0.006 | — | 0 |
| `weaker_goal` | reject | reject | 0.007 | — | 0 |

Imported admission inventory (union over cases): `axiom:counter_s0_nat`, `axiom:counter_step`, `axiom:nat_succ`, `axiom:step_s0_s1`.

The comparison-metrics aggregation column stays pending on tools/matching-logic-metrics landing.

### Sort encoding (measured translation leg)

`tools/matching-logic-sort-encoding/sort_encoding.py` — 392 physical lines (338 nonblank) across 1 file(s). Every emitted clause is an axiom admission on the candidate side. Fragment: `one-sorted finitary basic matching logic, no fixpoint symbols`; record schema `omega-sort-encoding-record/1` at semantics version `sha256:60a7179a6d7dce969db827009c6071198cda2ce6d642374d5d70bc170d9cfff9`.

1 case(s) check clean; 7 pin an expected violation.

| case | exit | admissions | diagnostics |
| --- | --- | --- | --- |
| `exclusive_loan_conflict.json` | 1 | 4 | `exclusive-loan-duplicated` |
| `missing_definedness.json` | 1 | 3 | `missing-definedness-precondition` |
| `non_injective_pair.json` | 1 | 6 | `pair-constructor-not-injective` |
| `reference.json` | 0 | 16 | — |
| `sum_payload_unknown.json` | 1 | 3 | `vacuous-membership`, `unknown-payload-membership` |
| `uncertified_fixpoint.json` | 1 | 0 | `unguarded-fixpoint` |
| `uninhabited_membership.json` | 1 | 3 | `vacuous-membership` |
| `widening_revision.json` | 1 | 3 | `revision-not-refinement` |
