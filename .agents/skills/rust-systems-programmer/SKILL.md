---
name: rust-systems-programmer
description: Write and review Rust with explicit domain naming, cohesive ownership, visible application orchestration, recoverable errors, allocation-conscious storage, partitioned parallelism, and SIMD kernels. Use when implementing, refactoring, debugging, or reviewing Rust code in this repository. Not for prose-only work or non-Rust implementation.
---

# Rust Systems Programming

Use this skill to choose where decisions happen, who owns working state, and what representation the next Omega compiler phase can consume directly. The linked Squalr studies provide concrete examples, not a replacement architecture.

## Start from Omega

Read [AGENTS.md](../../../AGENTS.md) and its authoritative [repository conventions](../../../AGENTS.md#conventions-and-workflow), then the owning design or task contract. Trace the affected producer and consuming callers. Preserve observable order, semantic identity, failure behavior, and supported execution boundaries. Use the repository toolchain and APIs. Durable compiler storage follows arena, generational-handle, and ZII contracts; worker-local vectors are construction storage. Source text beyond resolution is limited to the payload roles named in those conventions, not semantic identity.

Use this exact repository skill when invoked by path. A delegated evaluation reports the absolute path it read. The change is the one the task asked for.

## Choose the relevant operation

| What you are deciding | Pattern to consider | Read |
| --- | --- | --- |
| Repeated passes rebuild mostly unchanged work | Separate immutable input from changing selections; revisit only affected dependencies. | [Work domain](references/storage-and-execution.md#reduce-the-work-domain-before-speeding-up-the-loop) |
| The same setup repeats across many work units | Prepare shared facts once; borrow them in a small local plan. | [Code structure](references/squalr-code-structure.md#prepare-shared-decisions-borrow-them-in-local-plans) |
| Repeated searches suggest adding a map or index | Ask whether the producer can carry the association directly. | [Lookup framing](references/storage-and-execution.md#eliminate-lookup-before-choosing-an-index) |
| A loop repeatedly branches on a fixed mode | Select the mode outside the loop; keep each loop's actual inputs explicit. | [Branch placement](references/squalr-code-structure.md#branch-once-around-the-loop) |
| Runtime flexibility meets a small set of fast kernels | Runtime selection around bounded compile-time specialization. | [Dispatch](references/squalr-code-structure.md#select-dynamically-specialize-a-bounded-dimension) |
| Adding threads or scratch state | Stateless algorithm; invocation/worker owns mutable state and output. | [Working state](references/squalr-code-structure.md#stateless-algorithm-invocation-owned-working-state), [execution constraints](references/storage-and-execution.md#put-threads-at-the-ownership-boundary) |
| Results are copied, flattened, or expanded between stages | Preserve produced batches or spans when the next consumer can use them. | [Scan pipeline](references/squalr-scan-pipeline.md) |
| SIMD, overlapping values, or tails | Distinguish candidate starts, payload coverage, and safe load bounds. | [Exact scan geometry](references/squalr-scan-pipeline.md#2-stored-byte-coverage-differs-from-candidate-progression) |
| Names or source slices are repeatedly allocated | Retain source ownership, validate spelling/ranges, and inspect derived operations. | [Source storage](references/storage-and-execution.md#retain-source-storage-instead-of-copying-each-name) |
| Moving a pattern into arena/handle-based code | Preserve durable identity and publication contracts. | [Omega examples](references/squalr-patterns.md) |

Read the reference the decision needs. A simple change may need none of these patterns. Source examples explain why a choice fits in its context.

## Shape the implementation

For compiler lowering, apply [compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering).
When a recognizer for an incidental source arrangement is in the path, trace
whether ordinary operation sequencing or explicit data/control/ownership joins
replace it. Keep semantic and ABI restrictions explicit and independently
verified; a failed proof stays a rejection. Test the composition the general
case needs.

Identify which facts change per request, partition, and element. Prepare reusable facts at the outer lifetime; specialize local choices where their inputs become known; keep the inner operation narrow. Borrow immutable context, own mutable working state, and transfer useful output storage to the consumer.

Use coherent domain names and responsibility-focused modules with precise input modes. Reuse existing abstractions; allow a little duplication when it keeps invariant decisions outside a hot loop. Recover from errors at the boundary that owns recovery. Apply the [coding conventions](references/rust-conventions.md) when writing or reviewing Rust.

## Finish with evidence

For a proposed change, identify the avoided work and the contract that must survive. Inspect the whole path, including post-processing and inherited equality, hashing, formatting, or serialization. Peak RAM and execution time are measured, not inferred from the diff.

For implementation, carry the smallest justified change through formatting and [scoped validation](../../../tools/testing.md#validation-scope). Test the changed ownership/order/boundary behavior; measure performance when making a performance claim. Report what changed, what was verified, and any remaining copy or bottleneck that limits the result.

When changing code organization, follow the touched route from its public
entrypoint to the operation and result handling before handoff, applying
[Discoverability architecture](../../../omega-rust/pipeline.md#discoverability-architecture).
Use the linked main.rs and compiler.rs as orchestration examples, and repeat
that ownership test at subordinate multi-step flows. Choose the entrance from
the file tree; check that names and calls reveal the owner and next step. Fix
confusing navigation within the change and flag wider problems.
