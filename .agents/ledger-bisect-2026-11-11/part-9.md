# Ledger bisect part 9 (k=9 of 40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`. Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`. Regression queue slice: prior report cluster indices `9, 49, 89, 129, 169, 209` (226 clusters sorted by row count desc, tie-break diagnostic `LC_ALL=C`).

## PIN re-measurement summary (all 24 rows)

| Class | Count |
| --- | ---: |
| fixed on PIN (meets ledger expectation) | 16 |
| still broken (per frozen `pin_observed.txt`) | 8 |
| **total slice rows** | **24** |

Member recovery for report cluster index **9** (16 rows): from PR [#2359](https://github.com/cathedralos/omega/pull/2359) report `wiki/drafts/audits/ledger_regressions_2026-11-11.md` (measured `741270df60f7eab218e879eb6771d2bc75e64c00`), rank-9 cluster witness `pass/facts/computed_record_window_through_target_body_exit`, shown members plus `(+13 more)`. Full set = nine `may_write_computed_receiver_through_*_exit` plus seven `computed_record_window_through_*_exit` (four not in `regressed_fixtures.txt` at 741270df: `call_result`, `const_name`, `field`, `local`).

## Still-broken clusters at PIN (bisected where possible)

| Rows | Kind | Diagnostic (PIN obs.) | Witness | First bad | PR | Good anchor | Mechanism |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 4 | pass/run rejected | `Float::equal` terminal boundary selection | `pass/facts/computed_record_window_through_call_result_exit` | *(none — see below)* | — | `773b8422694b5a8e64b43284bcba1ceb6de2b1e9` | not established |
| 3 | incomplete | `` `main` calls `forward`; `forward` has no executable body `` | `pass/borrows/runtime_returned_nonzero_nested_window_exit` | `2476d1be2203009f1b32b8164bb6224c562c3bd5` | 1604 | `c15929193d3993f264c54338440f4e1405dff0f6` | see below |
| 1 | rejected | `Lowering(ConflictingStructuralValueHome` | `pass/ownership/runtime_match_nested_observation_cleanup_exit` | `22f215eb3e7d9814cb3e29f42856e56bb57e60ed` | 2078 | `5fe672430be9a1c5560d74bb5102f4f70ddfba68` | see below |

### Cluster Float-boundary (index 9 partial — four rows)

Frozen PIN observation (ledger `checked`, runtime `rejected`, `Float::equal` boundary). `find_nearest_good` + frozen-witness test at PIN: **exit 0** under commit-scoped `CARGO_TARGET_DIR` (reproduced on introduction worktree and PIN worktree). `git bisect run` **not executed** (`PIN_NOT_BAD` in `work/bisect_float_boundary.log.find_good`). No first-bad commit under matched-build methodology; **not** `never-good since 773b8422694b5a8e64b43284bcba1ceb6de2b1e9` (matched builds meet ledger from fixture introduction through PIN).

Members:

- `pass/facts/computed_record_window_through_call_result_exit`
- `pass/facts/computed_record_window_through_const_name_exit`
- `pass/facts/computed_record_window_through_field_exit`
- `pass/facts/computed_record_window_through_local_exit`

### Cluster incomplete-lowering (index 49)

- `pass/borrows/runtime_returned_nonzero_nested_window_exit`
- `pass/facts/matched_field_windows_through_other_state_exit`
- `pass/slices/runtime_returned_nested_array_window_home_exit`

### Cluster ConflictingStructuralValueHome (index 209)

- `pass/ownership/runtime_match_nested_observation_cleanup_exit`

## Mechanism (first-bad commits)

**Float-boundary cluster:** mechanism not established (no bisected first-bad).

**#1604 (`2476d1be2203009f1b32b8164bb6224c562c3bd5`) — incomplete lowering**

`omega-rust/psi/compilation/validation/src/machine_calls/reference_result_custody/captured_returns/invocation_returns.rs`, `impl Captures`:

```rust
let relative = self.target_sources(call.target_symbol, active)?;
// ...
return self.completed_descriptor_sources(
    state, operand, &actual_path, expected, &relative, current, &mut Vec::new(),
);
```

**#2078 (`22f215eb3e7d9814cb3e29f42856e56bb57e60ed`) — `ConflictingStructuralValueHome`**

`omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/expression_bindings.rs`, `retain_completed_structural_homes`:

```rust
if existing.argument != home.argument || existing.structural_type != home.structural_type {
    return Err(LoweringError::ConflictingStructuralValueHome { root: home.root, ... });
}
```

## Witness stability (3× `bisect_test.py` exit codes)

Full log: `work/stability_matrix.txt`. (`0` = meets ledger / good; `1` = reproduces frozen PIN regression class.)

| Cluster | Commit (40-char) | Scope | run1 | run2 | run3 |
| --- | --- | --- | ---: | ---: | ---: |
| Float good anchor | `773b8422694b5a8e64b43284bcba1ceb6de2b1e9` | all 4 `computed_record_window` rows | 0 | 0 | 0 each |
| Float PIN tip | `51e8c45c793007f211d0d9baee7de16767172444` | all 4 rows (matched build) | 0 | 0 | 0 each |
| Incomplete parent | `c15929193d3993f264c54338440f4e1405dff0f6` | 3 members | 0 | 0 | 0 each |
| Incomplete bad | `2476d1be2203009f1b32b8164bb6224c562c3bd5` | 3 members | 1 | 1 | 1 each |
| Conflicting parent | `d25889014d5e1a5f3770dacd77612125f046e372` | witness | 0 | 0 | 0 |
| Conflicting bad | `22f215eb3e7d9814cb3e29f42856e56bb57e60ed` | witness | 1 | 1 | 1 |

## Rows fixed on PIN (16)

| Report index | Rows | Witness |
| ---: | ---: | --- |
| 9 (partial) | 12 | `pass/facts/computed_record_window_through_target_body_exit` |
| 89 | 2 | `fail/facts/data_where_standing_mutable_recast_qualified_constant_rejected` |
| 129 | 1 | `fail/ownership/selected_storage_join_unrelated_root` |
| 169 | 1 | `fail/generics/record_const_foreign_nominal_carrier` |

## Bisect methodology

Frozen ledger at `/tmp/ledger_bisect_corpus_outcomes.txt` and per-fixture `pin_observed.txt` witnesses. `bisect_witness.sh` uses commit-scoped `target/bisect-pool-<12-char-sha>/` (fixes shared-pool skew). Skip ledger-only commits (exit 125).

Branch head: `work/ledger-bisect-part-9` at `312c13dc3729e53fb9037c00b58d992c95c2e3bd` (docs only after PIN).
