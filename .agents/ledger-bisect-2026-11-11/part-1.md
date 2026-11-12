# Ledger bisect part 1 (k=1, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`

Bisect uses fixed `/tmp/ledger-bisect-pin/corpus_outcomes.txt` and per-witness PIN observation JSON; each step uses fresh `/tmp/omega-target-bisect-<sha>`. Ledger-only commits exit 125.

## Cross-part conflict (`borrowed_slot_swap` witness)

Witness: `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap`. Three PIN-fixed runs per commit (`conflict_borrowed_slot_swap_3x4.log`).

| commit (full SHA) | role | run RCs | status | first diagnostic |
| --- | --- | --- | --- | --- |
| `f2c75870ebaf890328b787fa6d8ed50e868edbb8` | `4faf6413^` (#1672) | 125,125,125 | rejected | statement 12 mutates `second_buffer.value` while local borrow `second` is still active… |
| `4faf6413467dd7a7f6cf7e62ba77d1c607591f18` | part 32 first-bad (#1718) | 125,125,125 | rejected | same mutation diagnostic |
| `2ab64136c02d7879635a92e3e078d7374bec7590` | `0aeed111^` (#1811) | 1,1,1 | rejected | machine `swap_pending` stores a borrowed value without a complete normal-exit backing relation |
| `0aeed1114ae3d056f78113e377f10b7126b1e75e` | prior part-1 claim (#1896) | 1,1,1 | rejected | same `swap_pending` backing diagnostic |

**Verdict:** Part 32’s `4faf6413` is the first-parent edge where the witness **fails the ledger** with the **mutation-while-borrow** class (rc=125 under the PIN harness — not the ledger’s pinned expectation class). `#1896` is **not** the first good→bad edge for the **current** PIN diagnostic: `0aeed111^` already shows rc=1 with the `swap_pending` class. The row is **good → bad (mutation class) → stays worse with PIN-class diagnostic**; **latest** rc=1 `swap_pending` first-parent introduction in the probed window is `1bb9690b80851e6b149c9a414b3317439183a882` (#1819) with witness (1,1,1) at that commit but **first parent `96ecfb7c12877f0a60fe20c62334f4a1fd973d4a` is not (0,0,0)** (rc=125, different diagnostic). Fixture introduction good remains `3bbd06849b998f4fef4ccdffb8ef1f6059c07def` (rc=0).

## Prior-report slice reconcile (`% 40 == 1`)

Measured slice totals (name-sweep expanded idx 1): **83 / 75 / 8**.

| prior idx | members (measured) | fixed @PIN | broken @PIN |
| ---: | ---: | ---: | ---: |
| 1 | 74 | 71 | 3 |
| 41 | 4 | 0 | 4 |
| 81 | 2 | 2 | 0 |
| 121 | 1 | 1 | 0 |
| 161 | 1 | 0 | 1 |
| 201 | 1 | 1 | 0 |
| **total** | **83** | **75** | **8** |

Prior-report idx **1** table rows **70**; idx **1** name-sweep measures **74** in `regressed_fixtures.txt`. **`persistent_retention`** is **not** its own prior-report index — it is in the idx-1 `foreign_retained_*` sweep and is **PIN cluster 3** (mutation diagnostic), distinct from PIN cluster 1 (`swap_pending` / `persistent_restore`).

### Idx-1 sweep extras (outside report-70 totals)

| fixture | @PIN |
| --- | --- |
| `pass/facts/foreign_nested_record_backing_repeated_child_type` | fixed |
| `pass/facts/foreign_retained_mutable_borrow_mixed_issuers` | fixed |
| `pass/facts/foreign_retained_mutable_borrow_reverse_completion` | fixed |
| `pass/facts/foreign_retained_shared_borrow_enclosing_allocation` | fixed |

## PIN clusters (still broken @ PIN)

Witness stability (`cluster_witness_stability_v2.log`, `pinclass_edge_verify.log`):

| PIN | witness | first-bad (full SHA) | parent (full SHA) | bad runs | parent runs |
| --- | --- | --- | --- | --- | --- |
| 0 | `pass/slices/runtime_indexed_store_captures_selector_before_rhs_exit` | `2e3a14463a8255c4e25ef4e3a60edc2d4c3ba3a9` | `2be2bd0442046813713c68acbdf6da8b35b8b85e` | pending | pending |
| 1 | `pass/facts/foreign_retained_shared_borrow_borrowed_slot_swap` | `1bb9690b80851e6b149c9a414b3317439183a882` | `96ecfb7c12877f0a60fe20c62334f4a1fd973d4a` | 1,1,1 | 125,125,125 |
| 2 | `pass/borrows/runtime_forwarded_reference_array_bounded_read_exit` | `643ff5441c15ea62815109514a8047194d7397a0` | `8e7394363d14aca7dae993adeb86fc348c0d2c7e` | 1,1,1 | 1,1,1 |
| 3 | `pass/facts/foreign_retained_shared_borrow_persistent_retention` | `ade0aabb979920eea0ab13135fa2a86ca116acaa` | `028f2b1a70750c30a23802aece64c370b9bd7ace` | 1,1,1 | 125,125,125 |

Member matrix: `cluster_members_matrix.log` (in progress, tmux `part1-members`). Cluster 0 @ bad: all four slice members broken.

Mechanism hunks: `mechanism_hunks.txt` (quoted `git show` for #1007, #1718, #1819, #2397, #2442).
