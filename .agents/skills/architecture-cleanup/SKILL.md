---
name: architecture-cleanup
description: Find and repair high-impact Omega architecture problems, including discoverability, ownership, duplicated orchestration, feature-combination special cases and wasteful data flow. Use for architecture cleanup campaigns or an explicitly assigned structural refactor. Not an automatic repository sweep during a named bug fix, ordinary feature work, or a task-board cleanup.
---

# Architecture Cleanup

Remove unnecessary mechanisms and make necessary work easier to follow and
extend. Apply [AGENTS.md](../../../AGENTS.md), especially discoverability,
compositional lowering, ownership, scope checkpoints and validation. For Rust
inspection or changes, read the repository's
[Rust systems skill](../rust-systems-programmer/SKILL.md) and its relevant
references. Those documents own the coding rules; this skill owns the cleanup
workflow.

Respect the requested area and mode: an audit asks for evidence and proposals;
a cleanup authorizes implementation. Follow repository worktree, claims and
landing procedures. Campaign duration, overlapping-claim permission and broader
e2e recovery come from the invocation, not from a standing skill exception.

## Choose a consequential problem

For repository cleanup, fetch current main and inspect recent work unless the
request pins a revision. Keep supplied-artifact reviews within their provided
evidence. Start from a real operation, customer failure or repeated implementation
cost, not a ranking of file sizes. Scout enough to choose a defensible improvement,
then work it through rather than generating an ever-growing list of possible
cleanups.

First check whether the machinery serves an accepted product requirement.
Compare deletion, consolidation and repair before polishing an abstraction.
Preserve ratified semantics and required evidence; apparently unnecessary trust
or ownership checks need investigation, not removal. Raise an actual unresolved
design conflict through the existing owner process.

Trace the operation from the file tree through its entrypoint, subordinate
decisions, work and result/error handling. Name the reader's question and the
specific point where responsibility or sequencing becomes hard to find. Follow
the route below the crate root: a tidy coordinator above tangled children is
not a completed discoverability improvement.

Favor targets with demonstrated cost: duplicated stage orchestration, options
encoded as families of entrypoints, recognizers for incidental combinations of
otherwise supported operations, misplaced policy, or repeated reconstruction of
relationships that a producer could retain directly. Suspicious names and maps
are search clues, not verdicts. Evaluate the complete workload before changing
storage or adding an index, cache or parallel layer.

## Repair the ownership or representation

Before editing, state the bounded outcome, affected producer/consumers, existing
contract and acceptance. Identify the shared operation or data relationship that
replaces the special cases. Real semantic, ABI, effect and ownership differences
remain explicit; generalization must not turn them into unchecked fallbacks.

Make sequencing visible at the owner of each meaningful multi-step operation.
Choose cohesive names and APIs that expose the domain rather than forwarding
through generic context objects or wrapper chains. A representation crate may
start with its principal data structure; it does not need an invented pipeline.
Move responsibilities and dependency direction, not just source chunks. Leaving
a cohesive file intact can be the right result.

Carry one checked route through producer and consumers before multiplying
implementations. Remove replaced paths as the repair lands; do not retain a
second implementation merely as a migration convenience unless a real contract
requires it. Preserve the non-obvious reasoning at the entry or decision site
where the next reader needs it, rather than in a new architecture diary.

Delegate only independent, bounded work that helps the selected outcome, with
explicit edit ownership and one integration owner. Review findings and changes;
do not parallelize speculative variants of an unproven representation.

## Validate the actual improvement

For bug fixes, witness the failure and exercise the real customer command. For
composition or ownership repairs, challenge the mechanism with a small valid
variation and a relevant invalid control, reusing existing coverage where it
suffices. A helper passing is not evidence that the outer workflow works. For
organization-only changes, trace the before/after navigation route and verify
affected behavior, test selectors and source readers. Follow repository scoped
checks; do not run the full corpus merely to find a reason for more cleanup.

If an e2e failure prevents validation, attribute it. Repair an in-scope dependency
when necessary and justified; record unrelated baseline failures without quietly
starting a general rescue. Do not weaken checks, simplify the customer, or rewrite
expectations to obtain a pass. State precisely which boundaries were verified.

At each checkpoint, report what became easier to find, what duplicated work or
mechanism disappeared, what behavior was verified and what remains. File counts,
smaller functions and anticipated reuse do not establish improvement. Publish
validated checkpoints when authorized through the normal landing protocol.

Stop a bounded assignment when its named problem is resolved. Under a continuing
goal, reassess the next high-impact target against the same criteria rather than
automatically splitting the next large file. A well-supported no-change finding
is valid; it is not a delivered refactor. Apply the repository's scope checkpoints
when work accumulates infrastructure without improving the customer or audit path.
