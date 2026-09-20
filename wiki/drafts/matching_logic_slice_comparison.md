# Matching-logic slice comparison — bounded record

Status: measured Omega-side column; candidate-side columns are pending
landings of `MATCHING-LOGIC-BOUNDED-SLICE` (tools/matching-logic-slice) and `MATCHING-LOGIC-COMPARISON-METRICS` (tools/matching-logic-metrics).
Method and checklist: [matching_logic.md](matching_logic.md). Regenerate: `python3 tools/matching-logic-slice-comparison/compare.py`.

## Provenance

- revision: `b972133cade41a186b6b0f2eb072a41cc17f4613`
- toolchain: `nightly-2026-09-04`
- host: Linux x86_64
- checker binary: `target/debug/omega`

## Route sizes (physical lines of Rust)

| component | role | files | lines |
| --- | --- | --- | --- |
| `receiver_terminal_verifier` | receiver: Terminal Psi artifact verification | 153 | 52048 |
| `receiver_pcc_codec` | receiver: proof sidecar / bundle / trust-graph checking | 16 | 11434 |
| `admission_kernel` | admission kernel + enforced rule classifier | 21 | 5873 |
| `theory_surface` | mathematical theory the kernel checks against | 49 | 32204 |
| `derivation_support` | producer: obligations, checker, derivation store | 30 | 13011 |
| `trusted_check_total` | receiver-side total (what a receiver must run) | - | 63482 |

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
| `add_comm` | 20.9 | 15.0 | yes |
| `bag_view` | 17.0 | 15.4 | yes |
| `congruence` | 18.0 | 13.9 | yes |
| `constant_equation` | 15.4 | 14.6 | yes |
| `climbing_sum_step` | 34.9 | 20.0 | yes |
| `climbing_sum_unbounded` | 36.8 | 18.1 | yes |
| `gauss_sum` | 27.2 | 19.8 | yes |
| `gauss_sum_step` | 25.3 | 16.7 | yes |
| `kernel_theorem_equality` | skipped | skipped | heavy pair, `--include-heavy` |
| `linear_range_sum` | 19.2 | 12.9 | yes |
| `nonlinear_square_range` | 15.9 | 205.0 | yes |
| `order_antisymmetry` | 17.9 | 13.9 | yes |
| `order_asymmetry` | 17.4 | 13.6 | yes |
| `order_transitivity` | 16.6 | 13.2 | yes |
| `ranked_accumulator` | skipped | skipped | heavy pair, `--include-heavy` |
| `remainder_range` | 16.6 | 13.1 | yes |

All 14 run positives accepted and negatives rejected: NO; expected fragments observed: yes. Skipped heavy pairs: ['kernel_theorem_equality', 'ranked_accumulator'].

### Divergences observed on the current route

- `bag_view` — positive rejected:
  `error: authored Call declaration selection occurrence 0 remained unresolved after successful checking (CheckedCall)`

## Certificate artifact

Bare-file `--check` runs emit no artifact (`wrote_output=false`), so certificate bytes are measured from a produced `.proof` / `.psi.proof` sidecar via `--native-sidecar` and currently recorded as: `pending: no --native-sidecar supplied`.

## Candidate side

The matching-logic slice checker, its theory/axiom set, and its certificate format do not exist in the tree yet; those columns stay pending until tools/matching-logic-slice and tools/matching-logic-metrics land.

### Sort encoding (measured translation leg)

`tools/matching-logic-sort-encoding/sort_encoding.py` — 392 physical lines (338 nonblank) across 1 file(s). Every emitted clause is an axiom admission on the candidate side. Fragment: `one-sorted finitary basic matching logic, no fixpoint symbols`; record schema `omega-sort-encoding-record/1` at semantics version `sha256:4ea73dbecf672cf73d34a0d8d82a842406965216a1920a4e1d2d9b81e2f906df`.

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
