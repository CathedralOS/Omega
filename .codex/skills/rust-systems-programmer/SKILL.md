---
name: rust-systems-programmer
description: Write and review Rust with explicit domain naming, type-focused modules, thin application adapters, recoverable errors, allocation-conscious storage, partitioned parallelism, and SIMD kernels. Use when implementing, refactoring, debugging, or reviewing Rust code, especially libraries, engines, and platform integrations.
---

# Rust Systems Programming

Apply a readable, explicit systems-programming style derived from Squalr. Transfer the programming habits, not its crate names, scanner architecture, dependencies, or release workflow. This skill is self-contained and can be copied into another repository.

When this skill is invoked by a repository path, use that exact file rather than a same-named global skill. In a delegated evaluation, identify the absolute skill path actually read so the coordinator can verify provenance.

## Establish the local contract

Read the destination's README and applicable agent instructions first, then its current-task document if one exists. Inspect Cargo manifests, toolchain and formatter configuration, and neighboring implementations before editing. Trace the affected callers and implementations; reuse the existing domain types and shared execution path.

The conventions below guide new code. Explicit destination requirements take precedence. Preserve established public APIs and avoid unrelated renaming or restructuring just to apply this skill. Do not introduce a workspace, command bus, plugin system, nightly toolchain, or dependency merely because Squalr uses one.

## Apply the pattern to the destination

Before proposing a storage or threading change, trace input ownership, work scheduling, output production, and the consuming callers. Establish the contract that survives the change: result order, identity/handles, error and cancellation behavior, and peak memory. Decide whether the storage is temporary scratch or a durable representation. Choose a compatible layout before changing code.

For Omega, consult its README and AGENTS.md: durable lowered child lists use arenas, `Handle<T>`, and `HandleSpan<T>`; temporary worker buffers may use vectors. Squalr's partitioned ownership principle can support that design without replacing it. Arena publication may be required work. Paged storage alone does not make concurrent mutation safe; inspect the arena's actual API and ownership contract.

For concrete examples, read [Squalr source patterns](references/squalr-patterns.md) when working on storage, parallelism, or SIMD. Follow the named producer and consumer symbols in the actual checkout when available. Examples describe tradeoffs, not mandatory dependencies or universal layouts.

## Design storage and parallel execution together

Choose data structures by how work is partitioned, written, and consumed. Allocation behavior, memory traffic, and serial work are architectural concerns, not cleanup after the algorithm is finished. Preserve layouts that let independent workers produce useful final storage directly.

### Preserve the shape produced by parallel work

A `Vec<Vec<ResultSpan>>` can be the right final representation: each independently processed region or partition owns its result spans. Workers build separate inner vectors without a shared append lock. Collecting those vectors preserves their allocations; flattening into a newly allocated vector moves every span and adds a consolidation phase to the critical path.

Do not flatten, globally sort, or rebuild partitioned output merely to make the model look cleaner. Preserve required deterministic ordering: completion order is not source order, and removing a sort must preserve the caller-visible contract through another valid strategy. Consumers can traverse nested storage through iterators, or materialize only the requested page. A wrapper struct that retains the partitioned storage is fine; the cost comes from reorganizing the payload, not from naming its container.

Squalr illustrates this at multiple levels: snapshots contain regions; region results contain per-type filter collections; each collection retains `Vec<Vec<SnapshotRegionFilter>>` produced by scan work. The reusable principle is independently owned batches of compact spans, not a mandatory two-level schema. Nested vectors have headers, capacity slack, and separate allocations; assess those costs against the avoided copies, contention, and consolidation. Change the layout when an actual consumer or measured workload justifies the total cost.

### Put threads at the ownership boundary

Parallelize substantial independent regions or batches, with each task reading its input and owning its output. Keep inner scalar/SIMD loops free of locks, per-element task scheduling, and shared result appends. Choose task granularity that amortizes scheduling while leaving enough independent work to balance workers.

Reuse the project's worker pool. Trace work invoked inside each worker, including nested thread creation and configured stack reservations. Distinguish reserved address space from committed/resident memory. Account for nested parallel work, available cores, and memory bandwidth; more tasks do not automatically mean more throughput. Keep a sequential path for small workloads. Aggregate progress and counters at useful batch boundaries rather than contending on every result.

Inspect the entire path after the parallel loop: merging, sorting, counting, serialization, and publication can become the serial bottleneck. Preserve partitions through subsequent stages where possible, and reduce only the metadata that actually needs aggregation.

### Minimize allocations and repeated work

