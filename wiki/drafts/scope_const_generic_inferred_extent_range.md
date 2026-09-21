# CONST-GENERIC-INFERRED-EXTENT-RANGE — scope verification

Scoped record for the planner item NEW-SV-CONST-GENERIC-INFERRED-EXTENT-RANGE-SCOPE,
written at `8f58b6676b00` (linux x86-64). Board stub at TASKS.md (~:9498) is a
wave restub of the settled const-generic extent surface.

## What the name covers

`[T; N]` collections whose extent is a symbolic `ConstParameter`, and the
inferred binder-bounded index/range types (`u64[0..N]`, `u64[0..=N]`) used to
discharge index and range proofs against that extent — the same surface
CONST-GENERIC-EXTENT-RANGE-DISCHARGE already owns.

## Landed state

`67d68511bf14` ("check: discharge index/range proofs against const-generic
extents") lands the full lane:

- `symbolic_extent_floor` (`checks/ranges/types.rs:315`) resolves the binder's
  declared minimum through machine and attached-data const/value parameters.
- `declared_bound_names_symbolic_extent` (`types.rs:430`) recognizes
  `u64[0..N]` / `u64[0..=N]` binder-bounded types on the index or range end.
- `checks/ranges/proofs.rs` carries the discharge family:
  `symbolic_extent_index_is_proven` (:178), `symbolic_extent_range_bound_is_proven`
  (:219), `symbolic_extent_range_end_is_proven` (:258), `symbolic_extent_is_named`
  (:153), `symbolic_extent_range_is_proven`.
- Wired into `indexes/validation.rs` at :139/:146/:478/:528;
  `fixed_array_type_symbolic_extent` (`arrays.rs:47`) returns the binder the
  obligation discharges against.

## Witnesses (this wave)

- `typed-trees-to-checked-trees` pins `const_generic_extent_index_discharge`
  (validation/tests.rs:792) and `const_generic_extent_range_discharge` (:854) —
  re-verified 2/2 at `a84ebca9720c` per
  [const_generic_extent_range_discharge.md](const_generic_extent_range_discharge.md).
- The inferred-extent behavioral pin
  `inline_const_generic_selectors_execute_distinct_inferred_extents`
  (checked-interpreter `borrowed_subslices`) — distinct inferred `N` extents
  execute distinctly — is closed green on linux x86-64 at `7b224763615`
  (the earlier macOS arm64 checker-gap diagnosis is recorded and closed in
  `known_baseline_failures.md`).

## Verdict

Scope verified — the inferred-extent-range surface is landed and witnessed;
no independent slice exists under this name. The board restub folds into
CONST-GENERIC-EXTENT-RANGE-DISCHARGE's resolved record.
