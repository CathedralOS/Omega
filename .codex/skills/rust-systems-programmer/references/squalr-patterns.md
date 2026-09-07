# Squalr source examples

Use this reference when selecting parallel work boundaries, result storage, or SIMD organization. These are source-backed examples, not benchmark results or a claim that every current implementation is optimal.

The inspected checkout is `Y:/Development/Friends-and-Family/Zac/Squalr`, revision `e6538c99b2960f35a44727d09acf5f58ef2284ab`. Paths below are relative to that root. Search by symbol because line numbers drift. If the checkout is unavailable, use these explanations and the destination code; do not require this Windows path or invent current source observations. A caller-provided Squalr checkout can substitute after checking its revision.

## Squalr scan pipeline

Read [the deep scan-pipeline study](squalr-scan-pipeline.md) for the connected producer/consumer contracts: compressed runs as subsequent work, candidate stride versus byte coverage, vector eligibility and tails, encoder ownership, cursor-based paging, and snapshot buffer/I/O reuse. It includes concrete arithmetic, source anchors, and conditions where each pattern loses.

## Transfer example: Omega

Omega's README and AGENTS require arena-backed durable lowered children through `Handle<T>` and `HandleSpan<T>`; worker-local vectors may be temporary construction buffers. At Omega revision `8fb34c783fa91fd74d3ddddb37bc09e1eff40009`, inspect these repository-relative paths when applying the Squalr pattern:

- `omega-rust/psi/foundation/arena/src/handle_span.rs`: `push_contiguous` requires contiguous positions and one generation.
- `omega-rust/psi/foundation/arena/src/paged_arena.rs`: insertion requires exclusive `&mut self`; nested page storage is not concurrent semantic child-list publication.
- `omega-rust/psi/foundation/arena/src/generational_paged_arena.rs`: concurrent allocation supplies default-valued slots; the documented API does not support insertion of initialized nodes. Check current APIs before assuming that remains true.
- `omega-rust/omega/compiler/compiler/tests/canary_suite.rs`: `run_bounded_canary_jobs` restores source order. The `named_integer_conversion_filesystem_cross_targets_reach_checked_trees` caller in `omega-rust/omega/compiler/compiler/tests/canary_suite/abi_runtime_values_and_strings.rs` zips returned results with input targets. Completion order would misattribute failures.
- `omega-rust/omega/compiler/compiler/src/compiler/execution.rs`: `run_on_compile_thread` requests a 256 MiB stack so recursive work can reach depth guards. Executor reuse must preserve that execution requirement and failure propagation; a default worker pool is not automatically equivalent. The configured stack size is not a measurement of committed or resident memory.

These examples constrain how a change can be made. They do not establish a measured bottleneck or require changing the destination's existing storage or scheduler.

## Omega source ownership and copy boundaries

These examples were verified in Omega at `a76a826cd3dbe998f43dd0e8ce953447d57422ef`. Paths are relative to the Omega repository. Follow the named symbols in the destination checkout; these are bounded implementation examples, not a mandate to use reference counting everywhere.

- `omega-rust/omega/compiler/compiler/src/pipeline/checked_entry.rs`: checkpoints retain the original source-map `Arc` instead of cloning the whole assembled AST just to preserve source custody. `source_assembly.rs` keeps exact source identity checks. Transfer: retain the smallest authority the next phase actually needs.
- `omega-rust/omega/compiler/compiler/src/pipeline/source_assembly/checkpoint.rs`: consuming a single-use checkpoint allows `Arc::unwrap_or_clone` to move uniquely owned syntax storage; explicitly cloned reusable checkpoints retain the shared fallback. Transfer: expose ownership transfer at the API boundary before optimizing the clone implementation. Test unique and reused paths; do not assume an `Arc` is uniquely owned.
- `omega-rust/psi/representations/syntax-trees/src/syntax_trees/names/identifier.rs`: parsed identifiers can already share the file buffer. `omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/name.rs` still materializes an owned `DiagnosticName`. Inspect the whole path before claiming zero-copy compilation.
- `omega-rust/psi/foundation/symbols/src/name.rs`: `from_ref_with_sources` compares the validated source slice with `OwnedSource`'s semantic spelling. Matching names retain `SourceSlice` with shared file ownership; transformed or invalid-source names retain owned spelling. This prevents symbol-table insertion from making another text allocation, while preserving the earlier lowering contract.
- `omega-rust/psi/foundation/symbols/src/table.rs`: initial root/child insertion and extension top-level/child insertion all use that materialization path. `begin_extension` can replace or remove the source map; retaining the file allocation prevents existing names from changing or disappearing. An offset alone would depend on stronger source-map lifetime and replacement guarantees.

The `retained_names_compare_and_format_only_the_selected_spelling` regression catches an easy storage-refactor mistake: deriving equality/debug over the new backing file would compare or print unrelated source text for every symbol. The table's `builder_and_extension_names_borrow_retained_source_bytes` test checks pointer identity through all four insertion routes after removing the map. Invalid bounds, missing files, UTF-8 boundaries, and transformed spellings are covered beside name construction.

Transfer: choose ownership and semantic identity together, then inspect operations implicitly inherited from the storage type. This implementation saves a symbol-name allocation and copy for exact matches, but retains per-name `Arc` traffic and the earlier `DiagnosticName` copy. It does not establish whole-program zero-copy or a measured runtime speedup.