Reuse input, output, and scratch buffers across repeated operations when ownership permits. Retain useful capacity, reserve from realistic size information, and avoid allocating an object per match. Balance reuse against retained RAM; do not reserve worst-case capacity for every worker without a reason.

Store spans, ranges, or offsets when they represent many results compactly. Decode values and construct display objects on demand. Separate I/O from computation, write directly into the intended backing storage where practical, and avoid intermediate copies or repeated conversions. Keep repeatedly accessed bytes contiguous within each work unit. Choose a layout that serves the actual access pattern instead of imposing one universal container shape.

### Make SIMD part of the kernel design

Use existing vector kernels for batch comparisons and other suitable hot operations. Organize contiguous input, alignment rules, comparison dispatch, and output encoding so SIMD does useful work without per-lane allocation or expensive repacking. Hoist invariant decisions out of the inner loop.

Handle full vectors and tails explicitly, preserving bounds and scalar semantics. When common in the workload, process all-match and no-match masks as whole spans before examining individual lanes. Keep scalar/reference behavior for correctness comparisons, and select supported kernels at the appropriate dispatch boundary.

Validate empty input, tails, alignment, overflow, and dense/sparse results. Measure end-to-end time, allocation volume, peak/retained memory, and scaling where relevant, including post-processing. Do not infer allocations per operation from a library API name; inspect the relevant implementation/version or measure allocations. First check whether the suspected overhead matters at the real batch size and work cost; a channel send per expensive compilation has a different cost balance from one send per scanned byte. Preserve demonstrated SIMD and threading gains; do not claim a speedup from a tidier representation or a faster isolated loop alone.

## Name the domain, not the mechanics

- Use coherent, specific names for variables, parameters, fields, closures, and functions. No `i`, `idx`, or bare `index`; use `region_index`, `sample_index`, or `command_index`.
- Replace vague `data`, `temp`, and `flag` with what the value represents: `encoded_bytes`, `previous_sample`, `should_cancel`. Carry domain names through destructuring and match arms.
- Distinguish addresses, offsets, byte counts, element counts, and capacities in names. Avoid exchanging them through ambiguous arguments.
- Use `new` for construction, `get_<property>` for ordinary getters, `set_<property>` for mutation, and action names for work. Use predicates such as `is_empty`, `has_permission`, and `should_cancel`. Preserve a destination's established accessor style rather than creating competing APIs.
- Use descriptive generic names such as `RefreshRegion` when they explain a role. Conventional type parameters are acceptable when their meaning is already clear.

## Organize around responsibilities

Prefer one principal public type per snake_case file: `BufferRegion` in `buffer_region.rs`, its independent error type in `buffer_region_error.rs`. Keep the type's inherent and trait implementations together. Move unrelated structs and services to their own files; small private implementation details need not become modules.

Organize folders by domain, then operation. Use meaningful suffixes such as `_request`, `_response`, `_error`, and `_provider` where those roles actually exist. Keep `mod.rs` focused on declarations, visibility, and platform selection. Put behavior in named implementation files.

Prefer explicit imports for project types; grouping related standard-library or external imports is fine. Follow local formatting, remove unused imports, and do not churn existing imports solely for style. Do not add empty `impl` blocks or pass-through helpers with no responsibility.

## Keep policy above mechanisms

Shared models describe the domain; computation operates on those models; platform adapters perform I/O; application services coordinate them; frontends translate user actions and display results. Fit these responsibilities into existing modules or crates rather than creating a crate for each layer.

- Keep reusable computation usable with caller-owned inputs. A byte-processing function should not require a process handle, application singleton, or UI state merely to run.
- Keep CLI, GUI, and transport handlers thin. Validation and behavior shared by multiple entry points belong in their common execution path.
- Where commands already exist, use typed request and response models, explicit enum dispatch, and existing conversions. Keep serialization models separate from platform handles and executor state. Add derives only when required by their consumers.
- Introduce traits at actual substitution boundaries, such as native backends or external I/O. Do not create an interface for every struct.
- Select OS implementations at the module boundary with `cfg`; callers use the shared contract. Update every supported implementation when changing that contract, including explicit unsupported-operation handling. Keep platform dependencies target-scoped.

## Make ownership and failure visible

Borrow inputs when the operation does not retain them. Prefer `&[T]`, `&str`, and `&mut [T]` when only their contents matter. Return owned results when ownership transfers. Keep invariant-bearing state private; plain request/response records may expose fields.

Use early returns, `let ... else`, and explicit `match` arms to keep success paths readable. Short iterator chains are useful; use a named intermediate or loop when a chain hides state changes or failure handling.

