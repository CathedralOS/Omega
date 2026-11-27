# Preparing service source correspondence once

The compiler repeatedly rebuilt the same inherited method signatures while
checking a provider's interface. The check now prepares their exact identities
once and records which source declarations they came from. One-method
interfaces keep the original scan and need no lookup-table allocation.

These are measurements of correspondence validation, not whole compilation.
The Console input is its actual five-method declaration block, not a claim
that a complete Console application runs. The two-method input is the unchanged
`tests/fixtures/run/providers/target_selected_implementation/main.omg` program.
The benchmark projects each input in an isolated source package.

## Measured work and time

Baseline: `82dade1565e228e1ff472134b185944ae0aecd53`. Candidate: the source
correspondence change accompanying this note. Windows x86-64, Rust development
profile with optimization level 1, shared 18-CPU build budget, idle priority and
affinity to logical CPUs 6–23. Each comparison reserved one mbx CPU permit;
the validator runs sequentially. Other agents used the same workstation.

Nine adjacent before/after pairs alternated their execution order. The table
shows median latency per validation and the median of the nine paired ratios.
Parsing, projection and interner warm-up are excluded from both revisions.

| Workload | Methods | Identity derivations before → after | Median time before → after | Paired ratio |
| --- | ---: | ---: | ---: | ---: |
| Single method | 1 | 2 → 2 | 3.88 → 3.79 µs | Treat as unchanged |
| Target-selected provider | 2 | 6 → 2 | 10.71 → 3.91 µs | 2.73× |
| Console declarations | 5 | 30 → 5 | 48.40 → 8.38 µs | 5.78× |
| Generated inherited service | 16 | 272 → 16 | 435.61 → 27.09 µs | 16.06× |
| Generated inherited service | 64 | 4,160 → 64 | 6,658.92 → 112.17 µs | 59.21× |

The 16- and 64-method inputs test scaling, not the workload distribution. The
one-, two-, and five-method inputs anchor the ordinary-size comparison.
The small timing difference for one method is not credited as an improvement:
that path uses the same algorithm and has identical allocation counts.

Identity counts came from separate, temporary counters at the callable-identity
derivations inside correspondence validation and its inherited-row preparation.
One warm-up and one measured invocation reported the same counts for each input.
The counters were removed afterward. Timing binaries were captured without
those counters; allocation measurement used separate binaries too.

## Allocation tradeoff

Each allocation run measured one warmed validation. Requested bytes count
allocation and reallocation requests, not RSS or cumulative retained storage.
Peak extra live requested heap is the maximum above the live baseline at the
start of that validation.

| Workload | Allocation requests before → after | Requested bytes before → after | Peak extra live heap before → after |
| --- | ---: | ---: | ---: |
| Single method | 53 → 53 | 2,346 → 2,346 | 499 → 499 B |
| Target-selected provider | 138 → 51 | 10,044 → 3,540 | 732 → 844 B |
| Console declarations | 711 → 124 | 32,784 → 5,884 | 512 → 792 B |
| Inherited, 16 methods | 6,667 → 398 | 304,950 → 19,158 | 502 → 1,398 B |
| Inherited, 64 methods | 101,923 → 1,574 | 4,674,092 → 76,588 | 502 → 4,680 B |

Preparation retains one compact row table and two vectors of row positions for
the duration of validation. One order joins complete source handles, including
their generations; the other detects missing or ambiguous canonical identities.
No table spans all program symbols, and no normalized identity text is copied
into the table. This increases temporary live heap while removing repeated
normalization allocations. The table is not retained as a program cache.

Sampled whole-process peak RSS had medians of 13,561,856 B before and
13,611,008 B after, with ranges of 13,508,608–13,635,584 B and
13,455,360–13,955,072 B respectively. These measurements do not establish a
process-memory reduction.

After the repository comparison driver moved to Rust `xtask`, a second set of
nine alternating pairs over the same saved executables reproduced the counts
exactly and measured paired ratios of 2.72× for the two-method provider and
5.70× for Console declarations. The table above retains the first run rather
than mixing samples from the two runs. The Rust command records native kernel
resource observations; the earlier RSS figures above came from sampling and
are not interchangeable with those observations.

## Execution checks and limits

The unchanged target-selected provider program has native/Psi agreement at
exit 0 on Windows x86-64. The complete
`tests/fixtures/run/providers/default_console_provider_reached` application
still stops at 11 existing lowered-form checker refusals about reference use
without exact live backing in `windows_x64_scalar_parameters`.
Separate baseline and candidate compilers reported all 11 identical diagnostic
messages and source locations during the verbose interpreted and native comparison, each exiting 200.
These measurements do not establish successful Console application execution
or a whole-compilation speedup.

## Reproduction and evidence

See [the portable driver instructions](../../../tools/benchmark/service-correspondence/README.md).
The local receipts are under `worker-33/build/service-correspondence/`:
`final-comparison.json` retains the nine pairs and all four executable hashes;
`rust-comparison.json` retains the separate Rust-driver confirmation;
`before-counted.log` and `final-counted.log` retain the separate identity counts.
The temporary counter patches are `before-counter.patch` and
`final-counter.patch`. They are observations, not compiler changes to ship.

Timing executable SHA-256:

```text
before e11bbf21fed9cd6e8c695e84f95bbe835c6e34f8c8d7d70ca3d001792ba239b0
after  14a798deeb1f2f8eed9091d4c31ab13eaf150329efbf58fdb6043193668e17eb
```
