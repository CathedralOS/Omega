# Rebound deltas: allocation reduction confirmed, speed inconclusive

2026-10-29, fixed Dungeon EOF workload, Windows x86-64 / Ryzen 9 7950X,
shared busy host. Continues [cache diagnostics](../cache_diagnostics/README.md).
**Decision: preserve both candidates as experiments; retain no compiler change.**
The allocation reduction is established. These observations do not establish a
repeatable compile-time improvement, and the candidates were not promoted to
broader source/runtime qualification.

## Mechanism and scope

The original path already skips allocation when its entire rebound roster is
known. When any row is missing, it appends the whole roster and canonicalizes
it. The experiment keeps every substitution and normalization, but removes
exact already-retained rows from the computed roster before allocating Arcs.

The first variant preserves the original ordered scan / unordered binary-search
selection, retaining only missing rows in place. Its four timing pairs were
mixed, so a second variant stably sorts an unordered transformed roster once
and always subtracts with an ordered scan. Neither variant caches anything or
changes invalidation, incoming-edge joins, source contracts, authority or the
published canonical union. Both touch one caller and one private helper in
`payload_history.rs`; no source syntax or public API changes.

A source-free kernel test checks 1,024 retained-subset / rebound permutation and
duplicate cases, plus empty inputs, against the original full append/sort/dedup
union. Existing retained rows keep their Arc identity; empty deltas preserve
the published roster allocation. Both builds passed 23 selected payload tests
with 6 skipped. The complete transform still runs even for empty substitutions:
normalization can produce a genuinely new proposition.

## Counted result on the real program

Both strategies produced exactly the same hot-path counts:

| Observation | Count |
| --- | ---: |
| Rebinding requests | 7,683 |
| Fully computed output rows | 3,146,452 |
| Requests yielding no new rows | 6,921 |
| Requests appending rows | 762 |
| Arc allocations after filtering | 61,299 |
| Original append Arc allocations | 1,462,923 |
| Avoided Arc allocations | **1,401,624 (95.81%)** |

The original allocation count is the measured bypass from
[cache diagnostics](../cache_diagnostics/raw/cache-count-off/result.json), not
the rejected cache's larger allocation count. The trace is correlated by thread
inside 418 source-transfer windows. Whole-compile counts are 7,742 requests,
3,147,825 computed rows and 61,788 allocated rows. These are repeated roster rows
and allocation calls, not independent proof obligations or unique facts.

The existing worklist counters are unchanged: the main module has 202 machines,
109 candidates, 127 rounds (18 changing), 3,313 operation visits, 418 eligible
operations, 173 payload-target queries and 439 proposed updates. No stage,
substitution, premise-establishment obligation or incoming edge was bypassed.
The trace flag was absent during all primary timing runs.

## Timing comparisons

Each variant used alternating, matched fresh-cache runs against the same frozen
`payload` baseline, with identical outer attribution hooks and opt-level 1.
Negative changes below mean faster source replay; no valid run was discarded.

| Pair | Binary-search filter | Sort then ordered filter |
| --- | ---: | ---: |
| A, baseline first | +31.8% | +17.4% |
| B, candidate first | +27.5% | -10.9% |
| C, baseline first | -29.5% | -12.6% |
| D, candidate first | -22.6% | -16.7% |
| E, baseline first | — | +17.9% |
| F, candidate first | — | -14.7% |

The first four ordered-filter pairs suggested a lower replay median, but
included a slower observation. Two additional reversed pairs were explicitly
added to resolve that uncertainty; they split one slower / one faster. All six
are included in the summary.

| Median | Baseline / binary-search filter | Baseline / ordered filter |
| --- | ---: | ---: |
| Source replay, ms | 1707.89 / 1630.03 | 1910.61 / 1798.33 |
| Whole compile wall, s | 19.230 / 19.283 | 22.683 / 21.716 |
| Whole compile process CPU, s | 26.609 / 26.266 | 29.672 / 28.500 |

The source-replay median estimates are numerically 4.6% / 5.9% lower, but the
paired spread is large and the direction is not repeatable. These data are
encouraging for the allocation mechanism and inconclusive for the intended
compile-time benefit; they do not establish either a universal speedup or a
regression. Full-precision paired values, peak working set and peak process
commit are in [delta-summary.json](delta-summary.json) and
[delta-sorted-summary.json](delta-sorted-summary.json). No peak-memory reduction
is claimed.

Removing 95.8% of these allocation calls does not remove 95.8% of replay work.
The preceding bypass probe timed the original append interval at 42.5–67.9 ms;
this experiment still computes all 3.15 million transformed rows and adds
filtering/sorting work. The append interval is not a strict speedup bound:
allocation, locality and scheduling interact. The observations do not isolate
the cost of searching or sorting as the cause of any particular slow run.

## Behavior and evidence limits

All 24 compiler observations succeeded; every native artifact matched SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`
and executed with exit 0 and the authored EOF output. Each candidate separately
ran `omega run --both --keep`; native and Terminal Psi exits were zero and all
463 stdout bytes were explicitly compared. This establishes the real program's
artifact and output agreement, not general proof soundness.

No full corpus, invalid-source qualification, other host or interactive Dungeon
route was run. Focused positive/reject fixtures were identified but not executed
because no candidate was promoted; no fixture acceptance is claimed. All
production compiler files and temporary tracing hooks were restored.

## Replay

The working base is `c63ab0da5fc`, a measurement-only descendant of semantic
base `456c5e984452a45d977295e254b3ca9ae3cc7750`. The unchanged 203 authored source
files are checked by the existing [input manifest](../input-manifest.json).
Each compile case has a new build directory and private `LOCALAPPDATA` cache,
explicit library root, general phase profiler off, an optimized exact frozen
compiler and its ordinarily accepted lock. Update logs record zero changed
source entries. The two public `--both` correctness runs use the shared cache
and are excluded from the cold-cache timing comparisons.

[identities.json](identities.json) binds the baseline and both candidates.
Binaries and locks remain under ignored `build/proof-profile/producers/` as
`payload`, `rebinding-delta` and `rebinding-delta-sorted`. Each candidate's
selected tests and optimized CLI build ran inside one build-slot hold.

Choose exactly one patch: [candidate.patch](candidate.patch) or
[sorted-candidate.patch](sorted-candidate.patch) for a production-only replay,
or the corresponding `instrumented-*` / `sorted-instrumented-*` patch for outer
timing and optional row tracing. All four apply to scratch indices at the fixed
base; [patch-checks.json](patch-checks.json). `OMEGA_REBIND_DELTA_COUNTS` enables
trace by presence (even value `0`); leave it absent for timings. The trace emits
one before/after row count for each actual rebind. It does not change cache keys
or skip proof checks. [research.md](research.md) records the primary API source,
safety argument, falsification criterion and reason for the refinement.

The durable harness files reuse the previous compile/native/memory runner.
`rebinding_delta_run.py` contains the first count/four-pair/runtime schedule;
`rebinding_delta_sorted_run.py` contains the ordered variant's initial schedule;
`rebinding_delta_extra_run.py` contains its two extra reversed pairs. Existing
result labels are refused. The raw records and streams are in [raw](raw/);
large streams and build logs use lossless gzip. Verify the retained record with:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/rebinding_delta/analyze.py
```

Decision: keep the measured allocation finding and park both implementations.
The next useful question is how to reduce full-roster traversal/normalization
or merge canonical rosters once, with complete premise preservation. No such
additional change is included in this experiment.
