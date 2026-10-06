---
name: architecture-cleanup
description: Find and repair high-impact Omega architecture problems, including discoverability, ownership, duplicated orchestration, feature-combination special cases and wasteful data flow. Use for architecture cleanup campaigns or an explicitly assigned structural refactor. Not an automatic repository sweep during a named bug fix, ordinary feature work, or a task-board cleanup.
---

# Architecture Cleanup

Remove unnecessary mechanisms and make necessary work easier to follow and
extend. Apply [AGENTS.md](../../../AGENTS.md) and
[omega-rust/pipeline.md](../../../omega-rust/pipeline.md), especially
discoverability, compositional lowering, ownership, scope checkpoints and
validation. For Rust
inspection or changes, read the repository's
[Rust systems skill](../rust-systems-programmer/SKILL.md) and its relevant
references. Those documents own the coding rules; this skill owns the cleanup
workflow.

Respect the requested area and mode: an audit asks for evidence and proposals;
a cleanup authorizes implementation. Follow repository worktree, claims and
landing procedures. Campaign duration comes from the invocation. Claims own
outcomes, not files; report overlaps and reconcile rather than avoiding the
necessary repair. Retain one DRI through integration and acceptance.

## Choose a consequential problem

For repository cleanup, fetch current main and inspect recent work unless the
request pins a revision. Start from a real operation, a program failure, or a
repeated implementation cost. Scout enough to choose one improvement, then
work it through.

First check whether the machinery serves an accepted product requirement.
Compare deletion, consolidation and repair before polishing an abstraction.
Preserve ratified semantics and required evidence; investigate a trust or
ownership check before touching it. Raise an unresolved design conflict
through `OWNER_QUESTIONS.md`.

Trace the operation from the file tree through its entrypoint, subordinate
decisions, work and result/error handling. Name the reader's question and the
specific point where responsibility or sequencing becomes hard to find, and
follow the route below the crate root.

Targets with demonstrated cost: duplicated stage orchestration, options encoded
as families of entrypoints, recognizers for incidental combinations of otherwise
supported operations, misplaced policy, or repeated reconstruction of
relationships a producer could retain directly. Evaluate the complete workload
before changing storage.

## Repair the ownership or representation

Before editing, state the bounded outcome, affected producer/consumers, existing
contract and acceptance. Identify the shared operation or data relationship that
replaces the special cases. Real semantic, ABI, effect and ownership differences
remain explicit; generalization must not turn them into unchecked fallbacks.

Make sequencing visible at the owner of each meaningful multi-step operation.
Choose cohesive names and APIs that expose the domain. A representation crate
may start with its principal data structure. Move responsibilities and
dependency direction. Leaving a cohesive file intact can be the right result.

Carry one checked route through producer and consumers. Remove replaced paths
as the repair lands. Write the non-obvious reasoning at the entry or decision
site where the next reader needs it.

Delegate only independent, bounded work with explicit edit ownership and one
integration owner, and review the returned changes.

## Validate the actual improvement

For bug fixes, witness the failure and run the real program command. For
composition or ownership repairs, challenge the mechanism with a small valid
variation and a relevant invalid control. For organization-only changes, trace
the before/after navigation route and verify affected behavior, test selectors
and source readers. Follow repository scoped checks.

If an e2e failure prevents validation, attribute it on the base commit and
repair an in-scope dependency when the change needs it. Checks, the program,
and expectations stay as they are; the pass comes from the code. State which
boundaries were verified.

At each checkpoint, report what became easier to find, what duplicated
mechanism disappeared, what behavior was verified and what remains. Publish
validated checkpoints through the normal landing protocol.

Stop when the named problem is resolved. Under a continuing goal, choose the
next target by the same demonstrated-cost criteria.
