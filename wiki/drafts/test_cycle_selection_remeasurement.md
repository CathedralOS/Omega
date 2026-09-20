# Test-cycle selection remeasurement

Controlled replacement measurement for the selection and scheduling costs
recorded in [test_cycle_measurements.md](test_cycle_measurements.md). Same
method: sequential runs on an otherwise idle checkout, wall-clock `time` around
each invocation, no concurrent agent-owned builds or tests. Where the earlier
note measured Windows scheduling samples, this page measures the selector's
three input classes and the selected-versus-full execution gap on a single
host.

## Scope

Recorded on 2026-09-20 on Linux x86-64 (8 logical CPUs), cargo 1.100.0-nightly
(toolchain `nightly-2026-09-04`), cargo-nextest 0.9.144, no `mbx` — the
selector used Cargo directly. Worktree head `61da8491a2032df4140637d420387a534b5e890c`.
These are workload timings, not a measurement of hardware capacity, and they
do not rerun the earlier page's Windows/macOS experiments.

## Selection cost

`python3 tools/test_affected.py --base HEAD --plan` measured end to end,
including `git diff`, `cargo metadata --locked`, and plan construction:

| Changed input | Selection cost (s) | Selected work |
| --- | ---: | --- |
| Documentation only (`wiki/drafts/*.md`) | 0.138 | Architecture tests + the `retired_domain_when_surface_is_absent_from_authored_corpus` corpus audit; library filter `none()` |
| One crate source (`x86-encoding/src/lib.rs`, comment appended) | 0.129 | 16 affected packages via `rdeps(...)`, still architecture + corpus audit |
| Unclassified input (file under `tools/`) | 0.167 | `all()` fallback |

The selection step itself is sub-second and insensitive to input class; the
metadata call dominates it. Selector CLI cost is not a test-cycle concern at
this revision.

## Selected-run cost

| Scenario | Wall (s) | Test counts | Notes |
| --- | ---: | --- | --- |
| Documentation-only change | 6.7 (warm) / 60.9 (first run, cold) | 547 architecture (535 pass, 12 preexisting failures) + 1 corpus audit | Cold run spent 46.3 s building the `compiler` `canary_suite` test target |
| `x86-encoding/src/lib.rs` change | 938.2 | 1,068 library tests across the affected crates + 547 architecture + 1 corpus audit | 1,060 pass, 8 preexisting failures (package-manager review/discovery cluster, native-realization exclusion adjudication); wall time dominated by two native-realization tests, `stack_probe_commit` (323.7 s) and `runtime_spill_pressure` (355.8 s) |

The selected test count nearly matches the earlier recording (1,068 selected
of 15,666 total library tests versus 1,067 of 7,687 recorded on Windows), but
the elapsed saving is conditional: selection removed ~93% of tests yet the
run still cost 938 seconds because the two slowest tests in the workspace
(>5 minutes each) were selected. Selection reduces count reliably; it reduces
elapsed only when the slow tail is unselected.

## Full-suite control

`cargo nextest run --locked --workspace --lib --no-fail-fast` at the same
head, two sequential runs: 1,223.6 s and 1,221.4 s wall (1220.7 s nextest
summary), 15,666 library tests across 117 binaries: 15,571 passed (31 slow),
95 failed, 2 skipped. The failures are the preexisting baseline clusters in
[known_baseline_failures.md](known_baseline_failures.md) — selected-dispatch
and checked-trees-to-lowered-psi dominate, then terminal-codec block-wire,
package-manager review/discovery, abstract-operations-to-abstract-operations,
source-files-to-assembled-syntax, typed-trees-to-checked-trees,
calling-conventions and native-realization. At this revision a single-crate
selected run costs roughly three quarters of the full library run's wall time
once the slow tail lands in the selection.

## Conclusions

- The `test_affected.py` plan step is sub-second on this host; do not optimize
  it.
- For documentation-only diffs the selector replaces ~1200 s of library
  execution with ~7 s of architecture-plus-corpus work — the mechanism's main
  win is intact on Linux.
- For single-crate diffs the win is diluted by the slow-tail concentration:
  `native-realization`'s multi-minute runtime tests gate any selection that
  includes it. The bounded follow-up landed as `slow_tail` in the
  `test_affected.py --plan` output: a selection naming a measured-slow owner
  reports the test names and seconds before paying them. An exclusion list
  for routine diffs remains a separate coverage-policy decision.
