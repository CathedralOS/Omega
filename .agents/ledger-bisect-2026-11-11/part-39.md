# Ledger bisect part 39 (k=39, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`.

Slice: prior-report clusters with index `% 40 == 39` (5 clusters, 9 member rows). Host-only skipped.

## Summary at PIN

| | Count |
| --- | ---: |
| Still broken (regrouped by first diagnostic) | 3 clusters, 5 rows |
| Fixed on main (PIN ledger satisfied) | 4 rows |

## Clusters (still broken at PIN)

| id | rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | bisect |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| blockexec-suspend-ack | 2 | pass | call to `execute` has operational envelope `suspend block` but acknowledges neither suspension nor blocking | `pass/blockexec/blocking_executor_custody_claims_compile` | `2ab64136c02d7879635a92e3e078d7374bec7590` | #1811 | #1811 refactors call-site argument preparation into `ComputationCallArguments` with a canonical application roster, so blocking executor `execute` calls lose matching suspend/block acknowledgements | bisected |

### blockexec-suspend-ack members

- `pass/blockexec/blocking_executor_custody_claims_compile` still broken rejected
- `pass/blockexec/runtime_selected_worker_body_exit` still broken rejected

### Fixed on main (slice members)

- `pass/dependent/runtime_symbolic_slice_predicate_store_exit`
- `pass/facts/target_family_callback_guarantee/local`
- `pass/layouts/placed_reference_results/nested`
- `pass/generics/runtime_captured_map_borrowed_type` (ledger already records `rejected`)

## Commands

- Re-measure: direct `corpus_runner` with `OMEGA_CORPUS_FIXTURE_FILTER` (9 fixtures), jobs=1.
- Bisect: fixed PIN ledger `/tmp/ledger_bisect_pin_corpus_outcomes.txt`, per-witness baseline JSON, `bash /tmp/ledger-bisect-tools/bisect_witness.sh`, `git bisect run --first-parent`.
- Good anchor blockexec: `7a7ea313e7c76f396d15d757077251ddab8525c5` (exit 0); bad ends: `0aeed1114ae3…` and PIN (exit 1). Deterministic (3× re-run at anchors).
- Host: cloud VM, `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`.
