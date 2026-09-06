# Local test-cycle measurements

Measured on 2026-09-05: Windows, Ryzen 9 7950X, 16 cores / 32 logical processors,
mbx 1.8.1. All commands ran locally. No GitHub build, model call, or queue-policy
change was introduced. The three previously completed compiler commits remain
separate from this investigation.

## Where the time goes

Four previous complete library logs consistently put the cost in package tests:

| Library target | Minimum seconds | Mean seconds | Maximum seconds |
| --- | ---: | ---: | ---: |
| package-manager | 60.52 | 66.41 | 70.01 |
| package-source | 49.58 | 55.43 | 59.60 |
| bounded-process | 4.49 | 6.71 | 9.30 |
| typed-trees-to-checked-trees | 3.02 | 3.05 | 3.09 |

These logs cover successive integrated revisions, so they locate the dominant
work rather than establish a controlled speedup. Cargo executes test binaries
one after another while libtest parallelizes tests inside each binary. The two
Git-heavy package suites therefore add their elapsed times. They create fresh
repositories, invoke real Git commands, and exercise filesystem custody; the
checker suite's roughly 1,900 tests complete in about three seconds.

The complete finalc95 verification record took about 324 seconds across format,
normal Clippy, architecture, workspace check, two focused integration targets,
additional Clippy coverage permitting the reproduced baseline lint, and library
tests. It also required separate failure attribution. Build-cache warnings
named source paths removed by upstream representation moves; this investigation
did not measure their incremental cost or change mbx.

## Repeated scheduling experiments

Each condition below has three runs. The experiments executed the same existing
test binaries, excluding compilation, on the active workstation. Background
desktop/indexing activity was not disabled. The raw samples are in
[testing_performance.json](testing_performance.json). Minimum, mean, and maximum
are reported because the host was noisy; these are workload timings, not a
measurement of the hardware's CPU or storage ceiling.

| Workload | Minimum seconds | Mean seconds | Maximum seconds |
| --- | ---: | ---: | ---: |
| package-source, 32 test threads | 62.32 | 64.13 | 66.18 |
| package-source, 8 test threads | 48.33 | 53.49 | 58.20 |
| package-manager, 32 test threads | 65.61 | 70.09 | 76.40 |
| Both package binaries together, 8 threads each | 95.65 | 99.25 | 105.52 |

Every source run retained 150 passes, one existing local-origin failure, and
12 ignored tests. Every manager run retained 262 passes and two ignored tests.
The binaries performed observable fixture creation and checked their results;
this was process scheduling, not a synthetic arithmetic loop susceptible to
compiler dead-code elimination. No native code or optimization flags changed.

Overlapping the binaries reduced their combined phase compared with the sum
of their separate measurements. Perfect overlap would still take at least
the slower standalone workload, about 66 seconds in these measurements.
The observed overlap took at least 96 seconds, showing substantial interference.

A follow-up confirmation with no other agent-owned build or repository work
running alongside it did not reproduce the apparent eight-thread advantage:

| package-source confirmation | Minimum seconds | Mean seconds | Maximum seconds |
| --- | ---: | ---: | ---: |
| 8 threads, two runs | 61.20 | 62.63 | 64.05 |
| 32 threads, two runs | 59.05 | 61.87 | 64.70 |

Outcomes were identical. The source-only eight-thread recommendation was
withdrawn. macOS performance has not been measured.

## Full-run experiments

A full eight-thread workspace run on `014327fb2ce19367a8b14468641fa78be185fc16`
completed in 168.52 seconds: 117 targets, 7,681 passed, three failed, 14 ignored.
The failures were the known package-source local-origin case and two
platform-custody persistent-journal cases. However, package-manager took 80.72
seconds, slower than its standalone 32-thread measurements. This one full run
does not establish a whole-workspace speedup; a blanket cap was not adopted.

