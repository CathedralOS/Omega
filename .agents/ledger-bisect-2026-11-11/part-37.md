# Ledger bisect part 37 (k=37, N=40)

Measured commit: `51e8c45c793007f211d0d9baee7de16767172444` (PIN; unchanged).

## Summary

| Class | Count |
| --- | ---: |
| fixed on main | 10 |
| still broken | 0 |
| slice member rows | 10 |

Prior report (#2359) assigned **5** clusters (indices 37, 77, 117, 157, 197 among 226 clusters sorted by row count). After re-measure on the PIN, **every member row matches the ledger** (`is_worse_than_ledger` is false for all). No still-broken rows remained to re-cluster; **no `git bisect run` was executed**.

## Clusters still broken at PIN

(none)

## Prior slice clusters (all members fixed on PIN)

| Rows | Kind (prior) | Diagnostic at PIN | Witness | Breaking (prior report) | Mechanism | Bisect |
| ---: | --- | --- | --- | --- | --- | --- |
| 5 | fail wrong reason | cannot prove index `3` is within unknown slice length of `view` in read_guarded::entry | `fail/facts/slice_view_element_read_bound_through_call_result_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` | — | n/a (fixed on PIN) |
| 2 | pass/run rejected | (matches ledger `checked`; same diagnostic family as prior) | `pass/facts/target_family_callback_guarantee/other_machine` | `f3612364ffbb9489d7be1409397df6867fcbebc6` | — | n/a (fixed on PIN) |
| 1 | pass/run rejected | (matches ledger `checked`) | `pass/facts/target_family_callback_guarantee/call_result` | `f3612364ffbb9489d7be1409397df6867fcbebc6` | — | n/a (fixed on PIN) |
| 1 | pass/run rejected | cannot prove requires contract for call observe from main: first[0] in Cell::Issued… | `pass/borrow/owned_capture_replaced_reference` | (prior propagated table) | — | n/a (fixed on PIN) |
| 1 | fail wrong reason | partial exact remainder … divisor must be proven nonzero | `fail/arithmetic/zii_range_excludes_zero_rejected` | `f3612364ffbb9489d7be1409397df6867fcbebc6` | — | n/a (fixed on PIN) |

### Member lines (re-measure)

`fail/facts/slice_view_element_read_bound_through_call_result_rejected` — fixed on main; rejected; cannot prove index `3` is within unknown slice length of `view` in read_guarded::entry

`fail/facts/slice_view_element_read_bound_through_const_name_rejected` — fixed on main; rejected; cannot prove index `3` is within unknown slice length of `view` in read_guarded::entry

`fail/facts/slice_view_element_read_bound_through_field_rejected` — fixed on main; rejected; cannot prove index `3` is within unknown slice length of `holder.view` in read_guarded::entry

`fail/facts/slice_view_element_read_bound_through_local_rejected` — fixed on main; rejected; cannot prove index `3` is within unknown slice length of `v` in read_guarded::entry

`fail/facts/slice_view_element_read_bound_through_match_join_rejected` — fixed on main; rejected; cannot prove index `3` is within unknown slice length of `v` in read_guarded::entry

`pass/facts/target_family_callback_guarantee/other_machine` — fixed on main; checked (ledger: checked)

`pass/facts/target_family_callback_guarantee/other_state` — fixed on main; checked (ledger: checked)

`pass/facts/target_family_callback_guarantee/call_result` — fixed on main; checked (ledger: checked)

`pass/borrow/owned_capture_replaced_reference` — fixed on main; rejected (ledger: rejected); cannot prove requires contract for call observe from main: first[0] in Cell::Issued in state `entry`

`fail/arithmetic/zii_range_excludes_zero_rejected` — fixed on main; rejected (ledger: rejected expected); partial exact remainder in machine `main` state `entry` local `remainder`: the divisor must be proven nonzero

## Fixed-on-main rows

All 10 slice members (listed above).

## Measurement provenance

- Host: `cursor` (linux), 4 CPUs
- Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`, `nice -n 19`
- Build: `CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null`
- Corpus runner: `mbx test --no-run -p compiler --test corpus_runner < /dev/null`
- Direct measure:

```bash
cd /agent/repos/omega
FILTER=$(paste -sd, /tmp/part37_fixtures.txt)
RUNNER=target/debug/build/compiler/beaa8c7355c9ba28/out/corpus_runner-beaa8c7355c9ba28
export OMEGA_CORPUS_ROOT=$PWD OMEGA_LIBRARY_ROOT=$PWD/source/library
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS=/tmp/part37_observed.txt
nice -n 19 "$RUNNER" < /dev/null
```

- Classification: `wiki/drafts/audits/tools/classify_corpus.py` against `tests/omega/corpus_outcomes.txt` at PIN
