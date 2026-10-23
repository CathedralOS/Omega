# Direct worker publication

Baseline: `e6763695cf69d3c388a650795c0662093fbd936f`. Retain final-slot worker ownership and inline single-page metadata for measured allocation/data-movement reduction with preserved outputs and ownership.

## Ownership and data movement

The fused parser previously returned a large syntax forest in each worker's Result, converted the collected Results to page records, then moved the forests into a second vector while flattening source metadata. The new owner validates the complete nonzero page-generation interval, allocates the final forest slots, and gives each worker its disjoint slot and unchanged source chunk. Workers return only source/error metadata. A single-page batch retains that metadata inline and still executes on the dedicated parser pool's configured stack.

This preserves source order, generated-unit separation, page/handle identity, retained source ownership, lexical-before-parse diagnostic priority and lexing after a parse failure. Empty batches consume no page identity. No unsafe code, security relaxation or grammar change is introduced. Serial empty-forest construction remains a measured cost.

All 289 complete project assemblies preserve normalized stored-forest, source/custody and error output in both ownership modes:

| Ownership mode | Allocation requests | Cumulative requested bytes |
| --- | ---: | ---: |
| unique | 4,762,324 → 4,761,827 | 2,044,117,753 → 1,974,876,697 |
| shared | 4,820,855 → 4,820,358 | 2,050,420,780 → 1,981,179,724 |

Each mode removes 497 allocation requests and 69,241,056 requested bytes. No project increases either measure. These modes are separate comparisons, not additive savings. The all-worker mimalloc instrument counts requests including reallocations; this is neither RSS nor retained/peak memory. Its instrumented elapsed time is not used for CPU timing claims.

The first direct-slot draft removed large transfers but added one small allocation and 1,032 requested bytes on seven early-refusal projects. Reserving the known metadata count removed the excess bytes; inline single-page metadata then removed the extra allocation and additional common-path requests. Both predecessors and their raw evidence remain archived. The final figures describe only the selected implementation.

## Repetition timing and machine code

Four alternating paired rounds cover the 456-source append consumer, the same corpus as 456 independent units, one valid nested-source input, and one rejected source input. Retained-source preparation/cloning is outside timing; lexing, parsing, publication, append adoption where applicable, and result destruction are inside. Each run seeks three seconds without a new minimum with a 15-second cap. All valid samples, quiet-window status and whole-process resource observations remain in the archive. The independent-unit workload deliberately stresses that route; it is not a claim about typical generated-unit counts.

| Workload / producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| append / baseline | 7.135375 | 8.868771 | 9.084933 | 37.039291 | 2,402 |
| append / candidate | 7.104209 | 8.884125 | 9.155873 | 45.966625 | 1,947 |
| units / baseline | 8.727292 | 11.730291 | 13.127707 | 65.906459 | 1,338 |
| units / candidate | 8.406125 | 11.535250 | 12.437279 | 57.595667 | 1,966 |
| single-valid / baseline | 0.018833 | 0.044416 | 0.057138 | 10.570791 | 328,671 |
| single-valid / candidate | 0.016166 | 0.037750 | 0.045936 | 12.219500 | 323,988 |
| single-invalid / baseline | 0.007667 | 0.019208 | 0.021072 | 10.022750 | 835,816 |
| single-invalid / candidate | 0.006000 | 0.015000 | 0.018102 | 10.396209 | 773,297 |

All32 runs reached their quiet window. On the final456-source append comparison, minima and paired minima are mixed and pooled median/mean are essentially neutral; no reliable append-latency gain is established. Independent-unit minima favor the candidate in three of four rounds, with lower pooled median/mean. The valid and rejected single-source paths have lower candidate minima in all four rounds and lower pooled median/mean. These are scoped parser-consumer observations under continuing host activity, not a whole-compilation speedup or a universal bound. Retention is supported by removed work/storage, with the single-source timing corroborating that mechanism.

Frozen optimized machine code shows the old worker/collector moving 15,584-byte records containing a 15,240-byte forest. Final workers return 344-byte metadata; flattening handles only small metadata/iterator records. Both benchmark wrappers call the real SourceInput methods and drop their observed results before the clock ends. The unchanged page-adoption boundary still has large header copies and remains a separate open experiment. Exact addresses, indirect-symbol resolution and binary/disassembly hashes are recorded in single-page-assembly-audit.md and the code inspection artifacts. Standalone probes use opt-level1/debug0; the CLI retains normal workspace package-specific optimization settings.

## Behavior, integration and limits

Seventy direct-owner comparisons preserve complete normalized forests, parsed-source metadata, exact diagnostics, page identities, and source Arc counts before retention and after destruction. They cover empty batches, invalid/overflowing page intervals, ordinary and independent units, and lexical/parse-error ordering, including shared source chunks. Existing worker tests and normal scoped Clippy pass. An additional strict-Clippy attempt stopped at an unchanged optimization dependency's useless conversion; the same dependency object and failure were observed on the baseline. The normal scoped check remains the required check; no lint suppression was added.

Current engine observations use optimized CLI producer `963aa6b78163cbb930df99f083c6dca4a0dfc436` against integrated main base `b1493daf54dddd0257cac96412eda8df0b5b669f`. Complete declared producer source objects match the integrated candidate exactly; any reused executable retains its original build revision and receipt. It runs the nested length witness and unchanged length/local with native exit 0 and interpreter exit 0. The invalid count rejects at `count > 0 && count <= items.len`. Integrated worker and package-compilation tests and scoped Clippy pass;30 acknowledgement-related compiler roots match baseline exits and output exactly. Pinned qualification captures current source/artifact identities and actual engine observations. Historical allocation/timing probes remain observations of the stated original baseline and selected implementation, not relabeled integrated timing.


The thirty-root CLI comparison preserves baseline output; it is not thirty passing fixtures. Four roots accept and 26 refuse, including fifteen `pass` roots refused by program-entry binding admission. Actual native/Terminal acceptance comes from the separate runnable nested-length and unchanged length/local witnesses, with the invalid count still refusing at its required fact.

## Replay and remaining work

Full inputs/hashes, patches, frozen binaries, complete owner and project observations, raw timing samples, prior variants, disassembly, producer receipts and qualification history are retained in the primary checkout's `build/measurement-archives/worker-publication-2026-10-22` after archival. The working experiment is `build/worker-publication`: setup.py pins matching probes; measure.py and refine.py retain the initial/capacity variants; single_refinement.py checks the final variant, pauses for internal code inspection and runs time_single_refinement.py; integrate.py performs qualification, exact-SHA verification and the normal landing protocol. One-shot mutation scripts must start from their declared baseline, not be rerun over a selected candidate.

Layer 0 remains open. Page adoption, compressed integer spelling, remaining identifier/expression geometry, constructor paths, grammar retries, source-root associations and source/path ownership still have explicit questions in the design inventory. This change does not claim design saturation.
