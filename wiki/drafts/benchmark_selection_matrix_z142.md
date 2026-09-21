# BENCHMARK-SELECTION-MATRIX — verification record (2026-09-20, `71fb20485e`)

Bare mined stub at `TASKS.md:7309` (no `**ITEM.**` marker — claimed
freeform; sibling item claim BENCHMARK-SELECTION-MATRIX held by
c2l-attribution-worker, exp 06:33Z). Named sibling re-mine of the
resolved BENCHMARK-SELECTION-ROW-COVERAGE surface.

Re-verified at `71fb20485e782b11ea107b649957177bc0368663` (linux x86-64):

- The matrix renders **Selection** as its own isolating column:
  `python3 tools/benchmark/benchmark.py matrix` shows
  `wrapping_square_sum/linux_x86_64` under three selection rows —
  `default`, `sel-885944b13b84`, `sel-9c09e32a82fb` — no selection's
  measurements blur into another's.
- `tools/benchmark/records/` holds six committed rows (cli_mvp linux_x86_64
  default; wrapping_square_sum on linux_arm64/macos_arm64/windows_x86_64
  default + linux_x86_64 under two sel- labels); `selection_label` maps
  the empty selection to `default` and non-empty sets to `sel-<hash>`;
  `test_unsorted_selection_rejected` pins sorted-unique exact rule names.
- `python3 tools/tests/test_benchmark.py` — **21/21 green**.
- Unmeasured host legs stay explicit in the matrix (`macos_x86_64`
  unavailable-pending, `uefi_x86_64` measurable/needs-runtime cells).

Verdict: resolved by the BENCHMARK-SELECTION-ROW-COVERAGE landing —
the selection matrix already exists and is pinned. Coordinator can drop
the stub; residual record production stays with the row-coverage /
row-resumption lanes. Record only; no code change.
