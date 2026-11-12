# Ledger bisect part 9 (k=9 of 40)

Measured commit: `51e8c45c793007f211d0d9baee7de16767172444` (PIN). Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`. Regression queue slice: prior report cluster indices `9, 49, 89, 129, 169, 209` (226 clusters sorted by row count desc, tie-break diagnostic `LC_ALL=C`).

## PIN re-measurement summary (all 24 rows)

| Class | Count |
| --- | ---: |
| fixed on PIN (meets ledger expectation) | 16 |
| still broken on PIN | 8 |
| **total slice rows** | **24** |

Member recovery for report cluster index **9** (16 rows): from PR [#2359](https://github.com/cathedralos/omega/pull/2359) report `wiki/drafts/audits/ledger_regressions_2026-11-11.md` (measured `741270df60f7`), rank-9 cluster witness `pass/facts/computed_record_window_through_target_body_exit`, shown members plus `(+13 more)`. Full set = all nine `may_write_computed_receiver_through_*_exit` sweep rows plus seven `computed_record_window_through_*_exit` transports (the four not listed in `regressed_fixtures.txt` at 741270df: `call_result`, `const_name`, `field`, `local`). None of the 24 rows are unrecoverable.

## Still-broken clusters at PIN (bisected)

| Rows | Kind | Diagnostic (PIN) | Witness | First bad | PR | Good parent | Mechanism |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 3 | incomplete | `` `main` calls `forward`; `forward` has no executable body `` (`returned reference sources`) | `pass/borrows/runtime_returned_nonzero_nested_window_exit` | `2476d1be2203009f1b32b8164bb6224c562c3bd5` | 1604 | `c15929193d3993f264c54338440f4e1405dff0f6` | see below |
| 1 | rejected | `Lowering(ConflictingStructuralValueHome` | `pass/ownership/runtime_match_nested_observation_cleanup_exit` | `22f215eb3e7d9814cb3e29f42856e56bb57e60ed` | 2078 | `5fe672430be9a1c5560d74bb5102f4f70ddfba68` | see below |

### Cluster 1 members (report index 49)

- `pass/borrows/runtime_returned_nonzero_nested_window_exit`
- `pass/facts/matched_field_windows_through_other_state_exit`
- `pass/slices/runtime_returned_nested_array_window_home_exit`

### Cluster 2 members (report index 209)

- `pass/ownership/runtime_match_nested_observation_cleanup_exit`

### Report cluster index 9 — partial still broken at PIN (4 of 16)

Ledger expects `checked`; PIN rejects on terminal `Float::equal` boundary selection (distinct from the report’s `leading`/`entry` borrow diagnostic at 741270df).

- `pass/facts/computed_record_window_through_call_result_exit`
- `pass/facts/computed_record_window_through_const_name_exit`
- `pass/facts/computed_record_window_through_field_exit`
- `pass/facts/computed_record_window_through_local_exit`

## Mechanism (first-bad commits)

**#1604 (`2476d1be2203009f1b32b8164bb6224c562c3bd5`) — incomplete lowering / returned reference sources**

`omega-rust/psi/compilation/validation/src/machine_calls/reference_result_custody/captured_returns/invocation_returns.rs`, `impl Captures` — `invocation_return_origins` / `completed_descriptor_sources`: replaces lifetime-frame `reference_result_transports` with `target_sources(call.target_symbol, …)` and re-reads stored reference slots at the invocation’s storage cut (outer write-only pointer vs inner record slot), so completion edges require fuller captured-origin graphs before state-graph lowering reaches terminator `returned reference sources`.

```rust
// invocation_returns.rs (2476d1be) — excerpt
let relative = self.target_sources(call.target_symbol, active)?;
// ...
let carrier = super::super::input_carrier_type(self.program, reference).unwrap_or(reference);
// ... read inner slot at current storage cut ...
return self.completed_descriptor_sources(
    state, operand, &actual_path, expected, &relative, current, &mut Vec::new(),
);
```

**#2078 (`22f215eb3e7d9814cb3e29f42856e56bb57e60ed`) — `ConflictingStructuralValueHome`**

`omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/expression_bindings.rs`, `retain_completed_structural_homes`: when match/case-value evaluation registers a second owned `StructuralLocalBinding` at the same expression root, lowering fails closed with `LoweringError::ConflictingStructuralValueHome` (producer cites `unit_structural_values.rs` / `expression_bindings.rs` in the witness diagnostic).

```rust
// expression_bindings.rs — retain_completed_structural_homes (post-#2078)
if existing.argument != home.argument || existing.structural_type != home.structural_type {
    return Err(LoweringError::ConflictingStructuralValueHome { root: home.root, ... });
}
```

## Witness stability (3× `bisect_test.py` exit codes)

Detached worktrees at first-parent good vs first bad (`0` = meets ledger, `1` = reproduces PIN regression class). Full log: `.agents/ledger-bisect-2026-11-11/work/stability_matrix.txt`.

| Cluster | Commit | Witness / members | run1 | run2 | run3 |
| --- | --- | --- | ---: | ---: | ---: |
| 1 parent | `c15929193d39` | `runtime_returned_nonzero_nested_window_exit` | 0 | 0 | 0 |
| 1 parent | `c15929193d39` | `matched_field_windows_through_other_state_exit` | 0 | 0 | 0 |
| 1 parent | `c15929193d39` | `runtime_returned_nested_array_window_home_exit` | 0 | 0 | 0 |
| 1 bad | `2476d1be2203` | all three members above | 1 | 1 | 1 |
| 2 parent | `d25889014d5e` | `runtime_match_nested_observation_cleanup_exit` | 0 | 0 | 0 |
| 2 bad | `22f215eb3e7` | same witness | 1 | 1 | 1 |

## Rows fixed on PIN (remaining 16)

| Report index | Rows | Witness | PIN class |
| ---: | ---: | --- | --- |
| 9 | 12 | `pass/facts/computed_record_window_through_target_body_exit` | `checked` / `incomplete` meeting ledger |
| 89 | 2 | `fail/facts/data_where_standing_mutable_recast_qualified_constant_rejected` | `rejected` unexpected |
| 129 | 1 | `fail/ownership/selected_storage_join_unrelated_root` | `rejected` unexpected |
| 169 | 1 | `fail/generics/record_const_foreign_nominal_carrier` | `rejected` unexpected |

Fixed members (index 9 — twelve transports):

- `pass/facts/computed_record_window_through_other_machine_exit`, `…_other_state_exit`, `…_target_body_exit`
- `pass/facts/may_write_computed_receiver_through_{call_result,const_name,field,local,match_join,other_machine,other_state,parameter,target_body}_exit`

Plus index 89/129/169 members listed in `part-9.tsv`.

## Bisect methodology

Fixed copies at `/tmp/ledger_bisect_corpus_outcomes.txt` and per-fixture `/tmp/ledger_bisect_witness_pin.json` from PIN observations in `.agents/ledger-bisect-2026-11-11/work/pin_observed.txt`. `bisect_test.py` compares runtime outcome/diagnostic to PIN (not per-revision ledger). Good anchors: cluster 1 `c15929193d3993f264c54338440f4e1405dff0f6`; cluster 2 `5fe672430be9a1c5560d74bb5102f4f70ddfba68` (full SHA from `git rev-parse`).

Branch head (this session): `work/ledger-bisect-part-9`.
