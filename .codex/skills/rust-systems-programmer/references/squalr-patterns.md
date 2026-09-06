# Squalr source examples

Use this reference when selecting parallel work boundaries, result storage, or SIMD organization. These are source-backed examples, not benchmark results or a claim that every current implementation is optimal.

The inspected checkout is `Y:/Development/Friends-and-Family/Zac/Squalr`, revision `e6538c99b2960f35a44727d09acf5f58ef2284ab`. Paths below are relative to that root. Search by symbol because line numbers drift. If the checkout is unavailable, use these explanations and the destination code; do not require this Windows path or invent current source observations. A caller-provided Squalr checkout can substitute after checking its revision.

## Independent output batches

Read together:

- `squalr-engine-scanning/src/element_scans/element_scanner.rs`: `scan_snapshot_with_region_refresh`.
- `squalr-engine-scanning/src/scanners/element_scan_dispatcher.rs`: `dispatch_scan` and `dispatch_scan_for_snapshot_filter_collection`.
- `squalr-engine-api/src/structures/scanning/filters/snapshot_region_filter_collection.rs`: `snapshot_region_filters`, `new_with_result_size`, and `iter`.

The region loop owns each mutable snapshot region. Dispatch maps independent filters to owned vectors of result spans and collects those vectors into a per-type collection. That collection retains `Vec<Vec<SnapshotRegionFilter>>`. The nested layout preserves separately produced payload allocations without a global shared append buffer.

The current hierarchy has more than two levels: snapshot regions, per-type collections, then batches of spans. Do not describe every outer vector as a snapshot region. The constructor still filters, sorts inner and outer vectors, and counts results; constraint processing also contains flattening. Preserving batches avoids one kind of consolidation, not all serial work. Trace the exact path before asserting where a bottleneck exists.

Transfer: let workers produce independently owned output in a form the consumer can use. In a destination requiring durable arena handles, temporary batch vectors may feed an existing arena publication boundary. Preserve handle identity and ordering; do not replace arena-backed child ranges with nested vectors solely to imitate this example.

## SIMD comparison and compact encoding

Read together:

- `squalr-engine-scanning/src/scanners/vector/scanner_vector_aligned.rs`: `encode_results`, `encode_remainder_results`, and `scan_region`.
- `squalr-engine-scanning/src/scanners/structures/snapshot_region_filter_run_length_encoder.rs`.
- `squalr-engine-scanning/src/scanners/element_scan_dispatcher.rs`: `perform_debug_scan` and its call site.

The aligned kernel consumes contiguous current/previous bytes using a vectorization plan. An all-true comparison encodes a whole range, an all-false comparison finalizes the current run, and mixed masks examine lanes. Results describe spans instead of allocating a rich result object per match. Full-vector work and remainders are distinct, and optional debug validation compares specialized output against a scalar scan.

Transfer: align the input layout, vector operation, and result encoding. A SIMD primitive surrounded by per-lane allocation or repacking may lose its advantage. Read the pointer-producing code and vectorization plan before reusing unsafe loads; existing debug assertions are not a substitute for release-mode safety validation.

## Deferred result materialization

Read `squalr-engine-api/src/structures/results/snapshot_region_scan_results.rs`: `ScanResultsPageCursor`, `get_scan_results_page`, and `build_scan_result`.

The storage remains spans. Page cursors traverse collections in address order and materialize values/display strings for requested results. The page API has real ordering and deletion semantics. Those semantics explain why a consumer may need coordination even when production is parallel.

Transfer: keep computation storage compact and make presentation pay for the page it requests. Never remove sorting or identity mapping before tracing callers' observable ordering requirements.

## Repeated snapshot storage

Read the README's Snapshot System section and `squalr-engine-api/src/structures/snapshots/snapshot_region.rs` for current/previous byte buffers. Locate their refresh writers before changing reuse behavior; the data holder alone does not establish allocation frequency.

The README describes alternating current/previous storage so recurring scans can reuse allocations after initialization. Transfer the buffer-lifetime design while accounting for input growth, invalid reads, and retained memory. Verify the actual refresh route before claiming allocation-free steady state.