A split-Cargo prototype could overlap the two package suites, but separate
package selection can change Cargo feature unification (package-manager enables
package-source's `test-fixtures` feature). It was removed rather than claiming
equivalent full-workspace validation.

The already-installed nextest 0.9.140 was also tested with per-package groups
of eight, no retries, no fail-fast, and exclusive bounded-process tests.
It had completed only about 5,740 of 7,684 tests after about 183 seconds of test
execution, following 57.96 seconds of compilation. It was stopped at 240.85
seconds total. That interrupted experiment is not a full-suite result. The experiment did not establish a speedup on this Windows workload. The initial
profile was removed; nextest was subsequently adopted at the user's request with
change-based selection and earlier scheduling of the long package tests. That
adoption does not change the interrupted experiment into a passing result. Nextest documents its
[process-per-test execution](https://nexte.st/docs/design/how-it-works/) and
[global resource scheduling](https://nexte.st/docs/configuration/threads-required/).

## What this means for landing

The dominant delay in the earlier session was repeating the entire baseline
after incoming main changes, not just one slow run. A 180-second reservation
cannot reliably contain the historical roughly 324-second verification sequence.
Thread tuning alone does not fix that mismatch.

The new [nextest workflow](testing.md) can select affected library tests when
rechecking an already verified base under unchanged environmental inputs. Shared
or unknown inputs still run everything. It does not weaken assertions, suppress
failures, or bypass the queue. No active lease was extended here.

## Migration validation

On the isolated migration tree based on `ac3b45e9b462def078772e5bd5ca31be0a544ec0`:

- Nextest architecture: 448 passed in 9.258 seconds of execution.
- Nextest libraries: 7,684 passed, three failed, 14 ignored; 357.071 seconds
  of execution, following about 72 seconds of compilation. This confirms that
  nextest alone did not make the full suite faster on this host.
- Failures: package-source's `byte_identical_content_does_not_cross_local_origins`
  and platform-custody's two persistent journal lock tests. These are the same
  three failures observed with Cargo before the migration; no Rust code changed.
- Selector: ten Python tests passed, covering real staged/unstaged/untracked
  Git changes, deletions, moves, dependency closure, source readers, fallback,
  empty selections, and continued execution/nonzero status after failure.
- An actual nextest listing for an `x86-encoding` source change selected 1,067
  active library tests from 20 library binaries, versus 7,687 from 117 normally:
  86.12% fewer tests. The selector's 21-package closure matches that listing plus
  the binary-only `omega` crate. Package-manager remains selected; package-source
  does not. This is a coverage-count measurement, not a wall-clock speedup trial.
- A real nextest run of the binary-only `omega` library selection correctly
  reported zero tests and succeeded.
- The advance evaluation harness now retains the full first library log instead
  of executing the entire suite a second time to enumerate failures. A real
  shell run with stub commands confirmed one library invocation, both an early
  and a late failure retained beyond the summary tail, and exit 100 recorded.
- Workspace check passed. All 119 packages passed formatting individually;
  `cargo fmt --all` exceeded Windows' command-length limit at this worktree path.
- Strict Clippy still fails at `set_readonly(false)` in package-source's
  `git/tests/exact_revision/failures.rs:91` and `git/tests/root_pin.rs:194`.
  These files are unchanged by the migration. Clippy warnings were not suppressed.

The Python and nextest workflows ran on Windows. macOS runtime validation is
still outstanding. No new performance percentage is claimed for that host.

## Documentation selection follow-up

Audited root project docs and Markdown under `wiki/` now select all 448
architecture checks plus the compiler corpus audit, with no library tests.
Other Markdown (including fixture inputs), configuration, tools and unknown
paths still select all libraries. A mixed docs/Rust edit retains the Rust
reverse-dependency closure. See [the exact allowlist](testing.md).

Three real README-only edits were checked through the complete Python CLI on
the warmed Windows worktree. Each plan contained only README.md, selected
`none()` for libraries, ran the two required commands, and passed all 449 checks.
The probe edit was restored byte-for-byte afterward.

| End-to-end seconds | Minimum | Mean | Maximum |
| --- | ---: | ---: | ---: |
| 28.06, 21.89, 23.47 | 21.89 | 24.47 | 28.06 |

These samples include metadata, nextest startup/build checks, and test execution
on the active workstation. They do not establish a matched full-suite speedup
ratio: the earlier 357.071-second library run had different cache conditions.
The removed work is concrete: documentation-only rechecks no longer execute
7,687 unrelated library tests.

Validation also included 13 selector regression tests, compiler formatting,
and strict Clippy for the changed compiler canary target. An injected untracked
Markdown file with invalid retired syntax failed with exit 100 and identified
the exact file and line; removing it restored a pass. The corpus audit now
includes untracked, nonignored files so staging cannot determine whether this
check sees a new document. Existing full-suite failures were not suppressed or
retested as part of this focused documentation change. macOS was not available.
