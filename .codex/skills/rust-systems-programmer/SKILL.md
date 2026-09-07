---
name: rust-systems-programmer
description: Write and review Rust with explicit domain naming, type-focused modules, thin application adapters, recoverable errors, allocation-conscious storage, partitioned parallelism, and SIMD kernels. Use when implementing, refactoring, debugging, or reviewing Rust code, especially libraries, engines, and platform integrations.
---

# Rust Systems Programming

Use this skill as an interface to Squalr's implementation patterns: choose where decisions happen, who owns working state, and what representation the next consumer can use directly. Transfer the relationships, not Squalr's crate layout or every implementation detail.

## Start from the destination

Read its README, agent instructions, and current-task document when present. Trace the affected producer and consuming callers. Preserve observable order, semantic identity, failure behavior, and supported execution boundaries. Follow the destination's toolchain and existing APIs.

Use this exact repository skill when invoked by path. A delegated evaluation should report the absolute path it read. Do not create a framework or refactor unrelated code just to demonstrate the skill.

## Choose the relevant operation

| What you are deciding | Pattern to consider | Read |
| --- | --- | --- |
| The same setup repeats across many work units | Prepare shared facts once; borrow them in a small local plan. | [Code structure](references/squalr-code-structure.md#prepare-shared-decisions-borrow-them-in-local-plans) |
| A loop repeatedly branches on a fixed mode | Select the mode outside the loop; keep each loop's actual inputs explicit. | [Branch placement](references/squalr-code-structure.md#branch-once-around-the-loop) |
| Runtime flexibility meets a small set of fast kernels | Runtime selection around bounded compile-time specialization. | [Dispatch](references/squalr-code-structure.md#select-dynamically-specialize-a-bounded-dimension) |
| Adding threads or scratch state | Stateless algorithm; invocation/worker owns mutable state and output. | [Working state](references/squalr-code-structure.md#stateless-algorithm-invocation-owned-working-state), [execution constraints](references/storage-and-execution.md#put-threads-at-the-ownership-boundary) |
| Results are copied, flattened, or expanded between stages | Preserve produced batches or spans when the next consumer can use them. | [Scan pipeline](references/squalr-scan-pipeline.md) |
| SIMD, overlapping values, or tails | Distinguish candidate starts, payload coverage, and safe load bounds. | [Exact scan geometry](references/squalr-scan-pipeline.md#2-stored-byte-coverage-differs-from-candidate-progression) |
| Names or source slices are repeatedly allocated | Retain source ownership, validate spelling/ranges, and inspect derived operations. | [Source storage](references/storage-and-execution.md#retain-source-storage-instead-of-copying-each-name) |
| Moving a pattern into arena/handle-based code | Preserve durable identity and publication contracts. | [Omega examples](references/squalr-patterns.md) |

Read the relevant reference, not every reference. A simple change may need none of these patterns. Source examples explain why a choice fits; they do not establish a speedup or make that choice universal.

## Shape the implementation

Identify which facts change per request, partition, and element. Prepare reusable facts at the outer lifetime; specialize local choices where their inputs become known; keep the inner operation narrow. Borrow immutable context, own mutable working state, and transfer useful output storage to the consumer.

Use coherent domain names and responsibility-focused modules. Preserve precise input modes instead of growing an all-purpose context. Reuse existing abstractions; allow a little duplication when it keeps invariant decisions outside a hot loop. Recover from errors at the boundary that owns recovery. Apply the [coding conventions](references/rust-conventions.md) when writing or reviewing Rust.

## Finish with evidence

For a proposed change, identify the avoided work and the contract that must survive. Inspect the whole path, including post-processing and inherited equality, hashing, formatting, or serialization. A smaller source diff or fewer allocations at one stage does not prove lower peak RAM or faster execution.

For implementation, carry the smallest justified change through formatting and focused regression checks. Test the changed ownership/order/boundary behavior; measure performance when making a performance claim. Report what changed, what was verified, and any remaining copy or bottleneck that limits the result. A justified no-change decision is valid; generic advice is not a substitute for an authorized implementation.
