---
name: advance
description: Drive the Omega Rust reference compiler toward completion by working the execution boards (TASKS.md, TASKS_BOOTSTRAP.md, TASKS_OPTIMIZER.md, TASKS_PACKAGE_MANAGER.md) or obvious unspecified work in the same direction. Use when asked to advance, continue, drive, or make progress on the compiler, to work the boards, or to pick up the next task. Not for a specific named bug, a question about existing code, or a review.
---

# Advance the Omega compiler

One iteration of the completion loop: pick major work, build it, gate it, land it.
`AGENTS.md` already carries the commands, architecture, crate placement rule, and
conventions; `TASKS.md` already carries board hygiene and the ownership firewall.
Do not restate them — this skill is the loop that runs on top.

Composes with `/loop` for unattended sessions. One invocation is one iteration.

## What done looks like

The bar is not "the board is shorter." It is: **the Rust reference compiler
builds real canaries and serious samples using the full Omega feature set, with
features interleaved.** Interleaving is the actual test — one machine reused
across build time, proofs, runtime, and async in the same program. Features that
each work alone but fail in combination mean the internals have a combinatoric
problem, and that problem is the work.

Prefer the task that moves this bar. A skeleton that spans the pipeline end to
end beats a polished slice that stops at a stage boundary.

## Pick the work

Orient first, always:

1. `git pull --rebase` and read the newest commits in the lane you are about to
   touch. Overlapping an active change is the most expensive mistake available
   here. Other sessions commit in this same checkout, so the tree may already be
   dirty and ahead — if the rebase refuses, that is someone else's work in
   progress: fetch and read instead, and leave their files alone.
2. Read the relevant board. If the invocation names a board or its area, that
   board is the only one in scope — do not widen to the others because it looks
   thin, and do not touch their entries. Unnamed means all four boards are the
   candidate pool. Either way you leave with one task — the one that unblocks the
   most downstream work, not the one that is easiest to close.
3. Unspecified but obvious work counts — a missing lowering, an unimplemented
   stage, a representation with no producer. It does not need a board entry to be
   the right next task.

**Side-quests.** The test is whether the work moves or unblocks a board
acceptance condition. If it does not, it is a side-quest, however tempting:

- tidying a file you were only passing through
- refactoring a crate that is not blocking the task
- broadening coverage on a feature that already works
- renaming for consistency, or improving diagnostic wording

Note it and move on. If it names real unfinished work with an acceptance
condition, add a board line; otherwise leave it. Do not open a second front.

## Build it

Priorities specific to this codebase, in the order they bite:

- **Coherent lowering into portable Psi first, then into native.** Terminal Psi is
  the only portable boundary. A native shortcut that skips it is not progress.
- **Arena-backed packed allocation over fragmented `Vec`s.** `Handle<T>` and
  `HandleSpan<T>` for repeated child lists in lowered representations.

Do not add a crate until a module boundary has stopped moving. **Once a boundary
has earned a stage doc, it has earned a crate.** A stage doc under
`wiki/architecture/pipeline/stages/` with no matching `X-to-Y` crate is drift:
the layout has stopped describing the pipeline the docs describe. Put a board
line on it.

## Gate it

Run `mbx --version` first. It must report 1.7.0 or newer. If it does not, stop
and report the missing prerequisite; never substitute direct Cargo. Every
compiling command in this section uses `mbx`, including commands run in a
temporary clone or background job. Only `cargo fmt` and `cargo clean` remain
direct Cargo commands.

Run the gate list once before you edit anything. A repository is not reliably
green, and a red gate you cannot attribute leads to both expensive mistakes at
once: chasing someone else's failure into a second front, or landing your own
under cover of it. One run up front settles which is which.

Inner loop, while iterating:

```bash
mbx check --workspace --all-targets
```

plus the single filtered test for what you are changing.

Then the full baseline gate list in `AGENTS.md` — fmt, clippy, the architecture
test, check, and `mbx test --workspace --lib --no-fail-fast` — **on the tree
you are about to
commit**. Gates from before your edits describe a tree that no longer exists;
they are orientation, not evidence. The list is also not conditional on which
files you touched: fmt and clippy read every file, the architecture test reads
crate layout, and library tests read checked-in fixtures, so a commit with no
`.rs` file in it can still move all three. The architecture test is the only
thing that catches a wrong-direction dependency.

Report gate results against that pre-existing set: what is red now, what was
already red, and which gates you did not run. A failing gate you describe as
passing costs more than the failure did, and so does a gate list you imply you
ran.

## Land it

**Work directly on `main`. Do not branch first.** Invoking this skill is the
user's standing authorization to commit and push there — it overrides the
default "branch before committing on the default branch" behavior. A loop that
branches silently never lands anything.

Commit and push to `main` on coherent milestones — a working improvement, not a
finished epic. Terse imperative subject matching `git log`. Small checkpoint
commits are correct; a single giant commit at the end is not.

Update the board in the same change: delete the task when its acceptance
condition passes. Do not append landed substeps, test counts, or release notes —
boards are execution state, not changelogs.

**Debt gets registered on the way in, not on the way out.** If a change leaves
behind state the docs call temporary, transitional, bounded, or compatibility —
a second route on one input, a shim, a lane that exists only until another one
covers its roster — it gets a board entry in the same commit, naming the crate
and the acceptance condition that deletes it. A mandate that lives only in
`wiki/` prose is not tracked work; nothing will ever pick it up. Landing the
temporary thing without its deletion condition is the whole failure mode.

## When blocked

**Design blockers go to `OWNER_QUESTIONS.md`. Implementation details do not.**
If the end shape is known and only the work remains, it is not a design question
— build it. Engineering difficulty is never a design blocker.

Before filing, confirm it clears the bar that file sets: an independently
motivated product requirement or credible external use case. A test, benchmark,
or implementation task is never sole motivation. Audit first whether ordinary
Omega already expresses the customer.

If a slice is genuinely blocked and the question is filed, say so plainly and
move to the next task in the same session rather than stalling.
