# Verified sorted insert

Binary-search insertion into a fixed-capacity sorted table, with the proof
surface carried in contracts. `sorted` is a pure predicate machine over the
sixteen-cell array (`check_sorted` descends a `remaining` rank);
`seed` discharges sortedness statically through `ensures`, and `search`
requires it. `search` returns the upper-bound insertion point over
`values[lo..hi)`, carrying `lo <= result <= hi` as an `ensures` and the
interval `width == hi - lo` as its `Nat::Descending` termination measure —
the compiler proof that a ranked machine may cite a predicate-machine call in
its `requires`. `insert` shifts the suffix right under a `cursor` rank, then
places the key; sortedness is re-attested at runtime by a `sorted` guard
before the good exit.

The driver seeds `{1, 3, 5, 7}` (cells 4..15 pinned at `i32::MAX`), searches
for `5/2/20/1` (positions `3, 1, 4, 1`), inserts `20` then `2` (grown counts
`5, 6`), and attests the final prefix `{1, 2, 3, 5, 7, 20}`.

Oracle: exit `58 = (3 + 1 + 4 + 1) + (5 + 6) + (1 + 2 + 3 + 5 + 7 + 20)`
on success; `crash Trap` on any contract or attestation failure. The
`invalid/` control calls `search` without establishing `sorted` and must be
rejected with `cannot prove requires contract`.
