# Rust coding and verification conventions

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

Cover the changed success path and relevant failure or boundary cases. When changing scheduling or result collection, test deliberately reordered completions with explicit synchronization, preserving result identity/order, complete result delivery, and the existing error or panic propagation contract; do not rely on timing sleeps. For a bug fix, leave a regression test that fails on the original behavior. If a test cannot run, state the limitation and what would enable it; never substitute an empty passing test for evidence.

Remove newly unused imports and dead helpers, run formatting and relevant tests/checks using the destination's toolchain, and inspect the diff. Report what was actually verified and any untested platform or runtime behavior. Follow the destination's rules for task notes and commits; this skill does not impose Squalr's session workflow on another repository.

For performance reviews, separate observed source facts, inferred costs, and measured results. A documented tuning override is not a correctness bug merely because it bypasses a default cap; preserve its semantics and propose measurements before changing policy. Recommend the smallest contract-preserving change only when the evidence supports it; a justified no-change decision is valid. Give the source anchors, required regression check, and measurement that would decide between alternatives. Do not rewrite code or invent a speedup merely to demonstrate this skill. For implementation requests, carry justified changes through focused verification instead of stopping at a review.