# BENCHMARK-REJECTED-ROW-RECORDING — scope verification (2026-09-20, `00e1da7ae2`)

Bare mined stub at `TASKS.md:7133` (no `**ITEM.**` marker — claimed
freeform). Re-mines the benchmarks doc clause "record them as
non-applicable, not failed compiles"
(`wiki/drafts/benchmarks.md`:127, :136) — a committed record row for a
target whose review settlement rejects the subject
(`no bound required root slot <target>::ProgramEntry`), instead of an
absent or misleading measured row.

## Verified state at `00e1da7ae2` (linux x86-64)

- Record schema (`tools/benchmark/benchmark.py`) admits only per-metric
  `measured`/`unavailable`/`skipped` statuses; there is no row-level
  rejected/non-applicable carrier.
- The matrix keeps uncovered legs explicit without a record:
  `test_unmeasured_host_legs_stay_explicit` pins
  `uefi_x86_64 → "unavailable (needs QEMU or UEFI hardware)"` and
  `cross_platform_cli`/`local_unchecked` as `measurable`-but-unrecorded
  legs — today's surrogate for a rejected row.
- Producing a real non-applicable row needs a schema/test/doc change:
  `tools/benchmark` (record writer + matrix), `tools/tests/
  test_benchmark.py`, `wiki/drafts/benchmarks.md`, and
  `tools/benchmark/records/` (new committed rows).

## Fences (live at verification)

| Surface | Claim | Expiry |
| --- | --- | --- |
| `tools/benchmark`, `cli_mvp`, `euclid_gcd`, `TASKS.md` | BENCHMARK-ROW-RESUMPTION — Devin / z36 | 2026-09-21T03:42Z |
| `tools/benchmark`, `prime_counter`, `benchmarks.md`, `test_benchmark.py`, `TASKS.md` | BENCHMARK-PRIME-COUNTER-ROW — Devin / z178 | 2026-09-21T05:19Z |

Every implementing surface — schema, matrix renderer, doc, records dir —
is live-fenced. No independent unclaimed slice exists this wave; the leg
stays with the benchmark record-production rows above.

Verdict: verified re-mine of an unfilled but owned frontier — coordinator
should fold into the BENCHMARK-ROW-RESUMPTION / record-production
cluster. Record only; no code change.
