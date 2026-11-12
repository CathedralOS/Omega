# Ledger bisect part 22 (k=22, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

Prior-report cluster indices in this slice: 22, 62, 102, 142, 182, 222 (**15** member rows; full paths in report, no `(+N more)`).

**Re-measure on PIN** (direct `corpus_runner`, `OMEGA_CORPUS_JOBS=1`): **9 fixed on main**, **6 still broken**. Re-clustered still-broken rows by first diagnostic at PIN → **3 clusters**.

Bisect uses **fixed** PIN `tests/omega/corpus_outcomes.txt` plus witness PIN observation under `/tmp/ledger_bisect_*` (methodology correction #2). Ledger-only first-parent commits skipped (exit 125). Good commit = walk first-parent from PIN with fixed ledger until witness returns 0.

| Slice totals | Count |
| --- | ---: |
| Prior-report rows in slice | 15 |
| Fixed on PIN | 9 |
| Still broken on PIN | 6 |
| Clusters after PIN re-group | 3 |
| Bisected | 3 |
| Propagated | 0 |

## Clusters (still broken on PIN)

| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 4 | pass/run regression | returned projection source permission is unavailable… | `pass/borrows/returned_slice_record_reference_custody_exit` | `f2c75870ebaf890328b787fa6d8ed50e868edbb8` | 1672 | #1672 tightens returned-view borrowing: projected call results need completed invocation access and captured borrowing parents | bisected |
| 1 | pass/run regression | anonymous address value cannot prove fit in `addr::Bound`… | `pass/build/explicit_provider_storage_extent_exit` | `55f4a84e523c0f1b64f7d72d80f4cb83e84e7017` | 1074 | #1074 refactors normal-return/call-binding establishment and domain premises, breaking anonymous-address bound proofs for provider suffix | bisected |
| 1 | pass/run regression | terminating machine walk reaches recursive component `iterate`… | `pass/termination/runtime_named_state_remainder_endpoint_exit` | `0a85200733ef1f32849c682936c53c22fdff3226` | 1825 | #1825 enforces reach/termination ranking on nested components so the named-state remainder walk rejects `iterate` without `terminates by` | bisected |

Good commits (fixed ledger): c1 `576c5c9433a95c1f4dcb4f18197f408afbfe92cd`; c2 `bed3b818b169b1ff86fe30b5ec27fda873ffed5f`; c3 `6f4ad2d1e228899c7cd4a54b80a6f591764c0794`. All breaking commits are strict ancestors of `741270df60f7` (not PIN, not ledger-only).

### Report index 22 → cluster c1 (8 rows)

- `pass/borrows/returned_slice_record_reference_custody_exit` still broken
- `pass/borrows/runtime_returned_stored_view_reborrow_exit` still broken
- `pass/slices/runtime_mutable_slice_result_whole_forward_exit` still broken
- `pass/wire/wire_preserving_decode_relay_exit` still broken
- `fail/borrows/returned_stored_view_reborrow_parent_write_rejected` fixed on main
- `fail/slices/returned_inclusive_window_extent_mismatch` fixed on main
- `fail/slices/window_extent_does_not_establish_bounds` fixed on main
- `fail/wire/returned_preserving_remainder_input_write` fixed on main

### Report index 62 → cluster c2 (3 rows)

- `pass/build/explicit_provider_storage_extent_exit` still broken
- `fail/build/explicit_provider_scalar_budget` fixed on main
- `fail/build/explicit_provider_storage_extent` fixed on main

### Report indices 102, 142, 182 → fixed on PIN (no cluster)

- `fail/arithmetic/exact_overflow_value_call_hint` fixed on main
- `pass/borrow/owned_capture_replaced_reference` fixed on main
- `pass/facts/target_family_callback_guarantee/field` fixed on main

### Report index 222 → cluster c3 (1 row)

- `pass/termination/runtime_named_state_remainder_endpoint_exit` still broken

## Fixed on main (PIN)

Listed in `part-22.tsv` as `fixed-on-main`.

## Commands / host

- Host: linux x86_64
- Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `mbx build -p omega` + `corpus_runner` per bisect step
- Re-measure: `OMEGA_CORPUS_FIXTURE_FILTER` + `OMEGA_CORPUS_RECORDS` (15 fixture slice)
- Bisect: `.agents/ledger-bisect-2026-11-11/tools/run_cluster_bisect.sh <witness> <id>`
