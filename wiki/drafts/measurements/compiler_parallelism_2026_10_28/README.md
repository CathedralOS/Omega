# Rayon worker-count experiment

Date: 2026-10-28, America/Los_Angeles. First experiment from the
[compiler parallelism research](../../reference/compiler_parallelism.md).

## Question and fixed inputs

Does increasing the existing Rayon worker count improve successful single-process
native compile latency, and where do CPU work and memory costs begin to outweigh
the benefit? This experiment changes no compiler algorithm or global setting.

Fixed producer/source revision: `bf39a3c901bc8ea3ee60b8b57fdc408a591dbacb`.
The isolated checkout is `.agents/worktree/parallelism-sweep`; its primary checkout
was not updated. Producer construction uses `mbx build -p omega --release`, a
dedicated target directory, Cargo checksum freshness plus content fingerprints,
and the repository's effective-profile verification bound to the resulting bytes.

Host: Windows 11 x86-64, AMD Ryzen 9 7950X, 16 physical cores / 32 logical CPUs,
approximately 127 GiB visible RAM. The host is shared; no other sessions or owner
applications were stopped. The producer build finishes before timed runs begin.

Selected existing fact places, all with authored exit 0:

- `samples/cli/facts/length/local/main.omg`: small runtime view-length compile.
- `samples/cli/facts/dynamic_trait_dispatch/local/main.omg`: dynamic dispatch.
- `samples/cli/facts/thirtytwo_arg_stack_calling_plan/other_machine/main.omg`:
  register/stack ABI and native-allocation pressure.

The source, sibling Build/oracle files, invalid twins, target and producer remain
fixed. Every process receives this checkout's `OMEGA_LIBRARY_ROOT`. Within each
cache regime, vary `RAYON_NUM_THREADS`: 1, 2, 4, 8, 16, 32. These are configured
workers per pool, not an observed process-wide thread count. Fresh private-store
paths in the cold regime all represent the same empty starting state.

## Protocol and metric boundaries

Before timing, run all three positives with `omega run --both` at each worker
count, require explicit native/interpreter exit-0 observations, and check each
invalid twin rejects with the same diagnostic. Each compile uses a fresh process
and output directory. Every emitted image must exist, have a valid executable
envelope, match the subject's other native hashes and actually exit 0.

Each regime retains one complete curve as warmup. Five retained-cache curves
and three cold-cache curves alternate ascending and descending worker counts.
OS filesystem caches remain warm; compiler-cache state is specified separately
for each regime. Process and thread-pool startup remain included. Source/library
bytes and the producer digest are checked again at the end. Failed runs stop
the experiment; they are never counted as faster compiles.

Wall time includes process creation through exit. Root compiler user/kernel CPU
time comes from [GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes).
Its sum divided by wall time is average busy cores for that process, not pool
utilization. [GetSystemTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes)
records aggregate host busy CPU time over each observation to expose background
work; it does not attribute particular competing processes.

The existing benchmark helper's Windows job counter is reported as **peak job
private committed bytes**, including descendants, following the
[Windows job-memory contract](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information).
It is neither peak RSS nor the reservation implied by 256 MiB worker stacks.
Peak RSS, stack reservation and batch throughput are not measured. Separate
thread observations qualify the active path; they are not timed samples. Phase
durations are captured separately from wall time; overlapping work must not be
added as if sequential.

The [runner](sweep.py) reuses existing optimized-profile admission, timing parsing,
native-image checks and Windows job accounting. Its self-check verifies nonzero
exit capture, missing-artifact refusal, output capture, CPU accounting and a known
32 MiB allocation. No library, compiler, fixture or task-board source is changed.

## Replay

From a detached checkout of the recorded revision, with these experiment files:

```powershell
python tools/build_slot.py --timeout 600 -- python wiki/drafts/measurements/compiler_parallelism_2026_10_28/build_and_sweep.py
python tools/build_slot.py --timeout 600 -- python wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py --cold build/parallelism-target/release/omega.exe
```

