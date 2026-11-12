# Ledger bisect part 33 (k=33, N=40)

Measured commit: `51e8c45c793007f211d0d9baee7de16767172444` (PIN).

Host: linux/x86_64, 4 logical CPUs. `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`, `nice -n 19`.

## Summary

| Slice rows | Fixed on PIN | Still broken on PIN |
| ---: | ---: | ---: |
| 11 | 4 | 7 |

Prior report assigned 5 clusters (indices 33, 73, 113, 153, 193); after re-measure on PIN, 2 diagnostic clusters remain broken (7 rows); 4 rows fixed on main.

## Clusters (still broken on PIN, by first diagnostic)

| Rows | Kind | Diagnostic on PIN | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 6 | pass/run rejected | state `read` receives `holder.counters[index].value` without an exact retained backing for its loan: the incoming descriptor has no retained loan occurrence | `pass/facts/data_where_standing_borrowed_field_array_const_name` | `314f00e4e5a447511ce5379fbb9de61a58c45019` | #1509 | #1509 drops the pre-validation storage graph so validation’s machine loop cannot see retained loan occurrences for indexed `data_where` reads | bisected |

### Cluster `data_where_standing_borrowed_field_array` members

- pass/facts/data_where_standing_borrowed_field_array_const_name — still broken — rejected
- pass/facts/data_where_standing_borrowed_field_array_field — still broken — rejected
- pass/facts/data_where_standing_borrowed_field_array_other_machine — still broken — rejected
- pass/facts/data_where_standing_borrowed_field_array_other_state — still broken — rejected
- pass/facts/data_where_standing_borrowed_field_array_parameter — still broken — rejected
- pass/facts/data_where_standing_borrowed_field_array_target_body — still broken — rejected

**Bisect sanity:** `--first-parent` from good `084cb11bd22ad69524a2913c4735a37ee282b6d0` (ledger `checked` stamp) to PIN; first bad `314f00e4e5a` (not PIN). Witness deterministic: good exit 0 (×3), PIN exit 1 (×3). Repository not shallow (`git rev-parse --is-shallow-repository` → false).

## Fixed on PIN (prior cluster witnesses)

- pass/facts/target_family_contract_operand/field — fixed (prior idx 113); `checked` matches ledger `checked`
- pass/generics/runtime_inferred_index_statement_call_exit — fixed (prior idx 153); matches ledger `rejected`
- fail/control_flow/arm_pattern_rest_unknown_field — fixed (prior idx 193); `rejected` + `expected`
- fail/facts/static_callback_reach_requirement_telescope_requirement — fixed (prior idx 73); `rejected` + `expected`

## Commands

```bash
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx test --no-run -p compiler --test corpus_runner < /dev/null
RUNNER=$(ls -t target/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT="$PWD" OMEGA_LIBRARY_ROOT="$PWD/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER='<comma-separated fixtures>' OMEGA_CORPUS_RECORDS=/tmp/part33_observed.txt
nice -n 19 "$RUNNER" < /dev/null

export BISECT_WITNESS=pass/facts/data_where_standing_borrowed_field_array_const_name
git bisect start --first-parent 51e8c45c793007f211d0d9baee7de16767172444 084cb11bd22ad69524a2913c4735a37ee282b6d0
git bisect run bash .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh
```
