# Reuse declaration field inventories within source-contract discovery

The experiment removes repeated field-type collection from
`TypedTrees::applied_trait_roots`. It retains each original declaration's ordered
field-type handles within a single immutable discovery query. Every distinct
owner, type application, enclosing telescope and recursion check still traverses
those handles with its own context.

Baseline: `1f5fd5361e140b820a353d113a8a784b68572670`, fetched from main before
creating the isolated worktree. The host is macOS ARM64; the producer uses the
repository's optimized dev profile (opt-level 1), with the pinned Rust toolchain.
This is a bounded compiler-throughput experiment, not a new language mechanism,
a completed optimizer task, or an assertion that checking is fully optimized.

## Attribution and mechanism

The initial uninstrumented benchmark harness observed full native compilation of
`samples/cli/facts/length/local/main.omg` at 744.392, 648.540 and 651.008 ms
(median 651.008 ms). Its final stage table attributes 507.342 ms to checking.
All three published programs exited 0. Peak compile RSS was 253,689,856 bytes;
the native image was 16,640 bytes. These are exploratory observations, not the
paired acceptance comparison.

Three macOS `sample` captures identified source-field contract discovery as one
repeated-work centre inside checking/preparation. Temporary phase-profile probes
then counted 21 discovery queries, 61,951 field inventories and 381,936 field
nodes on the length workload. Collection took 9.922 ms inside 53.585 ms of whole
source-field discovery. The candidate counted the same 21 queries but only 1,253
inventories and 5,417 nodes; collection took 0.243 ms. The candidate attribution
capture overlapped functional validation, so these diagnostic times are not a
controlled speedup claim. Probes were removed before final measurement.

`FieldTypes` reads only declaration fields and the immutable type-reference
table. Its collection does not depend on lexical owners, substitutions,
enclosing applications, the visited-occurrence set or the active recursion
stack. Previously every application repeated that same collection and allocated
another vector. Now a query-local table retains a range in a contiguous scratch
vector. The table indexes a declaration symbol's arena slot and compares the
complete generational symbol before reusing a range. `data_application` selects
the first declaration for that exact symbol in the same immutable program, so
the key denotes the same declaration throughout this call.

The unchanged collector preserves field order, deduplication, case payloads,
reference/slice stopping and erased/source type distinctions. The unchanged
contextual traversal still handles `Wrapper<Wrapper<T>>`, growing recursion,
selectors and service contracts at each occurrence. Recursive visits may append
to the inventory storage; callers copy handles by stable position rather than
holding a slice across reallocation. The table dies with this query, before any
subsequent preparation mutation. No semantic judgment is cached across stages,
no verifier is removed, and no new compiler mode or persistent cache is added.

[Research and applicability](research.md) explains why immutable query reuse is
relevant and why a cross-stage cache was not selected.

## Replay

Build and freeze the baseline and candidate CLI binaries as
`build/throughput/omega-{baseline,candidate}`. Each binary must report its actual
optimized build profile. Run `python3
wiki/drafts/measurements/compiler_throughput_2026_10_21/measure.py` from the
repository root after builds and corpus runs finish. It imports the existing
benchmark harness's process timing/RSS and output parsing functions, retains
warmups, alternates producer order, records all trials and runs every emitted
image against exit 0. It captures three existing fact workloads, exact binaries,
source hashes, library tree, compiler patch, stage timings and host load.

An initial use of the benchmark's `measure` command generated `omega.lock` for
the length subject. Switching to the instrumented producer then correctly
refused that lock's old compiler identity. The generated lock was removed to
restore the original lock-free subject, which the ordinary compile command had
already admitted. The refused capture is retained separately; it is not a
semantic regression or a timing sample. No accepted lock was copied between
producers and no compiler identity check was relaxed.

## Result and decision

Retained as a small work reduction. Two lower-variance batches show lower median
full compilation time on all three programs. A second batch taken between them
is inconclusive: both producers rose to roughly 1.2 seconds with much larger
variation. Its one-minute load average ranged from 9.97 to 15.68, versus
7.23–12.13 in the first batch and 4.46–6.39 in the final batch. This is evidence
of different host conditions, not proof that all variation comes from contention.
No valid observation or outlier was discarded. There is no tail-latency or
universal throughput guarantee.

Each batch contains twelve alternating pairs per subject, plus a retained warmup
for each producer/subject. The final batch additionally shuffles subject order
with a fixed seed and records process CPU time through child resource accounting.
No build or corpus process from this session overlapped a timing batch. Other
workstation activity was left alone. These are fresh CLI processes with warm
filesystem state, not an incremental-build or cold-start measurement.