The [build chain](build_and_sweep.py) holds one build slot through construction,
profile verification, self-check and sweep. It writes an immutable producer
receipt and raw logs under `build/parallelism-first`; it refuses an already-used
sample output directory. Run in a fresh evidence directory/check-out when
repeating; never overwrite original observations or relabel their producer.

## Result

**Decision: no default or compiler change retained.** These observations do not
establish an optimal worker count. The retained-cache curve largely bypasses the
parallel frontend, while cold timings vary substantially on this shared host.
Increasing every pool to 32 workers increases measured CPU work and committed
memory without a consistent end-to-end latency advantage across the subjects.

Producer SHA-256: `3e4fb39360e6b84e89f57b6cd56ac1b486e9146892694ec529a477613fd78720` (verified opt-level 3).
The CLI version reports `unrecorded`; source provenance comes from the fresh
isolated build and its captured Cargo receipt, not that version label.
Both measurement commands completed with exit 0 and their final source/binary
identity guards passed. No Rust source, library, fixture or task board changed.

### First qualify the path being measured

The initial sweep used persistent prepared-source/checked-graph reuse. A separate
profile reported `source.graph.prepared_hits = 1` and
`05.b.retained_match.hit = 2`, with no machine-validation loop. A flat worker
curve under that condition does **not** show that checker parallelism scales
poorly. Separate instrumented checking observations saw 7 versus 8 root threads
at requested counts 1 and 32; full native observations likewise saw 7 versus 8.

A process-local `LOCALAPPDATA` override to a fresh task-owned directory creates
an empty compiler store through the existing
[cache-base contract](../../../../omega-rust/omega/packages/source-acquisition/src/storage.rs).
No shared cache was deleted, moved or disabled. This profile reported
`source.graph.cold = 1`, reached the machine-validation loop and visited 414 flow
groups. Separate cold `--check` probes with the calibrated OS thread counter
then observed:

| Cold requested workers per pool | Observed peak root threads |
|---:|---:|
| 1 | 11 |
| 32 | 103 |

The counter recovered a known three-thread increase, 5 → 8, in an actual Python
runtime. The first calibration addressed the Python launcher instead and was
rejected; the successful control uses `sys.executable`. Thread probes are separate
instrumented checks, never latency samples. One checking job-accounting probe
also confirmed one process, rather than attributing descendant CPU to the root.

