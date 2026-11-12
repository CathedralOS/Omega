# Ledger bisect part 33 (k=33, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

Host: linux/x86_64, 4 CPUs. `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`, `nice -n 19`.

Repository not shallow (`git rev-parse --is-shallow-repository` → `false`).

## Summary

| Slice rows | Fixed on PIN | Still broken on PIN |
| ---: | ---: | ---: |
| 11 | 4 | 7 |

Prior report slice (cluster indices 33, 73, 113, 153, 193). After re-measure on PIN, **two** diagnostic clusters remain broken (7 rows).

## Clusters still broken on PIN

| Rows | Kind | Diagnostic on PIN | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 6 | pass/run rejected | state `read` receives `holder.counters[index].value` without an exact retained backing for its loan: the incoming descriptor has no retained loan occurrence | `pass/facts/data_where_standing_borrowed_field_array_const_name` | _(in progress)_ | | completed-backings rejects indexed `data_where` reads when the descriptor has no retained loan occurrence | bisected† |
| 1 | pass/run rejected | ``quiet` does not satisfy `StepContract::step`: helper `Requirement` requires `Anchor::anchor`, but the implementation promises `Anchor::anchor` | `pass/facts/static_callback_reach_requirement_telescope` | | | | propagated‡ |

† Git bisect with fixed-PIN ledger was running at push time (`good` `12fb4455a4954a6f301015d8ebe973368a0ced0c` [#1510], `bad` PIN). Prior ledger-at-revision bisect landing on `#1509` is **invalid** under lead correction #2 (that revision fails with a different diagnostic class, exit 125). Mechanism line read from `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks/borrows/completed_backings.rs` (incoming-descriptor retained-loan check).

‡ Second cluster: witness spec prepared; bisect queued behind cluster-1 completion with the same `/tmp` fixed ledger + `bisect_test.py` + per-commit `target/bisect-$SHA` witness script.

### `data_where_standing_borrowed_field_array` members

- `pass/facts/data_where_standing_borrowed_field_array_const_name` — still broken
- `pass/facts/data_where_standing_borrowed_field_array_field` — still broken
- `pass/facts/data_where_standing_borrowed_field_array_other_machine` — still broken
- `pass/facts/data_where_standing_borrowed_field_array_other_state` — still broken
- `pass/facts/data_where_standing_borrowed_field_array_parameter` — still broken
- `pass/facts/data_where_standing_borrowed_field_array_target_body` — still broken

### `static_callback_reach` members

- `pass/facts/static_callback_reach_requirement_telescope` — still broken

## Fixed on PIN (prior slice witnesses)

- `fail/facts/static_callback_reach_requirement_telescope_requirement` (prior idx 73)
- `pass/facts/target_family_contract_operand/field` (prior idx 113)
- `pass/generics/runtime_inferred_index_statement_call_exit` (prior idx 153)
- `fail/control_flow/arm_pattern_rest_unknown_field` (prior idx 193)

## Methodology (lead correction #2)

- Fixed copies: `/tmp/ledger-bisect-part-33/pin_corpus_outcomes.txt`, `pin_observation.txt`, per-witness `*.json` from `prepare_witness.py`.
- `bisect_test.py`: exit `1` = PIN broken class; `0` = fixed-PIN ledger expectation; `125` = build failure, other outcome, or ledger-only parent (`tests/omega/corpus_outcomes.txt` / `*.md` only).
- Good commit: exponential search from PIN (`find_good_commit.sh`); witness uses isolated `CARGO_TARGET_DIR=target/bisect-$SHA` to avoid checkout cache poisoning.
- PIN witness sanity (after target fix): exit `1` ×3 at PIN; `12fb4455a49` (#1510) exit `0` as `good`.

## Commands

```bash
cp tests/omega/corpus_outcomes.txt /tmp/ledger-bisect-part-33/pin_corpus_outcomes.txt
cp /tmp/part33_observed.txt /tmp/ledger-bisect-part-33/pin_observation.txt
export BISECT_PIN_LEDGER BISECT_PIN_OBSERVATION BISECT_WITNESS_SPEC BISECT_WITNESS
python3 .agents/ledger-bisect-2026-11-11/tools/prepare_witness.py "$BISECT_WITNESS"
git bisect start --first-parent 51e8c45c793007f211d0d9baee7de16767172444 <good-sha>
git bisect run /tmp/ledger-bisect-witness-run.sh
```
