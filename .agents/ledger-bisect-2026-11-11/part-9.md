# Ledger bisect part 9 (k=9 of 40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`. Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`. Regression queue slice: prior report cluster indices `9, 49, 89, 129, 169, 209` (226 clusters sorted by row count desc, tie-break diagnostic `LC_ALL=C`).

## PIN re-measurement summary (all 24 rows)

Authoritative observation: `work/pin_observed.txt`, produced by `tools/remeasure_pin_slice.sh` at the PIN worktree with commit-scoped `CARGO_TARGET_DIR=target/bisect-pin-measure-51e8c45c7930/` and the `corpus_runner` binary from that same build. Each fixture was run **3×**; outcome **status** was stable on every row (timing ms varied only). Classification vs frozen ledger: `work/pin_classify.json`.

| Class | Count |
| --- | ---: |
| **fixed on PIN** (meets ledger expectation) | **20** |
| **still broken** | **4** |
| **total slice rows** | **24** |

**Supersedes stale capture:** An earlier `pin_observed.txt` listed four `computed_record_window_through_*` rows as `rejected` with `Float::equal` boundary text. That reading came from a **shared** `target/bisect-pool` paired with the PIN checkout (compiler/artifact skew), not from a matched build. It is not used for bisect or slice accounting.

Member recovery for report cluster index **9** (16 rows): from PR [#2359](https://github.com/cathedralos/omega/pull/2359) report `wiki/drafts/audits/ledger_regressions_2026-11-11.md` (measured `741270df60f7eab218e879eb6771d2bc75e64c00`), rank-9 cluster witness `pass/facts/computed_record_window_through_target_body_exit`, shown members plus `(+13 more)`. Full set = nine `may_write_computed_receiver_through_*_exit` plus seven `computed_record_window_through_*_exit` (four not in `regressed_fixtures.txt` at 741270df: `call_result`, `const_name`, `field`, `local`). **All 16 meet the ledger on PIN** under matched-build measurement (including the four transports previously misread as broken).

## Still-broken clusters at PIN (bisected)

| Rows | Kind | Diagnostic (PIN obs.) | Witness | First bad | PR | Good anchor | Mechanism |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 3 | incomplete | `` `main` calls `forward`; `forward` has no executable body `` | `pass/borrows/runtime_returned_nonzero_nested_window_exit` | `2476d1be2203009f1b32b8164bb6224c562c3bd5` | 1604 | `c15929193d3993f264c54338440f4e1405dff0f6` | see below |
| 1 | rejected | `Lowering(ConflictingStructuralValueHome` | `pass/ownership/runtime_match_nested_observation_cleanup_exit` | `22f215eb3e7d9814cb3e29f42856e56bb57e60ed` | 2078 | `5fe672430be9a1c5560d74bb5102f4f70ddfba68` | see below |

### Cluster incomplete-lowering (index 49)

- `pass/borrows/runtime_returned_nonzero_nested_window_exit`
- `pass/facts/matched_field_windows_through_other_state_exit`
- `pass/slices/runtime_returned_nested_array_window_home_exit`

### Cluster ConflictingStructuralValueHome (index 209)

- `pass/ownership/runtime_match_nested_observation_cleanup_exit`

## Mechanism (first-bad commits)

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

## Witness stability at PIN (3× corpus status per row)

Full log: `work/pin_remeasure_stability.txt` (`pin-remeasure` lines = outcome status on each run; all 24 rows stable). Historical bisect stability at first-bad/parent remains in `work/stability_matrix.txt`.

| Cluster | Commit (40-char) | Scope | run1 | run2 | run3 |
| --- | --- | --- | ---: | ---: | ---: |
| Incomplete parent | `c15929193d3993f264c54338440f4e1405dff0f6` | 3 members | 0 | 0 | 0 each |
| Incomplete bad | `2476d1be2203009f1b32b8164bb6224c562c3bd5` | 3 members | 1 | 1 | 1 each |
| Conflicting parent | `d25889014d5e1a5f3770dacd77612125f046e372` | witness | 0 | 0 | 0 |
| Conflicting bad | `22f215eb3e7d9814cb3e29f42856e56bb57e60ed` | witness | 1 | 1 | 1 |

## Rows fixed on PIN (20)

| Report index | Rows | Witness (representative) |
| ---: | ---: | --- |
| 9 | 16 | `pass/facts/computed_record_window_through_target_body_exit` |
| 89 | 2 | `fail/facts/data_where_standing_mutable_recast_qualified_constant_rejected` |
| 129 | 1 | `fail/ownership/selected_storage_join_unrelated_root` |
| 169 | 1 | `fail/generics/record_const_foreign_nominal_carrier` |

## Bisect methodology

Frozen ledger at `/tmp/ledger_bisect_corpus_outcomes.txt` (from PIN `corpus_outcomes.txt` at measurement time) and per-fixture `work/pin_observed.txt`. `bisect_witness.sh` uses commit-scoped `target/bisect-pool-<12-char-sha>/`. Skip ledger-only commits (exit 125). No bisect for index-9 float skew (artifact mismatch only).

Branch head: `work/ledger-bisect-part-9` (see `git rev-parse HEAD` on that branch).
