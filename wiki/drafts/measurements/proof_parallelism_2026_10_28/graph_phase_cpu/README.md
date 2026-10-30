# Graph omission: CPU before, within and after lowering

**Retain the clean lazy implementation.** In six centered cold blocks, lazy mode
uses less CPU in lowering every time: a 15.11% median reduction, saving 0.684 CPU
seconds at the median. Lowering plus all subsequent work also improves in every
block. The earlier cold CPU warning does not repeat in this targeted comparison;
no measured downstream cost cancels the local saving.

This is evidence for avoided work and a local CPU benefit, not a universal
whole-compilation speedup. Some favorable CPU difference occurs before the graph
can be constructed and cannot be credited to this change. The user's later
steering also makes correctness, simplicity and removed work part of the retention
decision. Earlier experiments keep their original parked/inconclusive decisions.

## What is retained

`ReferenceResultMemberships::for_operation` passes its existing OnceCell graph
provider into the common membership operation. Graph/analysis preparation happens
only when a projected shared-referent backing query actually needs it. The public
graph-supplied API stays unchanged. There is no new cross-sweep cache.

Carrier/mapping/path/extent/origin checks, receiver masking, the current normal
completion cut and all-origin intersections are preserved. Unsupported leaves
still validate. Tests show empty/exclusive/unsupported paths do not force the
provider, malformed mappings still reject, and a real projected shared query
forces it. The clean production file matches SHA-256
`74077c5efa605a5219e5abda4cf975dab17079ea461e24e3bcbea233974febeb`.
All runtime selectors, CPU probes and the temporary Windows module are removed.

## Targeted comparison

The preceding [same-binary experiment](../same_binary_graph/README.md) observed a
2.82% median cold CPU penalty in 5/6 blocks, without locating the CPU. This follow-up
keeps that eager/lazy membership file byte-identical in a new diagnostic executable,
then samples Windows process user/kernel CPU at existing timing-record boundaries.
It uses one executable, one normally accepted lock, the same 203 authored inputs
and Dungeon EOF oracle. Source and diagnostic patches are pinned in [source.json](source.json).

Two count preflights confirm four preparations per mode and three full 202-machine
graphs eager versus zero lazy. All three eager initializations appear within the
actual lowering window. Counts are off in the timing series.

Six four-run cold blocks, three ELLE and three LEEL in seeded shuffled order,
supply 24 timing observations and 12 same-mode controls. Every compiler cache and
output directory is fresh; OS caches remain uncontrolled. Both modes have equal
mean positions within each block. Endpoint-only host accounting replaces the
previous 250 ms memory sampler. No own builds overlap measurements and no user
process, priority or affinity is changed. All valid observations remain; there
are no timing-based retries, exclusions or extra unchanged trials.

## CPU partition and result

For each compiler process:

`total CPU = before lowering + lowering + after lowering`

Recorded checking windows are detail within before-lowering CPU. They include
prerequisite checks as well as the main source check, and are not added again.
All raw CPU endpoints, disjointness and partition closure are checked by the
analyzer. CPU sums all process threads; it is not a serial fraction or instruction
count. Medians below are medians of per-block effects, not ratios of global means.

| Region | Lazy uses less CPU | Median reduction | Median CPU seconds saved |
| --- | --- | --- | --- |
| Whole process | 6/6 blocks | 4.68% | 1.469 |
| Before lowering | 4/6 | 1.01% | 0.219 |
| Lowering | **6/6** | **15.11%** | **0.684** |
| After lowering | 3/6 | 0.33% | 0.016 |
| Lowering and everything after | **6/6** | **10.16%** | **0.719** |

Signed CPU differences in seconds, lazy minus eager; negative values mean less
CPU. These within-block means add exactly across the three disjoint regions.

| Block | Order | Before lowering | Lowering | After lowering | Whole process |
| --- | --- | --- | --- | --- | --- |
| 0 | ELLE | -0.304688 | -0.945312 | -0.125000 | -1.375000 |
| 1 | LEEL | -1.164062 | -0.328125 | -0.070312 | -1.562500 |
| 2 | ELLE | -1.687500 | -0.226562 | +0.039062 | -1.875000 |
| 3 | LEEL | +0.148438 | -0.515625 | -0.179688 | -0.546875 |
| 4 | ELLE | -0.132812 | -1.500000 | +0.070312 | -1.562500 |
| 5 | LEEL | +0.210938 | -0.851562 | +0.109375 | -0.531250 |

