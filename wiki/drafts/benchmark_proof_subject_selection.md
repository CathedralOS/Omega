# BENCHMARK-PROOF-SUBJECT-SELECTION — re-verification ledger

Re-verified on Linux x86-64 at `39317a770b1f` (board tip at claim time) by
Zergling-126 under claim ticket `4a74a3e6`. The item re-mines the
proof-subject leg of the benchmarks frontier ("no measurable subject").

## The named remaining leg has landed

- Record committed: `tools/benchmark/records/structural_proofs__linux_x86_64__default.json`
  (landed `d430cdbde5d5` "benchmarks: commit structural_proofs linux_x86_64
  record and matrix row", recorded_utc 2026-09-21T02:46Z) — 3 compile
  samples, median 34932.9 ms, compile_max_rss 150704128 B, 8192-byte
  published artifact, runtime `skipped (--no-run was passed)`.
- Matrix entry committed: `wiki/drafts/benchmarks.md:55` carries the
  structural_proofs × linux_x86_64 × default row; :182-184 documents the
  dependency-free proof subject.
- `samples/cli/proofs/structural_proofs/build.omg` present (authored
  `build.omg` + `ProgramEntry` bindings; `EXPLICIT_ENTRY_PROOF_SAMPLES`
  pins it).

## Re-verified on this host

```text
$ python3 tools/benchmark/benchmark.py validate tools/benchmark/records/structural_proofs__linux_x86_64__default.json
(exit 0 — record conforms)
$ python3 tools/benchmark/benchmark.py measure --root samples/cli/proofs/structural_proofs/main.omg --target linux_x86_64 --no-run --print
... emits a conforming record (runtime_ms skipped — --no-run; 3 compile samples)
```

## Verdict

Settled: the proof-subject selection leg (structural_proofs record + matrix
row) is landed at `d430cdbde5d5` and re-verified on this revision. The only
named sibling leg — `math_proofs`'s checked-call-selection fix — belongs to
PROOF-SAMPLES-CHECKED-CALL-SELECTION territory, not this surface.
