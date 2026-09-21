# DEPENDENCY-FREE-RUNTIME-BENCHMARK-SUBJECT — z142 verify+record

Board row: TASKS.md:11198 — resolved; gate recorded at `867443a8fd`,
corrected by z177 at `e7c0099cb2`. Re-verified at `32a6a7fa33`.

## Mined-source row

The runtime leg needs a depend-free subject whose process completes cleanly.
The row originally concluded depend-free subjects stay `--no-run` because
process completion routes through the std-bound `ProcessExit` provider and a
package-local boundary machine produces no provider plan. z177 corrected
that: `wrapping_square_sum` is depend-free and produced a measured runtime
row — its entry evidently does not need `ProcessExit` explicitly.

## State at 32a6a7fa33 — correction stands

- `samples/cli/arithmetic/wrapping_square_sum` is genuinely depend-free:
  `build.omg` binds four `ProgramEntry` roots and carries zero `depend()`
  edges (header comment states the intent).
- Committed record `tools/benchmark/records/
  wrapping_square_sum__linux_x86_64__default.json` (source e7c0099cb2):
  `runtime_ms.status = "measured"` — 5 samples, all exit code 0, median
  4.37554 ms, min 3.208 ms — plus `compile_time_ms` measured (median
  33,396.8 ms).
- The provider-authority analysis may still describe other depend-free
  subjects; this subject's runtime leg is measured, not skipped.

## Verdict

Resolved with the z177 correction — no slice under this name. Sibling stubs
on the same surface: DEPENDENCY-FREE-BENCHMARK-SUBJECT,
DEPENDENCY-FREE-MEASURABLE-SUBJECT (verified `fff3918dc42`),
BENCHMARK-DEPEND-FREE-RUNNABLE-SUBJECT,
BENCHMARK-MEASURABLE-SUBJECT-CORPUS, BENCHMARK-STANDALONE-SUBJECT. Further
measured rows = BENCHMARK-ROW-RESUMPTION / BENCHMARK-CROSS-HOST-ROWS.
Record only; no code change.
