# Test-cycle measurement reference

Temporary evidence for choosing the next local test-performance experiment,
especially distinguishing compilation, test execution, and repeated rechecks.
Remove this note when a controlled replacement measurement establishes the
relevant scheduling/selection costs, or when the measured workloads are no
longer used. It is not validation policy or a current checkout baseline; see
[AGENTS.md](../../AGENTS.md#validation-scope) and
[selector usage](../../tools/testing.md).

## Scope

Recorded on 2026-09-05 on Windows, AMD Ryzen 9 7950X (16 cores, 32 logical
processors), mbx 1.8.1. The scheduling samples used existing library binaries
without compilation on an active workstation; background desktop/indexing work
was not disabled. The recorded worktree head was
`014327fb2ce19367a8b14468641fa78be185fc16`. These are workload timings, not a
measurement of hardware capacity. macOS performance was not measured.

## Scheduling samples

All durations are seconds. Each source run had 150 passes, one local-origin
failure, and 12 ignored tests; manager runs had 262 passes and two ignored tests.
Those are historical outcomes, not claims about today's tests.

| Workload | Recorded samples |
| --- | --- |
| package-source, 32 threads | 66.180, 63.884, 62.318 |
| package-source, 8 threads | 48.332, 58.196, 53.957 |
| package-manager, 32 threads | 65.608, 68.279, 76.395 |
| Both package binaries together, 8 threads each | 95.654, 96.588, 105.522 |
| Source confirmation, 8 threads | 61.203, 64.053 |
| Source confirmation, 32 threads | 64.697, 59.051 |

The confirmation runs had no other agent-owned build/repository work alongside
them and did not reproduce the apparent eight-thread advantage. Overlapping
the package binaries reduced their combined elapsed phase compared with the
sum of standalone measurements, but substantial interference remained. These
results do not justify a universal thread cap. Splitting Cargo package builds
also changes feature-unification conditions: package-manager enables
package-source's `test-fixtures` feature.

## Whole-run and selection limits

A single full eight-thread workspace run at the recorded revision took 168.522
seconds including build work, with 7,681 passes, three failures, and 14 ignored
tests across 117 targets. The failures were the package-source local-origin
case and two platform-custody persistent-journal cases. This uncontrolled run
does not establish a workspace speedup.

A nextest 0.9.140 experiment with per-package groups of eight stopped after
240.846 seconds total (57.96 seconds compilation) and was incomplete. A later
nextest library run on a migration tree based on
`ac3b45e9b462def078772e5bd5ca31be0a544ec0` took 357.071 seconds of execution,
following roughly 72 seconds of compilation, with 7,684 passes, the same three
reported failures, and 14 ignored tests. Neither establishes a nextest-only
speedup or a passing baseline.

An actual narrow nextest listing for an `x86-encoding/src/lib.rs` change selected
1,067 active library tests in 20 library binaries, compared with 7,687 tests in
117 binaries normally. This measured test-count reduction, not elapsed-time
improvement; package-manager remained selected.

Three warmed Windows README-only selector CLI trials took 28.060, 21.888, and
23.469 seconds end to end. Each ran 448 architecture checks and one corpus
audit, with no library tests. These samples include metadata, nextest
startup/build checks, and execution. Different cache conditions prevent a
matched comparison with the earlier full library run. The useful result is
that document-only selection omitted unrelated library execution, not a claimed
full-suite speedup ratio.
