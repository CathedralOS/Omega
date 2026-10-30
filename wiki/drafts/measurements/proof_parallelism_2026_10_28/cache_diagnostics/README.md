# Why the edge cache did not help

2026-10-29, same fixed Dungeon EOF program, Windows x86-64, Ryzen 9 7950X,
shared busy host. Follow-up to [the rejected candidate](../edge_rebinding/README.md).
**The cache almost never avoided a transformation, and its result representation
added allocation and retention work. No new optimization was attempted.**

## Counted behavior

Two independent fresh-cache count runs were byte-for-byte identical in their
aggregated metrics. Within the expensive source-contract replay interval:

| Observation | Count |
| --- | ---: |
| Requests | 7,683 |
| True hits | 24 (0.3124%) |
| Cold misses | 1,537 |
| Changed-allocation misses | 6,122 |
| Total misses / transformations still performed | 7,659 |
| Changed-allocation misses with equal contents | 42 |
| Changed-allocation misses with different retained rows | 6,080 |
| Input premise rows avoided | 98 / 3,140,756 (0.00312%) |

The constructor computes before its first `requirements` call. Counting that
immediate pointer match as a hit would have falsely reported 1,561 hits.
The analyzer subtracts 1,537 cold entries, and asserts:
`true hits + cold misses + changed-allocation misses == requests` and
`transformations == cold misses + changed-allocation misses`.
Whole-compile counts are similar: 24 true hits / 7,742 requests (0.3100%);
the hot interval is explicitly separated from the remaining compiler uses.

Equal-content diagnostics compare the complete ordered retained proposition
rows, including exact subjects/types; they do not prove logical equivalence of
differently written theories. They run only in count mode, never in the timing
or unprofiled comparisons. The miss distribution shows that allocation churn
alone is not the principal explanation. Row counts are not weighted expression
nodes, CPU work or independent proof obligations.

## Allocation and retention cost

The cached path made **3,146,338 `Arc::new` calls** for transformed output rows.
The original bypass path made **1,462,923** at this rebinding step, because it
wrapped only rows actually appended. That is **1,683,415 extra allocations,
or 2.1507 times the original count**, for almost no avoided input traversal.
These are source-level Arc allocation calls, not OS allocation or page counts.

In 6,921 / 7,683 requests (**90.08%**), the returned roster was already contained
in the current theory. The cached path had nevertheless wrapped computed rows
before asking that question. Those contained rosters represented 1,683,529
returned rows, of which 114 came from the 24 genuine hits.

Across cache lifetimes, 897,106 output-row references remained at destruction;
the largest final history cache held 406,382 output rows in 176 entries.
These are lifetime sums / one final snapshot, **not simultaneous peak live
allocation counts**, and some objects are also shared by reached states.
The measured output-vector capacity sum was 7,176,848 bytes; that excludes Arc
control blocks, proposition objects, their child allocations and shared inputs,
and is not a process-memory estimate.

The instrumented retirement and append sites had identical copy-on-write
counts with caching on and off: 6 ambient retirement detachments, 6 shared
retirement detachments, 756 append detachments, and 1,493,955 copied row handles.
No additional COW cost was demonstrated at those sites. Other mutation sites
were not separately counted.

## Timed costs

A second diagnostic version explicitly times the bypass path's unused temporary
result destruction, avoiding the mistake of calling every cached destructor
cost newly introduced overhead. Two final cache-on and two bypass observations:

| Cached-path phase | Observed ms per source-replay interval |
| --- | ---: |
| Map search plus allocation-identity checks | 1.28–1.29 |
| Capture fixed bindings / formals and input reference | 0.14–0.16 |
| Wrap transformed rows in Arc objects | 97.67–106.17 |
| Replace old cached outputs / input | 45.79–48.31 |
| Destroy final caches | 19.08–19.97 |

This is **gross phase attribution**, not an additive estimate of net slowdown.
The bypass already paid for temporary result destruction (8.16–8.69 ms) and
its append interval, including needed Arc allocation (42.50–67.88 ms).
Cached append took 8.87–9.70 ms, with wrapping moved into the separate bucket.
Map insertion is not isolated from its surrounding control flow. Counts,
formatting and clock overhead are excluded from the primary comparisons, and
individual stopwatch intervals still include host scheduling effects.