Final-batch wall time, milliseconds:

| Fact workload | Baseline median (range) | Candidate median (range) | Median reduction |
| --- | --- | --- | --- |
| Length, local | 639.389 (629.958–695.562) | 631.285 (621.864–657.852) | 1.27% |
| Dynamic selector bound, local | 634.592 (627.907–648.723) | 628.690 (622.633–652.671) | 0.93% |
| Nine-level record bound, local | 638.883 (629.790–681.831) | 629.112 (621.472–657.282) | 1.53% |

The first batch's respective medians were 639.757→636.672,
640.047→633.562 and 644.200→634.898 ms. Its length mean increased by 2.024 ms
despite the lower median; that ambiguity motivated the additional batches.
In the final batch all three means also decreased (643.362→631.802,
635.024→630.804 and 641.149→632.320 ms). Median user CPU time decreased by
7.255 ms and 5.451 ms for length and dynamic dispatch, respectively, and was
nearly unchanged for the nested record (824.834→824.731 ms). Parallel compiler
work makes summed CPU time larger than elapsed time. No memory improvement is
claimed: final median peak RSS moved by about −0.11, +1.03 and −0.58 MB, within
the observed run-to-run variation.

All 234 emitted images across the three batches exit 0. Within each subject,
every baseline and candidate native image has exactly the same SHA-256. This
checks the actual downstream output as well as its authored runtime oracle.
The whole immutable query is still executed 21 times on the attribution
workload; this change only removes repeated field collection *inside* each
query. Source checking remains the dominant cost.

## Correctness and limits

The final implementation builds with `mbx build -p omega --bin omega -p compiler
--test corpus_runner --target-dir build/gate-cli`. File-scoped `rustfmt --check`
and `mbx clippy -p typed-trees --target-dir build/gate-cli` succeed; the latter
reports eight existing warnings outside the changed file.

Two frozen corpus runners measured the same fixtures and library closure with
`OMEGA_CORPUS_JOBS=2`, `OMEGA_CORPUS_ROOT` set to this checkout and
`OMEGA_CORPUS_RECORDS` naming each retained record file. The baseline runner
contains only the temporary attribution probes over the baseline source, with
profiling disabled; the candidate runner contains the final implementation and
no probes. [Producer identities](identities.json) bind all executable digests
and their opt-level-1 profiles; patches retain the exact diagnostic builds.

- [Source-check comparison](check-comparison.json): 1,053 discovered fixtures
  selected by [filter.txt](filter.txt), zero lost or gained outcomes, and zero
  changed diagnostics. Both runs report 438 checked, 611 rejected and four
  incomplete. Existing baseline issues remain: 26 positive/run fixtures refuse
  or are incomplete, and eleven negative controls miss an expected diagnostic
  fragment. They are individually listed in the record. This is equivalence,
  not a claim that the entire selection passes.
- [Native/Terminal comparison](both-comparison.json): 23 fixtures selected by
  [both-filter.txt](both-filter.txt), zero outcome or diagnostic changes.
  Twenty-one actually run in both engines and agree at exit 0, including all
  nine length places and eight dynamic-selector places. The parameter place of
  the dynamic-selector row and the applied-binding-field-telescope fixture
  reject identically on both producers; neither counts as runtime acceptance.
  The selected nested application and callback-scope controls also agree.
- The reject twins are included in the source-check selection. No source oracle,
  rejection expectation, compiler gate or golden was changed.

Run `python3 wiki/drafts/measurements/compiler_throughput_2026_10_21/compare.py`
to compare fresh records with the repository's outcome comparator plus exact
diagnostic comparison. The raw `.records` files retain every observed failure.
The final [paired observations](paired-final.json), [first batch](paired-first.json),
[second batch](paired-second.json) and [summary](summary.json) retain timing
ranges, individual observations, CPU times where collected, source/producer
identities, native hashes and exits. The exploratory benchmark is separate.
On Windows, the same Python replay uses the benchmark harness's job-object memory
accounting; POSIX CPU-time fields are unavailable rather than fabricated. Change
the explicit target to the executing host and use binaries built for it; no
Windows, Linux or cross-host performance result is established here.

This completes the bounded local throughput iteration. It does not close a
language capability, repair the retained baseline failures, or promote an
optimizer rule. The implementation and evidence are committed locally; remote
publication was outside the authorized scope.
