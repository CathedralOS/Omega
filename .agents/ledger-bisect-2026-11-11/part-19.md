# Ledger bisect part 19 (k=19, N=40)

Measured on PIN `51e8c45c793007f211d0d9baee7de16767172444`. Prior-report clusters indexed by descending row count; slice `index % 40 == 19` (6 clusters, 16 rows). Host-only skipped.

## Summary

| Metric | Count |
| --- | ---: |
| Slice rows re-measured | 16 |
| Fixed on PIN (vs fixed PIN ledger) | 15 |
| Still broken on PIN | 1 |
| Clusters still broken after re-cluster (first diagnostic at PIN) | 1 |
| Bisected (compiler commits) | 1 |
| Propagated | 0 |

Witness stability (PIN, fixed `/tmp` ledger + observation): **deterministic** — three successive `bisect_witness.sh` runs exit `1` after full `mbx build` + `corpus_runner` rebuild. Earlier inconsistency was **flake** from reusing a stale `corpus_runner` without `mbx test -p compiler --test corpus_runner --no-run` (wrong first diagnostic). Rebuilt `target/` after permission errors.

Bisect methodology: PIN-frozen `tests/omega/corpus_outcomes.txt` at `/tmp/ledger_bisect_corpus_outcomes.txt`; PIN witness observation at `/tmp/ledger_bisect_pin_observation.json`. Skip ledger-only commits (`BISECT_LOG` + diff only `corpus_outcomes.txt` / `*.md`). Good `f7e385cfff729ef831204e00de721bbe0484523a` found by binary search on first-parent `573463373293..PIN`. Prior report pointer `f3612364` (#2228) was ledger-only (invalid under corrected methodology).

## Clusters (prior grouping, PIN re-measure)

| Rows | Kind | Diagnostic on PIN | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 9 | pass/run rejected | (all fixed on PIN) | `pass/facts/finite_family_exclusive_result/call_result` | — | — | Fixed on PIN | n/a |
| 3 | pass/run rejected | (all fixed on PIN) | `pass/facts/static_callback_reach_evidence_scope` | — | — | Fixed on PIN | n/a |
| 1 | pass/run rejected | IntegerAt store in machine `guarded_write`… 8-bit signed storage | `pass/layouts/runtime_plan_laid_integer_at_proved_write_exit` | `c9a5d7d4cb7464dd48481032e2ee4819fdf7c6f5` | #209 | Build execution keeps Terminal activations through source/Product joins, breaking plan-laid IntegerAt proved writes | bisected |
| 1 | pass/run rejected | (fixed on PIN) | `pass/atomics/selected_reference_atomic_provider` | — | — | Fixed on PIN | n/a |
| 1 | fail wrong reason | (fixed on PIN) | `fail/layouts/plan_laid_policy_without_plan_machine` | — | — | Fixed on PIN | n/a |
| 1 | pass/run rejected | (fixed on PIN) | `pass/proofs/exhaustive_nonreturn_contract` | — | — | Fixed on PIN | n/a |

## Members

### Cluster finite_family_exclusive_result (9 rows) — 9 fixed, 0 broken

- `pass/facts/finite_family_exclusive_result/call_result` fixed checked
- `pass/facts/finite_family_exclusive_result/const_name` fixed checked
- `pass/facts/finite_family_exclusive_result/field` fixed checked
- `pass/facts/finite_family_exclusive_result/local` fixed checked
- `pass/facts/finite_family_exclusive_result/match_join` fixed checked
- `pass/facts/finite_family_exclusive_result/other_machine` fixed checked
- `pass/facts/finite_family_exclusive_result/other_state` fixed checked
- `pass/facts/finite_family_exclusive_result/parameter` fixed checked
- `pass/facts/finite_family_exclusive_result/target_body` fixed checked

### Cluster static_callback (3 rows) — 3 fixed, 0 broken

- `pass/facts/static_callback_reach_evidence_scope` fixed checked
- `pass/generics/runtime_captured_map_two_method_tuples` fixed checked
- `run/calls/nested_static_implementation_shadow` fixed checked

### Cluster plan_laid_integer_at_proved_write (1 row) — 0 fixed, 1 broken

- `pass/layouts/runtime_plan_laid_integer_at_proved_write_exit` broken rejected — IntegerAt store in machine `guarded_write` state `entry` assignment requires a value proven to fit 8-bit signed storage

### Cluster atomics (1 row) — 1 fixed

- `pass/atomics/selected_reference_atomic_provider` fixed checked

### Cluster fail plan_laid policy (1 row) — 1 fixed

- `fail/layouts/plan_laid_policy_without_plan_machine` fixed rejected (expected)

### Cluster exhaustive_nonreturn (1 row) — 1 fixed

- `pass/proofs/exhaustive_nonreturn_contract` fixed checked

## Fixed-on-main (all slice rows not listed under broken)

15 rows above marked fixed; sole remaining failure is `pass/layouts/runtime_plan_laid_integer_at_proved_write_exit`.

## Commands

```bash
PIN=51e8c45c793007f211d0d9baee7de16767172444
git checkout -B work/ledger-bisect-part-19 "$PIN"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx test -p compiler --test corpus_runner --no-run < /dev/null
# 16-fixture re-measure: OMEGA_CORPUS_FIXTURE_FILTER=... OMEGA_CORPUS_RECORDS=/tmp/part19_measure.txt corpus_runner
export BISECT_WITNESS=pass/layouts/runtime_plan_laid_integer_at_proved_write_exit
.agents/ledger-bisect-2026-11-11/tools/pin_bisect_prepare.sh "$BISECT_WITNESS" "$PIN"
GOOD=f7e385cfff729ef831204e00de721bbe0484523a
git bisect start --first-parent "$PIN" "$GOOD"
git bisect run "$(pwd)/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
```

Host: cloud test VM. `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`, `nice -n 19`.

Bisect good ends: `f7e385cfff729ef831204e00de721bbe0484523a` (exit 0 ×2). Bad end: PIN (exit 1 ×2). First bad: `c9a5d7d4cb7464dd48481032e2ee4819fdf7c6f5` — **not** PIN.