Transformation time remained 210.49–214.60 ms with the cache, versus
220.79–287.98 ms in the bypass observations: almost the same number of
transformations was still necessary. These different-run intervals cannot
establish the CPU cost of the 24 skipped transformations. Full disjoint query
metrics, cleanup metrics and per-run identities are in [summary.json](summary.json).

## End-to-end controls and limits

The same diagnostic binary has a temporary `OMEGA_EDGE_CACHE=off` bypass using
the original substitution/containment/append path, with all invalidation,
normalization, joins and proof obligations preserved. `on` replays the rejected
candidate. Ordinary A/B runs set `OMEGA_EDGE_REBIND_PROFILE=off`; detailed runs
use `counts` or `timing`. Both use the same outer attribution hooks. No mode is
retained in production.

| Unprofiled pair, order | Cache off replay, ms | Cache on replay, ms |
| --- | ---: | ---: |
| A, off then on | 1331.70 | 1395.80 |
| B, on then off | 1379.81 | 1376.39 |
| C, off then on | 3158.72 | 2116.46 |

The last pair had much higher compile CPU/wall times than the first two; it is
retained, not discarded. This shared-host spread prevents a precise universal
net-overhead or speedup claim. The causal explanation rests on deterministic
reuse/allocation counts and directly observed maintenance costs. Fresh frozen
original-baseline and rejected-candidate observations are also retained as
calibration, without presenting their single pair as a new speedup.

Peak process commit ranged 3283–3373 MiB for cache-on primary observations and
3321–3368 MiB for cache-off. No stable peak-memory difference is established.
The harness records commit charge independently from working set using the
already-declared Win32 structure. Its 64 MiB allocation sanity check observed
73,486,336 bytes peak commit and 79,552,512 bytes peak working set.
[Research](research.md) links the exact Microsoft field definitions.

Both diagnostic builds passed 23 selected payload kernel tests (6 skipped).
All 18 compiler observations succeeded; every emitted native image retained
SHA-256 `3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`
and the authored EOF outcome. Separate native/Terminal execution agreed on exit
0 and all 463 stdout bytes. Two count runs and all four final timing runs
reproduced the same logical work and COW counts. These checks cover this
program; no full corpus, invalid-source qualification, other host or interactive
Dungeon route was run. All compiler source edits and temporary hooks were removed.

## Reproduction

Base `6f7ebcd23c1` contains only prior evidence beyond the original semantic base
`456c5e984452a45d977295e254b3ca9ae3cc7750`. The unchanged 203 authored source
files are checked against the [parent input manifest](../input-manifest.json).
Every compile case uses an unused build directory, a fresh private
`LOCALAPPDATA` cache, explicit library root, general phase profiler off, an
optimized exact frozen compiler, and that compiler's ordinarily accepted lock.
The public `--both` runtime comparison uses the shared cache and is a separate
correctness observation, not one of the cold-cache timing comparisons.

[instrumentation.patch](instrumentation.patch) reproduces the first diagnostic
producer; [cost-instrumentation.patch](cost-instrumentation.patch) is the
alternative adding explicit bypass temporary-result destruction timing. Apply
exactly one, then use the existing one-hold `mbx nextest ... payload` / optimized
CLI build chain. [identities.json](identities.json) binds the frozen producers
and accepted locks; normal package update logs show zero changed source entries.
Binaries and accepted locks remain local under ignored `build/proof-profile`.
Both patches are checked against separate scratch indices at the recorded base.

The durable harness uses root discovery so it can run from this record folder:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/cache_diagnostics/analyze.py
python wiki/drafts/measurements/proof_parallelism_2026_10_28/cache_diagnostics/parser_selfcheck.py
```

`cache_diagnostics_run.py` provides `run(cache, mode, unused_label, producer=...)`
and the original case schedule; `cache_cost_run.py` provides the reversed final
timing pairs. Retained producers are named `edge-cache-probe` and
`edge-cache-cost-probe`. Existing result labels are refused. The raw-stream
parser accepts plain or gzip stderr. Large exact streams and build logs are
losslessly compressed; [manifest.json](manifest.json) binds every retained byte.

Decision: retain the diagnosis and the rejection. **The expensive key was not
the problem: reuse opportunity was negligible, and eager output wrapping added
millions of allocations.** This pass tries no replacement cache or optimization.
