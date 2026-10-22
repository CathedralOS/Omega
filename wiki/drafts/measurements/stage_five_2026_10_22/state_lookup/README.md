# Stage 05 state-owner lookup experiment

Decision: reject both variants. Neither establishes a useful reduction in ordinary
compiler latency. The compiler source is restored exactly to the baseline;
the patches are experiment records, not production implementations.

Measured base: `b428c347b51786c5f57d577d59413977d6cf5d9f`.
Host: Apple M4, macOS ARM64, Rust nightly 2026-09-04, optimized dev profile 1.
The user requested optimization of Stage 05, the CLI timing label for typed-tree
preparation/checking (current checking directory `04_typed-trees-to-checked-trees`).
The workload is the unchanged `samples/cli/facts/length/local/main.omg --check`.
No language capability closure or remote publication is claimed.

## Hypothesis and decisions

The first profile attributed 113–160 ms to construction validation under shared
host load. A one-second macOS sampling capture then named
`TypedTrees::machine_holding_state` as the largest active leaf (63 samples).
Those are exploratory observations, not the latency denominator. Sampling has
startup/collection overhead and the short capture does not isolate a hardware
limit. The later warm paired runs are substantially faster.

That lookup checks the declared parent first, then may scan every machine and
state. The existing `StatePositionIndex` already records exact symbol-to-storage
relationships, with arena/roster freshness and stable duplicate order.
[Research](research.md) supported testing reuse of that index.

1. **Full index**: resolve both owner and state position from indexed occurrences,
   preserving parent preference, symbol eligibility and duplicate selection.
   [Patch](full_index.patch). It reduced fact-checking median in profiled runs
   from 90.581 to 84.539 ms, while machine-loop median rose from 33.347 to
   37.429 ms. The whole workload did not improve. The sub-pass difference does
   not establish a hardware cause for the offsetting cost.
2. **Fallback only**: preserve the original short parent membership lookup and
   use the index only after that fails. [Patch](fallback_index.patch).
   Stage median moved by 2.918 ms (about 0.5%), but minima differ by only
   1.154 ms and run variation is much larger. The largest baseline observation
   remains included. This does not justify a useful speedup claim or retaining
   another lookup API.

The index replacement is not the larger design intervention. Both variants
leave repeated checking work, symbolic queries and all pass boundaries intact.
The next question is the amount and lifetime of semantic work per compilation,
not another unmeasured search-container substitution.
The subsequent [PerfAware design review](../design_review.md) evaluates that
question against current profiles and the existing checked-package-reuse task.

## Repeated ordinary CLI timings

Each pair alternates producer order. Every invocation starts a fresh process
with warm filesystem state; all 64 ordinary checks return 0. No observation is
excluded. No build belonging to this experiment ran during either batch. Other
workstation activity was left alone; per-run load averages are retained.
These are latency observations on this host, not corpus throughput or an
isolated-machine result. They do not establish a memory, instruction-throughput,
or hardware-bandwidth limit. Minima are observed minima from fixed-count trials,
not convergence-certified repetition-tester floors.

| Experiment / scope | Before min / median / mean / max, ms | After min / median / mean / max, ms |
| --- | --- | --- |
| Full index, Stage 05 (12 pairs) | 519.673 / 544.274 / 542.939 / 554.061 | 521.265 / 547.202 / 545.255 / 556.419 |
| Full index, total | 638.188 / 668.540 / 668.139 / 690.094 | 639.868 / 670.766 / 668.706 / 688.759 |
| Fallback only, Stage 05 (20 pairs) | 532.410 / 556.246 / 562.920 / 672.300 | 531.256 / 553.328 / 551.462 / 577.216 |
| Fallback only, total | 650.885 / 682.515 / 688.960 / 803.639 | 650.406 / 678.615 / 676.680 / 716.175 |

[Full-index observations](timings.json), [fallback observations](hybrid-timings.json),
and [summary](summary.json) retain the exact binary digests and profiles, source
hash, patch hash, toolchain and every output. Five additional pairs with
`OMEGA_PHASE_PROFILE=1` are retained in [phase-timings.json](phase-timings.json);
they are attribution measurements and are not pooled with ordinary CLI timings.
Nested and parallel scopes overlap; do not sum their inclusive durations.

## Correctness and replay

The full-index candidate passed all 21 typed-tree tests. The extended tests
compare both public lookup results against the original scan algorithm across
mutations, clones, visible-roster changes, invalid generations, duplicates,
parent/storage disagreement, non-state symbols and trait requirements. They also
exercise frozen lookup. The fallback-only variant built successfully but was
rejected before a fresh test/corpus campaign. These are not interchangeable
validation claims. Neither experimental implementation remains in production.

The ordinary baseline separately ran `omega run --both --keep` on `length/local`:
native exit 0 and interpreter exit 0. [Runtime output](baseline-runtime.txt).
No candidate native/corpus equivalence is claimed; those planned gates were
cancelled after the latency experiment failed its retention criterion. No corpus
goldens or sample sources changed. Formatting and diff checks passed.

Replay in isolated checkouts at the recorded base:

1. Build `mbx build -p omega`, freeze the baseline executable, and verify its
   `--build-profile` output. Keep baseline and candidate target directories separate.
2. Apply one recorded patch; build and freeze its candidate executable.
3. Run `measure.py --baseline ABSOLUTE_BINARY --candidate ABSOLUTE_BINARY
   --output OUTPUT.json --pairs 12` (20 for fallback). Add `--profile` for attribution.
4. Run `mbx nextest run -p typed-trees` to exercise the included independent oracle.

The invocation's CLI outputs are consumed and retained, so this is not a
DCE-sensitive empty-loop microbenchmark. No assembly-level speedup claim is made.
Frozen executables, build logs and the full sample capture remain under
`build/stage-five/` in the attached worktree. The committed sampling summary
includes the original capture digest. This experiment closes only this bounded
lookup comparison; the user's broader Stage 05 optimization remains a design
and measurement question.
