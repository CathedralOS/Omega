# BENCHMARK-COMPILE-ONLY-ROWS — landed-state record

Claimed slice: `wiki/drafts/benchmark_compile_only_rows.md` (record), verified at
base `71fb20485e78` on linux x86-64.

## Landed state (on main)

The three cross-target compile-only rows for the dependency-free subject
`samples/cli/arithmetic/wrapping_square_sum` are committed under
`tools/benchmark/records/` (landed at `52ceeeabb7`, per the board row's own
"Remaining: none" resolution):

| target | file | compile median | code size | runtime |
|---|---|---|---|---|
| windows_x86_64 | `wrapping_square_sum__windows_x86_64__default.json` | 24,118 ms | 1,024 B | skipped |
| macos_arm64 | `wrapping_square_sum__macos_arm64__default.json` | 24,454 ms | 16,640 B | skipped |
| linux_arm64 | `wrapping_square_sum__linux_arm64__default.json` | 28,130 ms | 8,192 B | skipped |

All three measured on a linux x86-64 host (recorded 2026-09-20T19:44-48Z),
`runtime_ms.status == "skipped"`, and each record carries the
`stage_timings_last_sample_ms` breakdown (prepare / compile / publish / total).
Code sizes match the w9 measurements cited on the row.

## Row adjudication confirmed

- The e48558bd41 integer-comparison-occurrence rejection that gated these
  compiles is resolved upstream (`76dc49a99e` — selected-vs-builtin custody
  split); the records postdate it and each shows a successful publish leg.
- `macos_x86_64` and `uefi_x86_64` remain non-applicable CLI-subject targets
  (review settlement has no bound `<target>::ProgramEntry` root slot; the
  x86_64 macOS host-profile gap is owned by MACOS-X64-HOST-PROFILE).
- `benchmarks.md` update leg landed at `d8825aef56`; host-row matrix repair at
  `7e38fc2763`. Further matrix refresh belongs to BENCHMARK-HOST-ROW-MATRIX.

## Residual

None under this name. No additional records were produced this pass: per-target
settlement (`benchmark.py prepare --target <t>`) needs one-time package-review
admission writes under `samples/cli/arithmetic/wrapping_square_sum` + the
records dir, and the item is already satisfied — re-measuring existing rows is
not a deliverable.
