# Squalr: representations that remove downstream work

Inspected revision: `e6538c99b2960f35a44727d09acf5f58ef2284ab`, at `Y:/Development/Friends-and-Family/Zac/Squalr`. Paths below are relative to that repository. These are source observations and transfer criteria, not measured performance claims. Use symbol names to find moved code.

## 1. A run is both compressed output and the next scan's input

Read `squalr-engine-scanning/src/scanners/vector/scanner_vector_aligned.rs` (`encode_results`) with `squalr-engine-scanning/src/scanners/structures/snapshot_region_filter_run_length_encoder.rs` (`encode_range`, `finalize_current_encode`, `take_result_regions`).

For an N-byte comparison mask, all-true adds N to one run counter; all-false finalizes the pending run and advances N bytes; only mixed masks inspect individual candidate positions. A dense matching region therefore becomes a span rather than one record per match. `take_result_regions` transfers the vector using `mem::take`.

`element_scan_dispatcher.rs` in the same scanners directory then applies each AND constraint to the surviving spans, replacing the previous list and returning early when it becomes empty. It constructs and maps a fresh `SnapshotFilterElementScanPlan` for each surviving span. Filtering changes the actual work domain of the next predicate, including whether a vector kernel remains worthwhile.

Transfer when result runs remain useful inputs. Measure all-match, no-match, clustered, and alternating matches: fragmented output can erase compression gains and create many small tasks. Constraints remain in their authored order here; reordering them requires a separate semantic and cost argument. The dispatcher does collect intermediate flat vectors between constraints, so this is not allocation-free filtering.

## 2. Stored byte coverage differs from candidate progression

Read `squalr-engine-scanning/src/scanners/vector/scanner_vector_overlapping.rs`, the encoder's `finalize_current_encode_with_padding`, and `squalr-engine-api/src/structures/scanning/filters/snapshot_region_filter.rs` (`get_element_count`).

When value width exceeds candidate stride, the final candidate needs trailing payload bytes. The encoder stores run length plus padding, but advances its address by the run length, excluding padding. The consumer subtracts `width.saturating_sub(stride)` before dividing covered bytes by stride. Example: three matching 4-byte values at byte stride 1 need a 6-byte span: `(6 - 3) / 1 = 3` candidates. Advancing the candidate cursor by six would skip possible starts. Two spans can overlap in byte coverage while representing different candidate starts; merging them as ordinary intervals can change the result set.

Transfer: document whether every offset/count denotes candidate starts, payload coverage, or storage capacity. Preserve the producer/consumer arithmetic together. Test one candidate, an overlapping final candidate, adjacent runs, and too-short spans. The collection constructor rejects spans shorter than the logical result width before counting.

## Vector loads are planned in valid-start space

Read `squalr-engine-api/src/structures/data_types/generics/vector_generics.rs` (`plan_vector_scan`), `squalr-engine-api/src/structures/data_types/generics/vectorization_plan.rs`, and `squalr-engine-api/src/structures/scanning/rules/element_scan/built_in_filter_rules/filter_rule_map_scan_type.rs`.

The plan subtracts trailing payload before counting candidate starts. A 64-byte vector scanning 4-byte values at stride 1 needs 67 readable bytes to cover its shifted comparisons, not merely a 64-byte region. Dispatch chooses among eligible widths or scalar; a nominal SIMD-capable type does not make every surviving span vectorizable.

The aligned tail can reread a full vector ending at the valid-byte boundary and consume only its new suffix. For 20 bytes of aligned 4-byte values with a 16-byte vector, loads begin at offsets 0 and 4; the second contributes only mask positions 12 through 15, representing the candidate at byte 16. Overlapping kernels instead use scalar comparisons for remaining candidates. The contracts differ: tail correctness requires both a safe load range and exactly-once candidate consumption. Do not generalize the aligned strategy to arbitrary partial values or plugin mask layouts.

## 3. Own the encoder; parallelize the surrounding work

Read `squalr-engine-scanning/src/element_scans/element_scanner.rs` (`scan_snapshot_with_region_refresh`) and `squalr-engine-scanning/src/scanners/element_scan_dispatcher.rs` (`dispatch_scan`). Regions are mutably independent; collections and filters provide further parallel work on shared input. Single-region/single-collection paths and the explicit single-thread option avoid some parallel dispatch. Each scan produces its own vector of spans; collection storage retains those vectors.

The encoder maintains an open run across comparisons. Its documentation explicitly calls out stitching and small boundary regions as costs of splitting that state across tasks. This is why parallelizing a stateful inner loop is a different decision from parallelizing independent filters. Existing nested Rayon work is not proof of optimal granularity at every workload size.

The exact storage hierarchy is region -> per-type collection -> vectors of filter spans. A `Vec<Vec<_>>` here preserves produced allocations; it does not mean every outer vector is a snapshot region. The collection still sorts inner vectors and the outer vector, then counts results. Removing those steps requires proving the ordering expected by consumers.

## 4. Merge views at query time, with arithmetic skipping

Read `squalr-engine-api/src/structures/results/snapshot_region_scan_results.rs`: `ScanResultsPageCursor`, `get_max_consecutive_results_before`, and `get_scan_results_page`.

Pagination creates one cursor per collection and a min-heap represented by `BinaryHeap<Reverse<(address, collection_index)>>`. It retains scan-time storage and materializes requested result values only for the page. Before the next competing address, a cursor can advance multiple logical results: for positive distance d and stride a, the bound is `(d - 1) / a + 1`, capped by the remaining run. Ties advance one result; the collection index participates in deterministic heap order.

Filtered offsets, deletions, and global result indices are separate concerns. Unselected collections still affect global positions. Replacing this with a selected-only iterator could silently change identity. This is not constant-time random access: reaching a page still traverses relevant streams/runs, and deletion accounting has its own cost. The avoided operation is materializing and globally sorting all individual results.

## 5. Swap temporal storage; recover failed I/O at finer granularity

Read `squalr-engine/src/command_executors/scan/snapshot_value_collector.rs` (`read_snapshot_region_values`). It swaps current/previous buffers and allocates the new current buffer only when empty. For merged regions with page boundaries, it first tries one large read; only failure constructs page slices and retries, recording failed addresses in tombstones. The common path avoids paying page-read setup for every region.

Transfer when alternating snapshots have compatible geometry and partial-failure information survives into consumers. A buffer containing bytes is not sufficient proof that all those bytes were read successfully. Initial snapshots and failure paths still allocate; do not claim allocation-free collection.

Read `squalr-engine-api/src/structures/snapshots/snapshot_region.rs` (`resize_to_filters`) for the counterweight: after filtering it drains prefixes and truncates both buffers to the retained bounding interval. Draining moves bytes; truncation does not guarantee capacity release; holes inside the bounds remain retained. Empty regions drop buffers. Shrinking the future scan domain, reducing live length, and returning RAM to the allocator are different outcomes.

## Applying the study elsewhere

Describe a proposed optimization as a chain: input geometry -> ownership boundary -> kernel output -> next consumer. Name what work disappears and what remains. For Omega, spans and compact handles can transfer; scanning-specific run encodings, result-order rules, and snapshot refresh policy do not transfer automatically. Use the existing [Omega ownership examples](squalr-patterns.md#omega-source-ownership-and-copy-boundaries) for those contracts.
