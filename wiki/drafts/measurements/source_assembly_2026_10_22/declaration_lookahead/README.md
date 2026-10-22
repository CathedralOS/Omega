# Declaration lookahead storage — 2026-10-22

This bounded layer 0 experiment tests whether ambiguous declaration lookahead can
use the caller's arena without changing the grammar. The original 463-file
workload executes **zero** allocating probes. A wider scan of 1,792 existing
source files selected for machine contracts and operator/domain spelling finds
**14 probes in three existing declaration-boundary fixtures**. Those workloads
are separate and overlap; their totals must not be added.

## Mechanism

The parser keeps the actual expression/declaration grammar. A complete fact such
as `operator < limit;` still takes priority over an operator declaration. Both
attempts append only private rows. One caller-forest checkpoint encloses them;
rewind drops those rows and restores root frontiers while retaining capacity.
Previously established handles and source ownership remain valid. A separate
boundary-head query keeps the operational-clause caller token-only.

Removing the disposable forest also retires `Input::retained_by` and the cursor's
extra borrowed source-owner reference. Names still address the same forest-owned
source page. The smaller cursor also reduces the size of unary-prefix scratch
records, which carry an Input for acknowledgement prefixes. This explains a
requested-byte reduction on files that never execute declaration lookahead.

This reuses the append-transaction research and invariants from
[ingress ownership](../ingress_ownership/research.md): drop rejected payloads,
restore every table/root frontier, preserve published handles, and never roll
back earlier-row edits. No new allocation policy, hashing, unsafe code, or
punctuation approximation is introduced.

## Allocation and exact behavior

| Workload | Requests before → after | Requested bytes before → after |
| --- | ---: | ---: |
| Original 463 files | 209,596 → 209,596 | 64,177,571 → 64,149,091 |
| Wider 1,792 files | 814,891 → 814,673 | 215,731,333 → 215,587,573 |
| 10 diagnostic cases | 919 → 774 | 100,183 → 85,319 |

The main affected positive fixture falls from **859 to 664 allocation requests**;
the two invalid fixtures fall from 164 to 154 and 140 to 127. The full wider scan
removes 218 requests. Aggregate retained requested bytes are unchanged for both
real workloads. One diagnostic case retains extra capacity, so this is not a
claim that arena reuse always reduces retained memory. Requested bytes are
allocator traffic, not RSS. There is no whole-compile memory or speed claim.

All complete normalized syntax observations match in each workload: roots,
all arena rows including detached rows, handles, spelling, source spans and
parser errors. Normalization is the prior retained-syntax oracle: it resolves
source handles to text and excludes owner bookkeeping and mutation revision
counters; it preserves handle identities and detached rows. Debug output does
not report vector capacity; retained capacity is covered by the allocation/live-byte
measurements rather than asserted equal by the syntax comparison.
Baseline branch instrumentation is an allocation-free atomic count, used only
for counts/allocation capture. Timing binaries contain no counter.

## Timing investigation

The first four alternating waves were slower in aggregate for the candidate:
minimum 23.821 → 26.102 ms, median 33.595 → 39.488 ms. Load and per-wave results
changed substantially. This result is retained, not discarded. It triggered a
freshly built baseline and a separate arena-only variant retaining the original
cursor representation. Four order-balanced waves compare those with the combined
candidate on the same 463-file, prelexed parse-and-drop workload.

| Producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| fresh-baseline | 25.785 | 37.995 | 38.628 | 68.159 | 594 |
| candidate | 25.040 | 36.001 | 36.704 | 95.649 | 515 |
| arena-only | 24.612 | 34.966 | 35.226 | 93.552 | 574 |

Each wave runs at least 20 trials, seeking three seconds without a new minimum,
with a 15-second cap. All observations, cap outcomes and load averages are
retained. I/O, lexing, allocation counters and snapshots stay outside the timed
interval. `black_box` keeps produced trees live; disassembly confirms the real
parser call. Own builds and timing are serialized; other host work stays active.

## Validation and limits

The 93 focused parser tests pass, including a new repeated/nested lookahead test
with a non-default page generation, retained handles, source-owner counts,
valid declarations, expression-first facts, and malformed syntax. Scoped Clippy
passes with the existing language-core unreachable-pattern warning.
The full parser suite initially reports 269 passes and seven failures. All seven
fail identically on unchanged baseline, including the missing sample `.gitignore`;
they are not called green or suppressed. See the attribution and diagnostic files.

The allocation baseline is `147c3a05de1b134cb8edea5341b223b33980378a` plus the
retained branch-count patch. The first timing baseline is the frozen float-pass
binary from `a38e680dfe48e488ce73852c8452d8185dad7e1e`; its complete parser dependency
source closure is byte-identical to the new baseline. The second timing baseline
is freshly built at the new base. Source patches, binaries, hashes, full captures,
logs and replay scripts are preserved under `build/declaration-lookahead/` and
archived with this completed iteration. Detailed counts and timing are linked below.

## Decision

Retain arena reuse and the obsolete cursor-owner removal for the measured allocation reduction. Timing is **inconclusive**: the first batch favored baseline, whereas the fresh three-variant comparison favors the candidate in aggregate. Neither is promoted to a speedup claim, and every valid observation is retained. The arena-only variant is an isolation experiment, not another shipped mode. The real-workload allocation reduction is small and localized; there is no claim that layer 0 is saturated.

## Integrated execution

Fresh optimized CLI producer `983e6054eb9f1021fecc14beabdf154f1419541f`, integrated base `b3f1046014e9395180acf20cdb251626e93fb958`, passes the existing declaration-boundary positive and both authored negative diagnostics. A retained scratch variant of the length witness names its count `operator` and splits its contract into two facts, exercising the ambiguous lookahead. It and the unchanged length/local sample execute with native exit 0 and interpreter exit 0. The scratch out-of-range call rejects at its required fact. Pinned qualification captures complete compiler/library/toolchain/fixture inputs and immutable executable and source/lock artifacts. 0 measured parser input files changed during integration; parser evidence reuses byte-identical source/input closures.

[Counts](comparison.json), [affected files](affected-comparison.json), [first timing](timing.json), [variant timing](variant-timing.json), [baseline failures](baseline-failure-attribution.json), [matching diagnostics](baseline-failure-diagnostics.json).