Mean whole-process CPU saving is 1.242188 seconds: 0.488281 before lowering,
0.727865 within lowering and 0.026042 afterward. The before-lowering portion
precedes graph construction. It is not a causal saving from laziness. In blocks
3 and 5, before-lowering CPU is worse for lazy, yet the later saving still wins.
After-lowering penalties are mixed and at most 0.109375 seconds in these blocks;
none offsets the corresponding lowering saving.

Whole wall time also improves 6/6 here, with a 4.69% median reduction. That is an
observed diagnostic result, not a clean-production latency guarantee. The host
and observer differ from the earlier series: non-target demand is 15.9-49.8%
of CPU capacity, versus a previously saturated host. Same-mode median variation
is 3.47% process CPU, 2.56% wall and 2.72% lowering CPU. Do not subtract the two
experiments or claim to have assigned the earlier negative pairs' causes.

## Measurement correction retained

The first eager preflight compiled, ran and met the exact image/output/count
oracles, then its parser incorrectly expected an outer Terminal CPU window.
That existing outer timer uses `add_completed`, not the wrapped operation.
The emitted lowering window contains every graph initialization, so accounting
was corrected to use its actual boundaries. The same successful preflight was
postprocessed without a compiler rerun; no observation was discarded. The
[failed postprocessing log](preflight-postprocessing.log.gz) and raw source rows
remain. The correction occurred before any lazy observation or mode comparison.

## Correctness and production qualification

The diagnostic build passed 31 host/membership tests. The retained clean source
passed 24 focused membership/payload tests, rustfmt and crate Clippy (warnings
retained). Fresh four positive fixtures execute with native/Terminal agreement;
three invalid controls reject with baseline-identical reasons. The corpus diff
gates still return **1** for the same existing missing-positive/changed-diagnostic
goldens. No golden was recorded and no diff gate is called green.

All 28 diagnostic Dungeon observations succeed: 24 timings, two count preflights
and two explicit both-engine comparisons. Every one of the 26 ordinary images
was actually executed, and both comparisons execute native and Terminal engines.
A separate fresh both-engine Dungeon qualification also succeeds using the exact
clean production producer. Every native image remains
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`,
with exit 0 and the same 463 stdout bytes.

The diagnostic producer is
`babdabe9df24eec6d812f31e2c8324e45d58ce63d514984714fa2f4da801a6dd`.
The clean exact-source production producer is the preserved
`379a5b851627c37fa53e6bd4f1fb8e6a7b75178c0cb18628b94c94faec0e5c8f`;
its original build base/patch and optimized profile remain explicit. It is reused
because all governing compiler/Cargo/toolchain/library/fixture inputs match,
not relabelled as a newly built executable. [retained-implementation.json](retained-implementation.json)
records these identities and the fresh qualification outcomes. The repository's
pinned qualification runner additionally records an immutable native replay on
the clean source checkpoint, retaining the actual preparatory Terminal observation.

## Replay and limits

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/graph_phase_cpu/analyze.py
```

The analyzer verifies the file manifest, exact diagnostic/pure source and producer
bindings, 28 diagnostic observations, CPU endpoints/partition closure, every block
and repeat, graph counts, fresh production fixture outcomes and the clean Dungeon
both-engine observation. Raw records and compressed streams are retained.
[research.md](research.md) holds the predeclared question, primary API sources,
scope correction and retention rationale. Signed means close; medians need not add.

Diagnostic base: `fb37124e244cf436f99976b0a508ed24cc28d285`; semantic compiler base:
`456c5e984452a45d977295e254b3ca9ae3cc7750`. Accepted locks, compiler/native executables
and caches remain ignored local evidence with recorded digests. Execution replay
needs the recorded producers or an explicitly identified rebuild and fresh labels.

No full corpus, alternate gameplay, other host/target, scheduler trace, universal
zero-regression bound or guaranteed production compile-time percentage is claimed.
Shared main is unchanged; the clean implementation is retained on this branch.
