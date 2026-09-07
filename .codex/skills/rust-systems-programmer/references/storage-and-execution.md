# Storage and execution decisions

## Design storage and parallel execution together

Choose data structures by how work is partitioned, written, and consumed. Allocation behavior, memory traffic, and serial work are architectural concerns, not cleanup after the algorithm is finished. Preserve layouts that let independent workers produce useful final storage directly.

### Start with Squalr's coupled decisions

For performance architecture work, read [the scan pipeline's deep patterns](squalr-scan-pipeline.md). Trace how one representation eliminates work in the next stage, then transfer the applicable relationship:

- Preserve worker-produced spans through filtering and querying; compressing output is more useful when consumers never expand it wholesale.
- Distinguish candidate stride from payload width. Overlapping values need trailing bytes without advancing the candidate cursor by that padding.
- Let selective constraints shrink the next stage's input, and re-evaluate kernel eligibility on the surviving spans. Fragmentation changes both SIMD usefulness and task size.
- Put parallelism around independently owned stateful encoders. Splitting inside a run introduces boundary ownership and stitching work; it needs an explicit benefit.
- Serve ordered pages by merging cursors over existing streams. Advance whole runs when ordering permits, while preserving ties, deletion positions, and global identity.
- Reuse temporal buffers and try the common bulk-I/O path first; retain page-level failure metadata for the fallback. Reduced vector length alone does not prove released RAM.

These are connected design choices, not six mandatory optimizations. For a proposed transfer, identify the producer, consumer, preserved invariant, avoided work, and workload that could make it lose. Do not prescribe SIMD, nested vectors, or more threads from appearance alone.

### Preserve the shape produced by parallel work

A `Vec<Vec<ResultSpan>>` can be the right final representation: each independently processed region or partition owns its result spans. Workers build separate inner vectors without a shared append lock. Collecting those vectors preserves their allocations; flattening into a newly allocated vector moves every span and adds a consolidation phase to the critical path.

Do not flatten, globally sort, or rebuild partitioned output merely to make the model look cleaner. Preserve required deterministic ordering: completion order is not source order, and removing a sort must preserve the caller-visible contract through another valid strategy. Consumers can traverse nested storage through iterators, or materialize only the requested page. A wrapper struct that retains the partitioned storage is fine; the cost comes from reorganizing the payload, not from naming its container.

Squalr illustrates this at multiple levels: snapshots contain regions; region results contain per-type filter collections; each collection retains `Vec<Vec<SnapshotRegionFilter>>` produced by scan work. The reusable principle is independently owned batches of compact spans, not a mandatory two-level schema. Nested vectors have headers, capacity slack, and separate allocations; assess those costs against the avoided copies, contention, and consolidation. Change the layout when an actual consumer or measured workload justifies the total cost.

### Put threads at the ownership boundary

Parallelize substantial independent regions or batches, with each task reading its input and owning its output. Keep inner scalar/SIMD loops free of locks, per-element task scheduling, and shared result appends. Choose task granularity that amortizes scheduling while leaving enough independent work to balance workers.

Reuse an existing executor when its stack size, blocking, thread-affinity, isolation, and failure contracts fit the work; otherwise preserve the required execution boundary. Trace work invoked inside each worker, including nested thread creation and configured stack sizes. Distinguish configured capacity, virtual reservation, and committed/resident memory; the configured size alone does not measure any of the latter. Account for nested parallel work, available cores, and memory bandwidth; more tasks do not automatically mean more throughput. Keep a sequential path for small workloads. Aggregate progress and counters at useful batch boundaries rather than contending on every result.

Inspect the entire path after the parallel loop: merging, sorting, counting, serialization, and publication can become the serial bottleneck. Preserve partitions through subsequent stages where possible, and reduce only the metadata that actually needs aggregation.

### Minimize allocations and repeated work

Reuse input, output, and scratch buffers across repeated operations when ownership permits. Retain useful capacity, reserve from realistic size information, and avoid allocating an object per match. Balance reuse against retained RAM; do not reserve worst-case capacity for every worker without a reason.

Store spans, ranges, or offsets when they represent many results compactly. Decode values and construct display objects on demand. Separate I/O from computation, write directly into the intended backing storage where practical, and avoid intermediate copies or repeated conversions. Keep repeatedly accessed bytes contiguous within each work unit. Choose a layout that serves the actual access pattern instead of imposing one universal container shape.

### Retain source storage instead of copying each name

For unchanged source text, aim for zero per-name text allocations: retain the input buffer once and use borrowed slices or source identities plus byte ranges. Use `&str` tied to the input owner's lifetime when the consumer can borrow. If values must move independently or outlive the source-map container, retain shared buffer ownership and a validated range. An `Arc` clone shares bytes but still costs reference-count traffic; a tiny retained slice can keep a large file alive. Do not spread lifetimes, shared ownership, or source text through phases whose contract calls for semantic handles.

Trace every representation boundary. A source-backed parser can still allocate during lowering and again when building symbols. A source span proves provenance, not that the semantic spelling matches the source: canonical paths, generated names, and rewrites may retain an authored span. Reuse the slice only when its validated bytes equal the required spelling; keep owned storage when spelling differs. Check missing sources, bounds, and UTF-8 boundaries explicitly; an invalid-range accessor returning an empty string is not evidence of a valid empty slice.

When a small value starts retaining a larger buffer, inspect derived equality, hashing, debug formatting, and serialization. These operations must observe the logical value and relevant provenance, not accidentally compare or print the whole backing allocation. Verify backing-storage identity, unchanged spelling/provenance, and owner replacement/drop behavior. Report exactly which copy disappeared; pointer-sharing tests do not establish end-to-end zero-copy, peak-RAM savings, or a speedup. See the [Omega ownership examples](squalr-patterns.md#omega-source-ownership-and-copy-boundaries).

### Make SIMD part of the kernel design

Use existing vector kernels for batch comparisons and other suitable hot operations. Organize contiguous input, alignment rules, comparison dispatch, and output encoding so SIMD does useful work without per-lane allocation or expensive repacking. Hoist invariant decisions out of the inner loop.

Handle full vectors and tails explicitly, preserving bounds and scalar semantics. When common in the workload, process all-match and no-match masks as whole spans before examining individual lanes. Keep scalar/reference behavior for correctness comparisons, and select supported kernels at the appropriate dispatch boundary.

Validate empty input, tails, alignment, overflow, and dense/sparse results. Measure end-to-end time, allocation volume, peak/retained memory, and scaling where relevant, including post-processing. Do not infer allocations per operation from a library API name; inspect the relevant implementation/version or measure allocations. First check whether the suspected overhead matters at the real batch size and work cost; a channel send per expensive compilation has a different cost balance from one send per scanned byte. Preserve demonstrated SIMD and threading gains; do not claim a speedup from a tidier representation or a faster isolated loop alone.
