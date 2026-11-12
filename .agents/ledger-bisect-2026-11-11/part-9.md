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
| 3 | pass/run incomplete | `` `main` calls `forward`; `forward` has no executable body `` (returned reference sources) | `pass/borrows/runtime_returned_nonzero_nested_window_exit` | `2476d1be220` | 1604 | #1604 extends reference-result custody / captured-return validation so returned-reference machine transitions stop lowering | bisected |
| 1 | pass/run rejected | `terminal-artifact production failed: Lowering(ConflictingStructuralValueHome` | `pass/ownership/runtime_match_nested_observation_cleanup_exit` | `22f215eb3e7` | 2078 | #2078 reshapes case-value matching so match-observation cleanup lowers two owned homes into one structural value | bisected |

### Members (`ConflictingStructuralValueHome`)

- `pass/ownership/runtime_match_nested_observation_cleanup_exit`

### Members (`returned-reference-sources incomplete`)

- `pass/borrows/runtime_returned_nonzero_nested_window_exit`
- `pass/facts/matched_field_windows_through_other_state_exit`
- `pass/slices/runtime_returned_nested_array_window_home_exit`

**Bisect methodology (lead #2):** Fixed copies at `/tmp/ledger_bisect_corpus_outcomes.txt` and `/tmp/ledger_bisect_witness_pin.json` from PIN; `bisect_test.py` compares runtime outcome/diagnostic to PIN (not per-revision ledger). `bisect_witness.sh` skips ledger-only commits (exit 125). Good commit from first-parent walk/binary search toward PIN. Tooling copied to `/tmp/ledger-bisect-tools` so historical checkouts keep scripts.

**Cluster returned-reference:** bad=`51e8c45c7930`, good=`c15929193d39`. Stability: PIN deterministic bad; `22f12808` deterministic good.

**Cluster conflicting-home:** bad=`51e8c45c7930`, good=`5fe672430bea`.

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
