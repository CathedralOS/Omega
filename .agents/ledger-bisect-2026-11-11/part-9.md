# Ledger bisect part 9 (k=9 of 40)

Measured commit: `51e8c45c793007f211d0d9baee7de16767172444` (PIN). Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`. Regression queue slice: prior report cluster indices `9, 49, 89, 129, 169, 209` (226 clusters sorted by row count desc, tie-break diagnostic `LC_ALL=C`).

## PIN re-measurement summary

| Class | Count |
| --- | ---: |
| fixed on main (slice members) | 16 |
| still broken (PIN) | 4 |

Member recovery: 20 of 24 report rows located in `regressed_fixtures.txt` + report branch corpus batches (cluster index 9 lists 16 rows but only 12 `computed_record_window` / `may_write_computed_receiver` transports appear in the sweep list).

## Still-broken clusters at PIN (re-clustered by first diagnostic)

| Rows | Kind | Diagnostic | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 3 | pass/run incomplete | `` `main` calls `forward`; `forward` has no executable body `` (returned reference sources) | `pass/borrows/runtime_returned_nonzero_nested_window_exit` | `2476d1be220` | 1604 | #1604 changes selected-call crash/checked routing so returned-reference transition lowering no longer completes for the witness family | bisected |

### Members (`returned-reference-sources incomplete`)

- `pass/borrows/runtime_returned_nonzero_nested_window_exit`
- `pass/facts/matched_field_windows_through_other_state_exit`
- `pass/slices/runtime_returned_nested_array_window_home_exit`

**Bisect:** `git bisect start --first-parent` bad=`51e8c45c7930` good=`c15929193d39` (nearest good on first-parent walk with fixed PIN ledger snapshot). Stability: PIN 3/3 exit 1; good anchor 22f12808 3/3 exit 0 (deterministic). Prior ledger-at-revision bisect (`f3612364` / #2228) rejected as ledger-only false positive per lead correction #2.

## Prior-report slice rows now fixed on PIN

| Report index | Rows (report) | Witness (report) | PIN outcome |
| ---: | ---: | --- | --- |
| 9 | 16 | `pass/facts/computed_record_window_through_target_body_exit` | all 12 recovered members pass ledger expectation (`checked` or `built`) |
| 89 | 2 | `fail/facts/data_where_standing_mutable_recast_qualified_constant_rejected` | `rejected` with expected unexpected |
| 129 | 1 | `fail/ownership/selected_storage_join_unrelated_root` | `rejected` unexpected |
| 169 | 1 | `fail/generics/record_const_foreign_nominal_carrier` | `rejected` unexpected |

Fixed member lines:

- `pass/facts/computed_record_window_through_other_machine_exit` — fixed (checked at PIN)
- `pass/facts/computed_record_window_through_other_state_exit` — fixed (checked at PIN)
- `pass/facts/computed_record_window_through_target_body_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_call_result_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_const_name_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_field_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_local_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_match_join_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_other_machine_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_other_state_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_parameter_exit` — fixed (checked at PIN)
- `pass/facts/may_write_computed_receiver_through_target_body_exit` — fixed (checked at PIN)
- `fail/facts/data_where_standing_mutable_recast_qualified_constant_rejected` — fixed (rejected unexpected)
- `fail/facts/data_where_standing_mutable_recast_stronger_rejected` — fixed (rejected unexpected)
- `fail/ownership/selected_storage_join_unrelated_root` — fixed (rejected unexpected)
- `fail/generics/record_const_foreign_nominal_carrier` — fixed (rejected unexpected)

## Commands

```bash
# PIN re-measure (direct corpus_runner)
OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER=<members.csv> OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/work/pin_observed.txt \
  target/bisect-pool/debug/build/compiler/*/out/corpus_runner-* </dev/null

# Bisect (fixed PIN ledger + witness observation in /tmp)
BISECT_WITNESS=pass/borrows/runtime_returned_nonzero_nested_window_exit \
  .agents/ledger-bisect-2026-11-11/tools/run_cluster_bisect.sh ...
```
