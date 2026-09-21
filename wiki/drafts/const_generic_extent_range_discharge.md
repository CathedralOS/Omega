# CONST-GENERIC-EXTENT-RANGE-DISCHARGE — re-verification ledger

Re-verified on Linux x86-64 at `a84ebca9720c` (board tip at claim time) by
Zergling-126 under claim ticket `be360623`. The board stub says "verify scope
then implement" — the named surface is already landed: commit `67d68511bf14`
("check: discharge index/range proofs against const-generic extents").

## What landed

`[T; N]` collections with a symbolic `ConstParameter` extent previously
skipped bounds checking entirely (`fixed_array_type_length` → None → not a
slice → Unsupported, silently Ok); the checked template claimed validity for
admissible `N` it did not have. Now `check_indexed_access` recognizes
`FixedArrayLength::ConstParameter` and runs a symbolic-extent lane mirroring
the unknown-length slice lane:

- `symbolic_extent_floor` (`checks/ranges/types.rs:315`) resolves the
  binder's declared minimum through machine and attached-data const/value
  parameters.
- `declared_bound_names_symbolic_extent` (`types.rs:430`) recognizes
  `u64[0..N]` / `u64[0..=N]` binder-bounded types on the index or range end.
- `checks/ranges/proofs.rs` — the discharge family:
  `symbolic_extent_index_is_proven` (:178, strict `index < N`),
  `symbolic_extent_range_bound_is_proven` (:219, `bound <= N`),
  `symbolic_extent_range_end_is_proven` (:258),
  `symbolic_extent_is_named` (:153), `symbolic_extent_range_is_proven`.
- Wired into `indexes/validation.rs` at :139/:146/:478/:528.
- `fixed_array_type_symbolic_extent` (`arrays.rs:47`) returns the binder the
  index obligation is discharged against.

## Re-run on this host

```text
$ cargo nextest run -p typed-trees-to-checked-trees --no-fail-fast -E 'test(~const_generic_extent)'
PASS const_generic_extent_index_discharge
PASS const_generic_extent_range_discharge
2 tests run: 2 passed
```

## Verdict

The discharge leg the name scopes is landed at `67d68511bf14` and re-verified
green on Linux x86-64 — no unclaimed slice remains inside the surface.
