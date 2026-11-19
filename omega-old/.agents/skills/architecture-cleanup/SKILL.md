---
name: architecture-cleanup
description: Find and repair high-impact Omega architecture problems, including discoverability, ownership, duplicated orchestration, feature-combination special cases and wasteful data flow. Use for architecture cleanup campaigns or an explicitly assigned structural refactor. Not an automatic repository sweep during a named bug fix, ordinary feature work, or a task-board cleanup.
---

# Architecture Cleanup

Delete mechanisms the compiler does not need, and make the rest easier to
follow and extend. Apply [AGENTS.md](../../../AGENTS.md) and
[omega-rust/pipeline.md](../../../omega-rust/pipeline.md), especially their
rules on discoverability, compositional lowering, ownership and validation. For
Rust, read the [Rust systems skill](../rust-systems-programmer/SKILL.md) and its
relevant references. They own the coding rules; this skill owns the workflow.

An audit asks for evidence and proposals; a cleanup authorizes implementation.
Stay in the requested area and follow the repository's worktree, claims and
landing procedures. The request sets the campaign's length. A claim only shows
others your work; report overlaps and reconcile them rather than skip a needed
repair. The session that starts a cleanup carries it through merge and
acceptance.

## Choose a consequential problem

Fetch current `main` and read recent work unless the request names a revision.
Start from a real operation, a failing program, or a cost implementers keep
paying. Choose one improvement, then finish it.

First check that the code serves an accepted product requirement. Consider
deleting, merging or repairing it before polishing an abstraction. Keep ratified
semantics and their evidence; learn what a trust or ownership check protects
before touching it. Raise design conflicts in `OWNER_QUESTIONS.md`.

Follow the operation from the file tree to its entry point, decisions, work,
results and errors. Name the reader's question and where it becomes hard to see
who is responsible or what runs when, below the crate root too.

Good targets have a cost you can show: one stage sequence written in several
places, options as a family of entry points, code that recognizes one
combination of supported operations, misplaced policy, or code rebuilding
relationships the producer could pass on. Measure the whole workload before
changing storage.

## Repair the ownership or representation

Before editing, state the outcome and its limits, the affected producer and
consumers, their contract, and how you will prove the result. Find the one
shared operation or data relationship that replaces the special cases. Keep
real semantic, ABI, effect and ownership differences explicit, never unchecked
fallbacks.

Make each multi-step operation's order visible in the code that owns it. Choose
names and APIs that say what the code does. A representation crate may start
with its main data structure. Move responsibilities and fix dependency
direction; leaving a coherent file intact can be right.

Send one checked route from producer through consumers, deleting the paths it
replaces. Write non-obvious reasoning where readers need it.

Code unused on `main` may still be needed by work in progress. Before deleting
it, search the lines that open pull requests and recently pushed `work/*`
branches add for each name you remove, keep anything they use, and leave files
they modify alone. A duplicate between a producer and its independent validator
is deliberate: the validator must not share the producer's mistakes, so do not
merge the two.

Delegate only complete, independent pieces; whoever does each lands it, and you
review it.

## Validate the actual improvement

For a bug fix, reproduce the failure and run the real program command. For a
composition or ownership repair, test a small valid variation and a relevant
program that must be refused. For a reorganization, compare how a reader finds
the code before and after, and check behavior, test selectors and readers of the
moved files. Run the repository's focused checks.

If an end-to-end failure blocks validation, check whether the base commit also
fails, and repair an in-scope dependency your change needs. Leave checks, the
program and its expected result alone; the pass must come from the code. Say
which boundaries you verified.

At each checkpoint, report what became easier to find, which duplicate you
deleted, what you verified and what remains. Land validated checkpoints.

Stop when the named problem is fixed. Under a continuing goal, pick the next
target by a cost you can show.
