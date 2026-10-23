# Uniform arena slot identity

Baseline: `a1ad64980f53c002ca7f9d9c83e3c18c8f3ba935`. Selected candidate: implicit uniform metadata with a compact cold owner and a single metadata borrow around clear. This record distinguishes exact storage/behavior observations from timing under host contention. Publication and integrated validation are recorded below when complete.

## Design

An append-only `Arena<T>` already validates handles and spans from one page generation and row bounds. It previously also allocated and filled a generation and occupancy entry for every row. The candidate retains uniform identity in the arena and materializes those vectors only on the first valid retirement. Three cold vector headers move into one optional metadata owner. Payload rows stay contiguous, generational handles keep their full width, and stale slots still reject after reuse.

The first retirement now initializes O(n) metadata. Reset and clone_from retain allocated capacities. Logical freshness remains separate from physical storage mode because a payload operation can unwind after metadata allocation but before retirement. Logical Debug/equality preserve existing observations. Clone-unwind guards preserve the initialized payload prefix. No unsafe code, new language limit or relaxed custody check is introduced. Final ownership review also changed the consuming iterator to keep only occupancy; generation/free-list storage releases at iterator creation as it did before this experiment. The initial compact iterator had retained that unused cold storage until exhaustion. Historical parser/history/CLI timing captures precede this small correction; its focused allocation observation and fresh integrated tests/runtime are recorded separately.

## Storage

| Workload | Metric | Baseline | Candidate | Removed |
| --- | --- | ---: | ---: | ---: |
| 463 original inputs | Allocation/reallocation requests | 205,625 | 178,225 | 27,400 |
| 463 original inputs | Cumulative requested bytes | 63,456,771 | 59,663,523 | 3,793,248 |
| 463 original inputs | Retained requested bytes | 30,745,944 | 28,770,528 | 1,975,416 |
| 1,792 wider inputs | Allocation/reallocation requests | 797,848 | 635,702 | 162,146 |
| 1,792 wider inputs | Cumulative requested bytes | 212,660,373 | 199,495,005 | 13,165,368 |
| 1,792 wider inputs | Retained requested bytes | 104,515,106 | 97,459,298 | 7,055,808 |

The corpora overlap; do not add their totals. These are sums of per-input allocator-request observations, not RSS or one compiler invocation's peak. The inline-header variant has the same heap reductions. Peak requested parser memory was lower on every measured input in both variants (463 original and 1,792 wider); these are separate per-input peaks, not a whole-invocation peak.

On this macOS arm64 toolchain, `Arena<u64>` shrinks from 128 to 64 bytes, `Arena<Row112>` from 232 to 168 bytes, and `SyntaxTrees` from 18,504 to 15,240 bytes. Expression rows remain 112 bytes. The compact header saving is separate from the heap-byte totals above.

All 289 project assemblies match exact normalized output/custody/error digests in both unique and shared source-owner modes. Across the complete workloads:

| Ownership mode | Requests removed | Cumulative requested bytes removed |
| --- | ---: | ---: |
| unique | 540,772 | 129,138,987 |
| shared | 541,583 | 131,342,413 |
These are workload totals; unique and shared modes are separate comparisons, not additive memory savings.

## Timing and tradeoffs

All current timing probes use mimalloc like the CLI, optimized dev code, runtime inputs, black_box observation and inspected machine code. Small histories batch multiple operations per timer interval. Captures retain minima, median, mean, maximum, all samples, order, load and quiet-window status. Timer/sample caps are reported rather than called convergence.

Initial compact whole histories improve append/read/drop, clone/reset and insert/free/reuse minima. For 16,384 rows of 112 bytes, insert/free/reuse/drop measured 87.333 microseconds baseline versus 82.042 microseconds compact. The first free itself pays about 4.8 microseconds to initialize metadata; baseline first-free is below reliable timer resolution in that case, with some zero observations. It is not literally free and no ratio against zero is meaningful.

The first compact clear loop regressed because it repeatedly accessed the optional metadata owner. Borrowing the existing block once around the loop preserves per-slot default/generation/occupancy order. Fresh four-way comparisons measured large whole insert/clear/drop minima 112.875 microseconds baseline, 125.917 original compact, and 106.417 refined compact. First clear remains more expensive than baseline because deferred metadata still needs initialization. Initial whole-history captures apply to the original compact variant; the refined variant has fresh clear and batch measurements. Full numerical distributions and identities are in results.json.

