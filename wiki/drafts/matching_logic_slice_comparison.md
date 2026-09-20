# Matching-logic slice comparison — bounded record

Status: measured Omega-side column; candidate-side columns are pending
landings of `MATCHING-LOGIC-BOUNDED-SLICE` (tools/matching-logic-slice) and `MATCHING-LOGIC-COMPARISON-METRICS` (tools/matching-logic-metrics).
Method and checklist: [matching_logic.md](matching_logic.md). Regenerate: `python3 tools/matching-logic-slice-comparison/compare.py`.

## Provenance

- revision: `1ccc88fb51cfa5d1999c152a066344acd1c8f239`
- toolchain: `nightly-2026-09-04`
- host: Linux x86_64
- checker binary: `target/debug/omega`

## Route sizes (physical lines of Rust)

| component | role | files | lines |
| --- | --- | --- | --- |
| `receiver_terminal_verifier` | receiver: Terminal Psi artifact verification | 153 | 51849 |
| `receiver_pcc_codec` | receiver: proof sidecar / bundle / trust-graph checking | 16 | 11369 |
| `admission_kernel` | admission kernel + enforced rule classifier | 21 | 5873 |
| `theory_surface` | mathematical theory the kernel checks against | 49 | 32204 |
| `derivation_support` | producer: obligations, checker, derivation store | 30 | 13011 |
| `trusted_check_total` | receiver-side total (what a receiver must run) | - | 63218 |

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
| `add_comm` | 18.0 | 12.1 | yes |
| `bag_view` | 13.8 | 11.7 | yes |
| `congruence` | 15.6 | 10.6 | yes |
| `constant_equation` | 12.6 | 11.6 | yes |
| `climbing_sum_step` | 26.2 | 12.7 | yes |
| `climbing_sum_unbounded` | 26.4 | 14.6 | yes |
| `gauss_sum` | 19.4 | 12.6 | yes |
| `gauss_sum_step` | 20.0 | 12.5 | yes |
| `kernel_theorem_equality` | skipped | skipped | heavy pair, `--include-heavy` |
| `linear_range_sum` | 13.4 | 10.3 | yes |
| `nonlinear_square_range` | 12.8 | 9.9 | yes |
| `order_antisymmetry` | 13.0 | 12.0 | yes |
| `order_asymmetry` | 12.8 | 11.8 | yes |
| `order_transitivity` | 13.1 | 10.8 | yes |
| `ranked_accumulator` | skipped | skipped | heavy pair, `--include-heavy` |
| `remainder_range` | 13.0 | 9.3 | yes |

All 14 run positives accepted and negatives rejected: NO; expected fragments observed: yes. Skipped heavy pairs: ['kernel_theorem_equality', 'ranked_accumulator'].

### Divergences observed on the current route

- `bag_view` — positive rejected:
  `error: authored Call declaration selection occurrence 0 remained unresolved after successful checking (CheckedCall)`

## Certificate artifact

Bare-file `--check` runs emit no artifact (`wrote_output=false`), so certificate bytes are measured from a produced `.proof` / `.psi.proof` sidecar via `--native-sidecar` and currently recorded as: `pending: no --native-sidecar supplied`.

## Candidate side (pending)

The matching-logic slice checker, its theory/axiom set, and its certificate format do not exist in the tree yet. When tools/matching-logic-slice lands, this record's `candidate` section is filled with the same axes over the same pinned pairs.
