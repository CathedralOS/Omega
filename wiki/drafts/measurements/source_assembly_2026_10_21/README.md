# Source assembly attribution, 2026-10-21

Source assembly is 7.091 ms median, about 0.98% of the 723.767 ms median
`length/local --check` workload. This first attribution capture retained no
implementation change. Its original decision to stop because assembly was a
small whole-program fraction was incorrect for the requested per-layer work.
The [subsequent parser experiment](prefix_probe/README.md) removes measured
work and reduces assembly time; `CHECKED-PACKAGE-REUSE` remains separate.
The [import/forest assembly audit](import_assembly/README.md) transfers consumed
source metadata buffers, removing allocations with no claimed timing speedup.

This continues the ingress work landed at
`245ce317c6742015981a84ca35cb6bab60ca0f63`. Both producers use that source;
the diagnostic producer adds only the adjacent `instrumentation.patch`.
The host is an Apple M4, macOS ARM64, with the repository's optimized dev
profile (opt-level 1). Exact profiles, binary hashes, source hashes, library
tree, toolchain and all forty observations are in `observations.json`.

## Comparison and decision

Twenty paired rounds alternate which producer runs first. Every invocation
starts a fresh CLI process with warm filesystem/build state. All forty checks
exit 0 and report 20 compiled source files. No timed observation was excluded.
No builds ran during this batch; other workstation activity was left alone.
The one-minute system load average ranged from 4.19 to 4.72. These are observed
host timings, not an isolated-machine guarantee or a throughput benchmark.

| Operation | Baseline median | Baseline range | Diagnostic median | Diagnostic range |
| --- | ---: | ---: | ---: | ---: |
| Project preparation | 4.173 ms | 4.064–10.667 ms | 4.152 ms | 4.023–14.948 ms |
| Source assembly | 7.091 ms | 6.843–10.000 ms | 7.223 ms | 7.010–22.245 ms |
| Stage 05 | 607.165 ms | 598.818–671.132 ms | 606.730 ms | 597.000–676.083 ms |
| Total elapsed | 723.767 ms | 714.063–819.409 ms | 722.536 ms | 711.934–829.559 ms |

The diagnostic producer is an attribution instrument, not an optimization.
Its 0.132 ms larger assembly median includes probe overhead; the total-time
variation does not establish a speedup. The inclusive assembly scope contains
loading, lexing and parsing, so its time must not be added to Stage 01/02.

Within diagnostic assembly, entry-contract imports cost 5.880 ms median and
the generated Build prelude 1.089 ms. Child validation costs 0.023 ms, syntax
storage cloning 0.042 ms, and import binding retention 0.026 ms. These separate
medians are descriptive and need not sum to the median inclusive interval.
Across all physical import waves (including preparation, not only assembly),
`SourceInput::append` costs 5.459 ms median, frontier admission 0.258 ms and
import discovery 0.498 ms. `append` includes source loading, lexing, parsing,
and syntax-page adoption. This capture does not isolate parsing CPU from I/O.

This capture alone does not isolate a custody, clone or parser cause. Assembly's
roughly 1% share of total time does not establish that its work is necessary or
that optimization should stop. The follow-up isolates speculative parser work
and removes it without a cache or relaxed checks. Stage 05 remains about 84%
of this historical total; that does not supersede the owner's per-layer scope.

## Replay and validation

From the repository root at the recorded base:

1. Build with `mbx build -p omega`. Create `build/source-assembly/`, and copy
   `target/debug/omega` to `build/source-assembly/omega-baseline`.
2. Apply `instrumentation.patch` using `git apply --unidiff-zero` and build the same package.
3. Run `python3 wiki/drafts/measurements/source_assembly_2026_10_21/measure.py`.
   It checks producer optimization, alternates 20 pairs, requires every command
   to succeed, and writes full outputs to `build/source-assembly/observations.json`.
4. Reverse this exact patch with `git apply -R --unidiff-zero` when finished.

The probes were removed after capture and the Rust tree is byte-identical to
the base. The baseline producer then ran
`run --both --keep samples/cli/facts/length/local/main.omg`: native exit 0,
Terminal interpreter exit 0, command exit 0. This is an actual engine observation,
separate from the forty source checks. No new language behavior is claimed;
there is no implementation diff requiring a new corpus or crate-test campaign.
The earlier untimed exploratory profile and cold first run were used only to
choose probe locations; they are not substituted for this paired batch.

The saved script emits raw records; the committed JSON retains their timings,
probe totals, exits and source counts, plus runtime evidence and producer
identities. Import-wave times are summed within each invocation. This record
covers one macOS ARM64 workload only and does not reproduce the previous
session's timing comparison or establish performance on other hosts.
