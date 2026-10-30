# Cold variation: identical-binary controls

**Large run variation is established; a persistent regression in lazy graph
lowering is not.** The candidate's median centered whole-compilation wall
improvement is 2.29%, while identical-binary repeats vary by 8.46% median. Cold
performance remains unresolved, and the implementation stays parked.

This follows [deferred reference graphs](../lazy_reference_graph/README.md).
It changes the experiment to measure the noise itself, reusing the exact two
frozen executables and their normal accepted locks. Compiler code is unchanged.

## What made the previous negative pairs suspicious

The worst old cold pair adds 4.5264 seconds wall time. Of that, 3.1398 seconds
(69.4%) occurs in source checking, before the changed Terminal code, and 0.2196
seconds (4.9%) in Terminal lowering. The other negative pair adds 0.8185 seconds,
including 0.3332 checking and 0.0228 lowering. The patch changes no source-checking
algorithm. This suggests variation, but cannot retroactively identify a host
cause: those old observations have no system telemetry.

[summary.json](summary.json) retains the attribution for all six old cold pairs.
These built-in phase durations are wall observations, not additive CPU work or
a serial-fraction calculation.

## Design and results

Six four-run blocks use ABBA or BAAB, three of each in seeded shuffled order.
A is original and B candidate. Each role's mean position is identical, cancelling
a linear time trend. Every block also supplies an A/A and B/B repeat; outer and
inner repeat distances are balanced across roles. There are 24 cold compilations
and 12 same-binary repeat controls, with no timing-based exclusions or retries.

All runs use the same 203 authored inputs, target, EOF stdin and fresh private
compiler cache/output directories. The OS file cache and executable pages are
not flushed: cold compiler cache does not mean cold physical disk or cold CPU
caches. No priorities, affinity or user processes were changed, and no own builds
overlapped measurements. Each native image was actually executed and checked.

Every wall observation is below, in execution order within each producer. A
positive reduction means candidate faster. Block effects compare the two means;
the summary takes the median of those effects, not a ratio of global medians.

| Block | Order | Original runs, seconds | Candidate runs, seconds | Wall reduction | CPU reduction | Lowering reduction |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | BAAB | 29.8643, 28.2805 | 23.5373, 22.0490 | 21.60% | 17.43% | 30.32% |
| 1 | ABBA | 20.0768, 16.6441 | 18.4882, 16.1123 | 5.77% | 7.12% | 2.33% |
| 2 | BAAB | 17.1170, 18.1620 | 16.6960, 19.1845 | -1.71% | -1.24% | 19.70% |
| 3 | ABBA | 19.8389, 18.8223 | 20.0579, 24.6049 | -15.52% | -10.81% | -33.81% |
| 4 | ABBA | 19.3803, 24.6140 | 21.2774, 21.0939 | 3.69% | 1.77% | 10.45% |
| 5 | BAAB | 24.4907, 22.3642 | 24.1302, 22.3110 | 0.88% | -5.07% | 14.49% |

| Metric | Candidate improves | Median block reduction | Median identical-binary absolute variation |
| --- | --- | --- | --- |
| Whole wall | 4/6 blocks | 2.29% | 8.46% |
| Whole CPU | 3/6 blocks | 0.27% | 7.29% |
| Source checking wall | 1/6 blocks | -4.34% | 14.15% |
| Terminal lowering wall | 5/6 blocks | 12.47% | 16.13% |

Same-binary variation is `100 * abs(second - first) / mean(first, second)`.
It is descriptive repeat spread, not a confidence interval for the block effect.
Wall variation reaches 23.79% on that symmetric definition. In the adjacent
candidate control, 20.0579 -> 24.6049 seconds is a 22.7% slowdown relative to the
first run, larger than the earlier 19.64% negative A/B pair with no binary change.

## What the resource observations support

Non-target work consumed 14.7-88.0% of the 32 logical processors' CPU capacity
over these runs. The adjacent candidate control above moves from 36.4% to 67.8%.
Per-producer Pearson correlations between wall time and non-target CPU demand
are 0.903 original and 0.825 candidate. These correlations corroborate a changing
environment; they are not causal fractions or timing correction factors.

Process I/O accounting shows 26,784,150 read bytes / 4,646 read operations for the
candidate, versus 26,982,204 / 4,670 original. Writes remain approximately
230,621,242 bytes in both producers (original varies by 22 bytes). There is no
observed growth of cold write volume. Peak commit and page-fault effects are
mixed, with median block reductions of 0.22% and 0.02%. No exact instruction-count
or physical-disk equality follows from these counters.

The harness samples Win32 process and aggregate system counters every 250 ms
and at endpoints. It subtracts idle time from kernel+user system time, then
subtracts the target's CPU to estimate non-target demand. This is not scheduler
ready time, and identifies no particular external process. Page-fault counts
include soft and hard faults; peak pagefile usage is committed memory, not swap
traffic. All final memory counters were available. Elapsed sampler-call time
is 0.053 seconds median, 0.399 seconds maximum, at most 1.36% of observed wall.
That includes descheduling and is not an exact measurement of observer overhead;
no correction is subtracted. [research.md](research.md) links the Microsoft API
contracts and records the predeclared design and stopping point.

## What remains unresolved

The affected lowering phase favors the candidate in 5/6 blocks; the whole-compile
effect is smaller than observed same-binary variation. This does not demonstrate
a consistent regression in lazy graph lowering. It also does not prove a cold
speedup or zero regression.

Source checking is slower for the candidate in 5/6 blocks despite its unchanged
algorithm. Its own repeat variation is large. Environmental variation,
per-process behavior and producer/code-layout effects are not fully separated.
That result is preserved, rather than normalized away. Host activity alone
cannot explain every observation or assign the causes of the old slow pairs.

The previous acceptance is unchanged and the patch stays parked. More identical
trials on the same busy host are not a useful next step. If a smaller residual
producer effect must be resolved, a same-binary eager/lazy control with phase
CPU/scheduler evidence, or an isolated measurement environment, would remove
ambiguities this record cannot.

## Replay and validation

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/cold_variation/analyze.py
```

The analyzer verifies the manifest, input roster, exact existing producer/lock
identities, centered/balanced schedule, all 24 compilation/native outcomes,
raw telemetry accounting, all 12 same-binary controls and every block effect.
Every native image has SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`;
all native exits are 0 and all 463 output bytes match the preceding experiment.
The producers' prior both-engine comparisons, 24 focused tests and seven
baseline-equivalent source fixture outcomes remain applicable because their
binaries and source inputs are unchanged; those checks were not rerun. Existing
golden discrepancies remain as recorded in that experiment.

Base: `55fb4298335eece20c9d4426c6bf477380e69a69`, compiler semantic base
`456c5e984452a45d977295e254b3ca9ae3cc7750`. [identity.json](identity.json) binds
the exact inputs and producers; [run.py](run.py) retains the harness and self-check.
[Raw observations](raw/) and [run.log.gz](run.log.gz) retain all scheduled runs.
Accepted locks, caches, binaries and build outputs remain ignored local evidence.
Fresh execution requires the frozen producers or an explicitly recorded rebuild
and fresh labels; scripts refuse existing case directories.

No Rust source change, new promotion, full corpus, alternate gameplay, other
host, physical-disk trace, scheduler trace or zero-regression bound is claimed.
