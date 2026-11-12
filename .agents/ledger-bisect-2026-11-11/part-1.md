# Ledger bisect part 1 (k=1, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`

Bisect uses fixed `/tmp/ledger-bisect-pin/corpus_outcomes.txt` (from PIN) and per-witness PIN observation JSON; each step uses a fresh `/tmp/omega-target-bisect-<sha>` tree. Ledger-only commits (`tests/omega/corpus_outcomes.txt`, `*.md` only) exit 125.

## Prior-report slice (cluster index % 40 == 1)

| prior idx | rows @741270 | witness | members recovered | fixed @PIN | broken @PIN |
| ---: | ---: | --- | ---: | ---: | ---: |
| 1 | 70 | `pass/facts/foreign_nested_record_backing_call_result` | 74 (`foreign_nested_record_backing_*` + `foreign_retained_*` in sweep list) | 72 | 2 |
| 41 | 4 | `pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit` | 4 (full in report) | 0 | 4 |
| 81 | 2 | `fail/proofs/field_view_index_missing_bound` | 2 | 2 | 0 |
| 121 | 1 | `pass/facts/target_family_callback_guarantee/parameter` | 1 | 1 | 0 |
| 161 | 1 | `pass/borrows/runtime_forwarded_reference_array_bounded_read_exit` | 1 | 0 | 1 |
| 201 | 1 | `pass/constants/direct_named_implementation_initializer_exit` | 1 | 1 | 0 |

**Slice totals:** 83 fixtures measured; 75 fixed on main, 8 still broken (4 PIN diagnostic clusters).

## PIN clusters (still broken, re-grouped by first diagnostic @PIN)

| rows | kind | diagnostic (sample) | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 4 | pass/run rejected | cannot prove requires contract for call middle … `start <= end && end <= bytes.len` | `pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit` | `2e3a14463a8255c4e25ef4e3a60edc2d4c3ba3a9` | 1007 | Retained scoped lifetimes through selected callbacks changed slice indexed-store call `requires` proof | bisected |
| 2 | pass/run rejected | machine `swap_pending` stores a borrowed value without a complete normal-exit backing relation | `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap` | `0aeed1114ae3d056f78113e377f10b7126b1e75e` | 1896 | Placed-field accessor admission in scalar computations regressed foreign retained slot-swap backing | bisected |
| 1 | pass/run rejected | cannot prove requires contract for call require_cell_value … `value >= 0 && value <= 100` | `pass/borrows/runtime_forwarded_reference_array_bounded_read_exit` | `643ff5441c148ce34a0e391b42e7b389de6380e5` | 2397 | Lowered-form invariant-window judging no longer establishes cell bounds on forwarded reference reads | bisected |
| 1 | pass/run rejected | statement 8 mutates `first_buffer.value` while local borrow `first` is still active | `pass/facts/foreign_retained_shared_borrow_persistent_retention` | `ade0aabb979920eea0ab13135fa2a86ca116acaa` | 2442 | Retained custody / `checks_borrows` tightened shared persistent retention borrow end | bisected |

Stability: 3× PIN-end and 3× good-end reruns were deterministic (rc=1 at PIN, rc=0 at good) for clusters 0–1 and 3; cluster 2 witness deterministic at PIN and at `ade0aabb979`.

### Cluster members @PIN

**PIN-0:** `pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit`, `pass/slices/runtime_indexed_store_operand_effect_order_exit`, `pass/slices/runtime_namespace_range_contract_exit`, `pass/slices/runtime_returned_range_extent_contract_exit`

**PIN-1:** `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap`, `pass/facts/foreign_retained_shared_borrow_persistent_restore`

**PIN-2:** `pass/borrows/runtime_forwarded_reference_array_bounded_read_exit`

**PIN-3:** `pass/facts/foreign_retained_shared_borrow_persistent_retention`

### Fixed on main (slice)

Listed in `part-1.tsv` with `class=fixed_on_main` (includes 72/74 prior idx=1 `foreign_*` rows plus idx 81/121/201 singles).

## Commands

```bash
cp tests/omega/corpus_outcomes.txt /tmp/ledger-bisect-pin/
python3 .agents/ledger-bisect-2026-11-11/tools/prepare_pin_snapshots.py <witnesses...>
mbx test -p compiler --test corpus_runner --no-run  # via run_slice_direct.sh / bisect_witness.sh
git bisect start --first-parent <PIN> <good> && git bisect run .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh
# plus first_bad_binary.sh where git bisect stalled on 125-skips
```

Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`, `MBX_SCHEDULER_CPUS=4`.
