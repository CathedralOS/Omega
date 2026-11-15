---
name: rust-systems-programmer
description: Write and review Rust with domain names, clear state ownership, visible top-level coordination, recoverable errors, low-allocation storage, parallel work split by ownership, and SIMD kernels. Use when implementing, refactoring, debugging, or reviewing Rust code in this repository. Not for prose-only work or non-Rust implementation.
---

# Rust Systems Programming

Use this skill to decide where decisions happen, which code owns working state, and what data shape the next compiler stage can use directly. The Squalr studies (a Rust memory scanner) give examples, not an architecture to copy.

## Start from Omega

Read [AGENTS.md](../../../AGENTS.md) and its [repository conventions](../../../AGENTS.md#conventions-and-workflow) (arena storage, handles, ZII, source text), which take precedence, then the design page or `TASKS.md` item for the code. Trace the code that produces the affected data and the callers that use it. Keep observable order, semantic identity, failure behavior and execution boundaries unchanged. A worker's local vectors are only scratch while it builds arena data.

When invoked by path, use this repository's copy; an agent evaluating the skill reports the absolute path it read.

## Choose the relevant operation

| What you are deciding | Pattern to consider | Read |
| --- | --- | --- |
| Repeated passes rebuild mostly unchanged work | Separate fixed input from changing selections; revisit only affected dependencies. | [Work domain](references/storage-and-execution.md#reduce-the-work-domain-before-speeding-up-the-loop) |
| The same setup repeats across many work units | Prepare shared facts once; borrow them in a small local plan. | [Code structure](references/squalr-code-structure.md#prepare-shared-decisions-borrow-them-in-local-plans) |
| Repeated searches suggest adding a map or index | Ask whether the producer can carry the association directly. | [Lookup framing](references/storage-and-execution.md#eliminate-lookup-before-choosing-an-index) |
| A loop repeatedly branches on a fixed mode | Select the mode outside the loop; keep each loop's actual inputs explicit. | [Branch placement](references/squalr-code-structure.md#branch-once-around-the-loop) |
| Runtime choice among a few fast kernels | Choose at runtime among a few compile-time specializations. | [Dispatch](references/squalr-code-structure.md#select-dynamically-specialize-a-bounded-dimension) |
| Adding threads or scratch state | Stateless algorithm; each call or worker owns its mutable state and output. | [Working state](references/squalr-code-structure.md#stateless-algorithm-invocation-owned-working-state), [execution constraints](references/storage-and-execution.md#put-threads-at-the-ownership-boundary) |
| Results are copied, flattened, or expanded between stages | Keep produced batches or spans when the consumer can use them. | [Scan pipeline](references/squalr-scan-pipeline.md) |
| SIMD, overlapping values, or tails | Distinguish candidate starts, payload coverage, and safe load bounds. | [Exact scan geometry](references/squalr-scan-pipeline.md#2-stored-byte-coverage-differs-from-candidate-progression) |
| Names or source slices are repeatedly allocated | Keep the source buffer, check spelling and ranges, and inspect derived traits. | [Source storage](references/storage-and-execution.md#retain-source-storage-instead-of-copying-each-name) |
| Moving a pattern into arena/handle-based code | Keep stored identity and result publication order. | [Omega examples](references/squalr-patterns.md) |

Read only the reference you need; a simple change may need none.

## Shape the implementation

For compiler lowering, apply [compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering).
**Do not add special cases for combinations of features. Clean up as you go;
this is mandatory.** When a fix would need code that recognizes one incidental
arrangement of source, repair the owning operation (or the point where data,
control flow or ownership join) and delete the replaced path in the same change. Do not add another producer, fallback, mode flag
or eligibility exception for combinations such as local versus parameter,
call result versus initializer, or scalar versus Unit caller that need the
same operation. Wrapping the branches in
a shared helper does not generalize them, and a planned cleanup does not
justify another special case.

Keep real differences in type, ownership, effect and ABI explicit, and check
each separately. An optional optimization may specialize only on top of a
correct general path. If the general repair does not fit in scope, report
exactly what is missing; do not ship a workaround that accepts fewer programs.
This covers the mechanism you touched, not unrelated rewrites. Before
reporting completion, name the removed special path and run, through the real
caller, one valid variation of the combination plus one program refused for an
ownership or proof error.

Identify which facts change per request, partition and element. Prepare reusable facts once, outermost; make local choices where their inputs become known; keep the inner operation small. Borrow read-only context, own mutable state, and hand output storage to the consumer.

Use domain names, modules organized by responsibility, and signatures that take exactly each mode's inputs. Reuse existing abstractions; a little duplication is fine to keep a fixed decision out of a hot loop. Recover from an error where recovery belongs. A check that refuses
must say why. Flag `map_err(|_| ...)` or a bare `None` from a validator in
review: without a reason, finding the next failure takes a rebuild per
logged exit. Carry the reason (a string or error variant) to the diagnostic. Apply the [coding conventions](references/rust-conventions.md) when writing or reviewing Rust.

## Finish with evidence

For a proposed change, name the work it avoids and the behavior that must survive. Inspect the whole path, including post-processing and derived equality, hashing, formatting or serialization. Measure peak RAM and time; never infer them.

For implementation, carry the smallest justified change through formatting and [scoped validation](../../../tools/docs/testing.md#validation-scope). Test the ownership, ordering or boundary behavior you changed; measure any claimed speedup. Report what changed, what you verified, and any copy or bottleneck that remains.

Follow [Explain the change](../../../AGENTS.md#explain-the-change) in PRs,
reports and comments: say what the program needs before naming the mechanism.
Check that the before/after behavior, the changed operation and the passing
and refused programs describe one change; if not, inspect code and results
before rewriting the explanation.

Test the changed path with programs that must pass and must be refused;
reuse earlier results when the rules below allow. Run nothing wider without a
reason in this change ([AGENTS.md](../../../AGENTS.md#commands)). Land through
[landing](../../../tools/docs/landing.md).

For compiler work and review, apply [requirements and evidence](../../../AGENTS.md#requirements-and-evidence),
[main-change validation](../../../tools/docs/testing.md#main-change-validation), and
the [independent semantic review boundary](../../../tools/docs/testing.md#independent-semantic-review)
when the assignment requires it. Before reusing an earlier result, check that
the change's premise and chosen meaning still hold, that both outcomes of each
comparison were checked, and which artifact or format changes happen on every
path. Reasoning about which branch runs does not replace Rust checking,
formatting or lint. Keep approval of the exact submitted change separate from
earlier runs; the author resolves findings and lands it.

When changing code organization, follow the changed code from its public
entry point through the operation to result handling, applying
[Discoverability architecture](../../../omega-rust/pipeline.md#discoverability-architecture).
Use [main.rs](../../../omega-rust/omega/application/src/main.rs) and
[target_batch.rs](../../../omega-rust/omega/compilation/compiler/src/target_batch.rs) as examples of
visible coordination, and apply the same test to multi-step flows below them.
Pick the starting file from the file tree; names and calls should show who owns
the work and what comes next. Fix confusing navigation in the change; report
wider problems.