Return `Result` for failure and `Option` for legitimate absence. Use the destination's error convention; prefer domain error variants with actionable context when callers must distinguish failures. Use `thiserror` if already available, otherwise use existing errors or standard error traits. Do not add `anyhow` or erase typed library errors just to shorten signatures.

No `unwrap()`, `expect()`, `panic!()`, or `unreachable!()` in production error paths. Logging and returning is appropriate only at a boundary that owns recovery; never turn a failed operation into apparent success. Handle lock acquisition failures explicitly. Assertions and descriptive `expect` messages are appropriate in tests; debug assertions can document internal invariants, but cannot replace input validation.

For external offsets, sizes, and counts, validate bounds and conversions before indexing, allocating, or doing pointer arithmetic. Use checked arithmetic when overflow is an error. Use saturation only when clamping is the intended domain behavior.

## Keep synchronization and unsafe code local

Choose ownership before synchronization. Use `Arc` only for shared ownership and a lock only for shared mutation; `Arc<RwLock<T>>` is an option, not a default wrapper. Reuse the project's channels and runtime. Use `OnceLock` for required one-time initialization when the supported Rust version permits it, not as a reason to introduce global state.

Keep critical sections small. Release guards before blocking sends, callbacks, I/O, or awaiting when the operation can safely run outside the lock; preserve the operation's consistency requirements. Choose atomic ordering for the actual synchronization contract, not by copying a progress counter elsewhere.

Wrap owned OS resources in RAII guards with `Drop` so early returns release them. Keep unsafe operations in narrow platform or kernel boundaries. Document the concrete safety argument: valid range, alignment, initialization, lifetime, and aliasing as applicable. Do not copy undocumented unsafe code merely because an existing implementation uses it.

## Format and document for the reader

Use the destination's rustfmt configuration. The source style uses vertical multi-parameter signatures and a 160-column limit; adopt these only when establishing a new formatter policy, not by silently changing an existing one. Leave toolchain and edition choices to the destination project.

Write rustdoc for public contracts and non-obvious functions: units, ownership, errors, boundary semantics, and safety requirements. Comments explain intent or constraints and end with a period. Avoid narrating assignments or adding documentation that only repeats a name.

For example, this pure operation keeps domain names, borrows its input, and makes invalid ranges explicit:

```rust
/// Borrows the requested byte range, returning None if the range overflows or exceeds the buffer.
pub fn get_region_bytes(
    buffer_bytes: &[u8],
    region_offset: usize,
    region_byte_count: usize,
) -> Option<&[u8]> {
    let region_end = region_offset.checked_add(region_byte_count)?;
    buffer_bytes.get(region_offset..region_end)
}

#[cfg(test)]
mod tests {
    use super::get_region_bytes;

    #[test]
    fn region_bytes_respects_bounds_and_overflow() {
        let buffer_bytes = [10, 20, 30];
        assert_eq!(get_region_bytes(&buffer_bytes, 1, 2), Some(&buffer_bytes[1..3]));
        assert_eq!(get_region_bytes(&buffer_bytes, 3, 0), Some(&buffer_bytes[3..3]));
        assert_eq!(get_region_bytes(&buffer_bytes, 2, 2), None);
        assert_eq!(get_region_bytes(&buffer_bytes, usize::MAX, 2), None);
    }
}
```

## Verify the changed contract

Test reusable logic near its implementation using `#[cfg(test)]`, and test command/adapter integration through the destination's existing test harness. Prefer deterministic input buffers or mocked I/O over live processes for library tests. Test frontend behavior when the change concerns that frontend; command tests alone do not establish UI behavior.

Cover the changed success path and relevant failure or boundary cases. For a bug fix, leave a regression test that fails on the original behavior. If a test cannot run, state the limitation and what would enable it; never substitute an empty passing test for evidence.

Remove newly unused imports and dead helpers, run formatting and relevant tests/checks using the destination's toolchain, and inspect the diff. Report what was actually verified and any untested platform or runtime behavior. Follow the destination's rules for task notes and commits; this skill does not impose Squalr's session workflow on another repository.

For performance reviews, separate observed source facts, inferred costs, and measured results. A documented tuning override is not a correctness bug merely because it bypasses a default cap; preserve its semantics and propose measurements before changing policy. Recommend the smallest contract-preserving change only when the evidence supports it; a justified no-change decision is valid. Give the source anchors, required regression check, and measurement that would decide between alternatives. Do not rewrite code or invent a speedup merely to demonstrate this skill. For implementation requests, carry justified changes through focused verification instead of stopping at a review.