# Ledger bisect part 22 (k=22, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

Prior-report cluster indices in this slice: 22, 62, 102, 142, 182, 222 (15 member rows, all member paths present in report — no `(+N more)`).

Re-measure on PIN (direct `corpus_runner`, `OMEGA_CORPUS_JOBS=1`): **9 fixed on main**, **6 still broken**. Re-clustered still-broken rows by first diagnostic at PIN → **3 clusters** below.

Bisect uses fixed PIN `tests/omega/corpus_outcomes.txt` and PIN witness observation at `/tmp` (methodology correction #2).

## Cluster summary (PIN)

| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | --- | --- | --- |
| 4 | pass/run regression | returned projection source permission is unavailable… | `pass/borrows/returned_slice_record_reference_custody_exit` | `f2c75870ebaf890328b787fa6d8ed50e868edbb8` | 1672 | returned-view formation now requires completed invocation access and captured borrowing parents for projected call results | bisected |

### Cluster 1 members (report idx 22)

- `pass/borrows/returned_slice_record_reference_custody_exit` still broken
- `pass/borrows/runtime_returned_stored_view_reborrow_exit` still broken
- `pass/slices/runtime_mutable_slice_result_whole_forward_exit` still broken
- `pass/wire/wire_preserving_decode_relay_exit` still broken
- `fail/borrows/returned_stored_view_reborrow_parent_write_rejected` fixed on main
- `fail/slices/returned_inclusive_window_extent_mismatch` fixed on main
- `fail/slices/window_extent_does_not_establish_bounds` fixed on main
- `fail/wire/returned_preserving_remainder_input_write` fixed on main

Good commit (walked first-parent from PIN, fixed ledger): `576c5c9433a95c1f4dcb4f18197f408afbfe92cd`

## Fixed on main (PIN)

`fail/build/explicit_provider_scalar_budget` `fail/build/explicit_provider_storage_extent` `fail/arithmetic/exact_overflow_value_call_hint` `fail/borrows/returned_stored_view_reborrow_local_escape_rejected` `fail/borrows/returned_stored_view_reborrow_parent_write_rejected` `fail/slices/returned_inclusive_window_extent_mismatch` `fail/slices/window_extent_does_not_establish_bounds` `fail/wire/returned_preserving_remainder_input_write` `pass/borrow/owned_capture_replaced_reference` `pass/facts/target_family_callback_guarantee/field`

## Commands

Host: linux x86_64. Jobs: `OMEGA_CORPUS_JOBS=1`.

```bash
bash .agents/ledger-bisect-2026-11-11/tools/prepare_bisect_witness.sh <witness> 51e8c45c793007f211d0d9baee7de16767172444 .agents/ledger-bisect-2026-11-11/pin_observed.txt
FILTER=$(paste -sd, .agents/ledger-bisect-2026-11-11/slice_members.txt)
# OMEGA_CORPUS_FIXTURE_FILTER=$FILTER OMEGA_CORPUS_RECORDS=... corpus_runner
bash .agents/ledger-bisect-2026-11-11/tools/run_cluster_bisect.sh <witness> <cluster-id>
```