For 456 prelexed sources across 38 pages, six rotating-order rounds measured baseline/inline/compact minima 5.920ms, 4.984ms, 5.106ms. Means were 14.163ms, 12.281ms, 12.801ms and maxima 63.565ms, 168.975ms, 214.220ms. Round winners vary under heavy host contention; these observations do not establish a precise application speedup or improved tail latency.

The first batch harness extracted a successful forest with Result::unwrap, introducing an 18,568-byte baseline/inline or 15,304-byte compact copy. Those valid caller-consumption measurements remain archived separately. The primary harness now checks success and observes/destroys the Result in place. Assembly confirms the post-parse copy is gone in all three producers. Token/source preparation is outside timing; parsing and result destruction are inside.

## Correctness and evidence

Initial arena tests pass 59/59 and scoped Clippy passes; incoming main adds a separate handle-deduplication test, so the corrected integrated arena suite has 60 tests. All 463 original and 1,792 wider parser observations match exactly, including normalized complete forests, refusals and retained source-owner counts. Sixteen focused malformed-input retained forests also match. The parser suite has 271 passes and seven baseline failures with identical diagnostic payloads after normalizing only checkout roots, Rust coordinates and thread IDs; results.json names them. Incoming main renamed 18 wider-corpus inputs with identical Git blobs; paths were mapped and no input was dropped.

The first compact standalone build reused stale inline artifacts because source installation preserved older draft timestamps. Their identical hashes and impossible unchanged layout exposed it before timing acceptance. Those invalid results are archived with the reason. Rebuilding with fresh source timestamps produced distinct binaries and the expected layout; later source installs write fresh timestamps. Invalid outputs never substitute for the valid captures.

Current engine observations use optimized CLI producer `562c46fe479b383e4ec76a9e1b4b8e5268904917` against integrated main base `4e0178247b78546872ce4bfc8da8a38f69c78d17`. Complete declared producer source objects match the integrated candidate exactly; any reused executable retains its original build revision and receipt. It runs the nested length witness and unchanged length/local with native exit 0 and interpreter exit 0. The invalid count rejects at `count > 0 && count <= items.len`. Integrated arena tests, scoped Clippy, facts storage and proof retirement controls pass. Parser tests retain the same seven baseline failures with matching diagnostic payloads. Pinned qualification captures current source/artifact identities and actual engine observations. Historical allocation/timing probes remain observations of the stated original baseline and selected implementation, not relabeled integrated timing.

For a 16,384-row u64 arena with one retired slot, the consuming iterator retains 147456 requested bytes on baseline, 213080 in the initial compact candidate and 147456 after the ownership correction. All payload values match and live requested bytes return to zero after consumption.

Whole-compilation timing uses the actual optimized CLI binaries on the original experimental tree. Wall-time min/median/mean/max milliseconds: baseline 2083.049/2306.260/2330.429/2800.423; candidate 1970.084/2278.881/2422.666/3854.431. Phase distributions, trial counts and raw output are preserved. Concurrent host load limits any causal latency claim; storage reduction is the retained result.


## Reproduction

Raw samples, exact parser observations, workload lists/hashes, frozen producers, source patches, assembly and failed attempts are retained in the primary checkout's `build/measurement-archives/arena-slot-metadata-2026-10-22/` after archival. The active experiment is `build/arena-metadata`. `phase_one.py` compares baseline/inline; `phase_two.py` prepares compact; `repair_compact_build.py` records the freshness repair; `refine_compact_clear.py` and `refine_batch.py` preserve earlier variants and build the selected one. `measure_variants.py`, `measure_histories.py`, `measure_clear_refinement.py`, `phase_three.py` and `measure_compile.py` separate parser, primitive histories, full assembly and whole compilation. One-shot archive scripts require fresh experiment paths rather than overwriting prior captures.

Layer 0 remains open: source-path/buffer ownership, integer representation, expression geometry/copies, root association and worker publication remain separate questions. This result does not establish absolute optimality or layer closure.