The global, parser and checking pools are distinct. In particular,
[batch sizing](../../../../omega-rust/psi/compilation/driver/src/source_parsing/batches.rs)
queries `rayon::current_num_threads()` before entering the parser pool; outside a
pool this consults the
[global registry](https://docs.rs/rayon/1.12.0/rayon/fn.current_num_threads.html).
The observed thread multiplication is consistent with these owners. It is not
evidence that all those threads do useful work simultaneously.

### Native compile latency

Seconds, **median (minimum–maximum)**. Every slow valid sample is retained.

Retained-cache regime: five measured observations per cell, plus one excluded
warmup per cell. Fresh process/output directory, warm compiler/OS caches:

| Workers per pool | Length | Dispatch | Stack32 |
|---:|---:|---:|---:|
| 1 | 0.875 (0.817–1.121) | 1.002 (0.818–1.479) | 2.399 (2.144–2.502) |
| 2 | 0.853 (0.811–0.934) | 0.885 (0.816–0.919) | 2.312 (2.211–2.510) |
| 4 | 0.873 (0.804–1.181) | 0.877 (0.816–1.040) | 2.551 (2.189–2.621) |
| 8 | 0.975 (0.828–1.346) | 0.956 (0.823–1.068) | 2.586 (2.124–2.660) |
| 16 | 0.870 (0.823–1.056) | 0.911 (0.812–1.441) | 2.391 (2.069–2.651) |
| 32 | 0.887 (0.812–1.020) | 0.903 (0.812–1.048) | 2.263 (2.136–3.293) |

Cold compiler-cache regime: three measured observations per cell, plus one
excluded warmup per cell. Every command receives a newly created private store;
OS caches remain warm. This is a fresh-store startup comparison, not a claim
about a normal one-body edit with reusable dependencies:

| Workers per pool | Length | Dispatch | Stack32 |
|---:|---:|---:|---:|
| 1 | 3.010 (2.530–3.125) | 2.652 (2.540–3.037) | 6.823 (6.353–8.074) |
| 2 | 2.135 (2.093–2.604) | 2.615 (2.366–2.745) | 6.851 (5.802–7.112) |
| 4 | 2.619 (2.576–2.801) | 2.598 (2.232–3.178) | 4.821 (4.685–5.554) |
| 8 | 2.325 (1.988–2.556) | 2.609 (1.709–2.947) | 5.963 (4.165–6.087) |
| 16 | 2.116 (1.864–3.405) | 2.647 (2.141–2.689) | 5.460 (4.975–6.726) |
| 32 | 3.206 (2.182–3.367) | 2.952 (2.050–3.076) | 6.766 (4.617–7.478) |

The best observed medians differ by subject. No percentage from this table is
promoted to a repeatable speedup. Aggregate host busy CPU during each observation
ranged from 8.3–22.0 logical-core equivalents in the retained regime and
10.1–29.9 in the cold regime. Those totals include this compiler and background
work; they explain why this run cannot select a universal latency optimum.

### Work and memory cost

Medians from the cold regime. CPU is root-process user plus kernel time.
Memory is the job's private committed bytes, not RSS or reserved stacks:

| Cold subject | CPU ms, 1 → 32 | Peak private commit MiB, 1 → 32 |
|---|---:|---:|
| length | 2703 → 4359 | 384 → 990 |
| dispatch | 2500 → 4406 | 380 → 1019 |
| stack32 | 6281 → 8375 | 488 → 1081 |

The monotonic memory increase across intermediate worker counts is retained in
the raw record. Reducing this cost is a concrete opportunity; its effect on
single-program latency still needs a matched experiment.

### Correctness and evidence closure

- 144 measured native compiles: 90 retained-cache + 54 cold-cache.
- 36 separately labelled warmup compiles; none counted in the medians.
- All 180 emitted images have stable per-subject SHA-256 across every worker
  setting **and both cache regimes**, and all actually exited 0.
- 36 positive `run --both` invocations explicitly observed native and Terminal
  exit 0 (three subjects × six settings × two regimes).
- 36 invalid controls rejected for the intended index, dynamic-call requirement
  or exact-arithmetic bound. Per-subject diagnostics match across settings and
  cache regimes. No refusal or missing artifact counts as a passing compile.
- The first runner attempt stopped on an extended Windows path bookkeeping
  error before recording any accepted timing. Its pilot logs are preserved and
  excluded. The runner now records the exact absolute artifact path and tests
  a valid executable outside the checkout as well as failed/missing artifacts.

[Compressed evidence](evidence.json.gz) contains the unrounded observations,
source/oracle hashes, benchmark/profiling logs (including the excluded pilot), exact warm
and cold runner sources, host/probe records and the verified
[producer receipt](producer.json). The optimized producer, build log, native
images and private caches remain in this worktree's `build/parallelism-first`
and `build/parallelism-target`. The receipt and packet identify those original
paths and bytes; no remote/main or other-host result is claimed.

### Next bounded experiment

Test one concurrency-budget change, starting with the avoidable global-registry
initialization during parser chunk sizing. Freeze the producer's compiler/library
inputs, use independent baseline/candidate artifacts, verify the intended thread
reduction on a cold path, and compare cold end-to-end time, CPU and committed
memory while keeping the retained-cache path as a control. Require the same
engine outcomes, negative diagnostics and native bytes. Retain a change only
when its benefit is measured; do not promise an absolute maximum or replace the
default from this noisy curve. Batch throughput remains a separate unmeasured
question.

The proposed parser-pool change was subsequently
[tested and rejected](../parser_pool_2026_10_28/README.md): fewer threads did not
establish a reliable latency or committed-memory improvement.
