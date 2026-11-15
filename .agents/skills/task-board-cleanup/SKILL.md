---
name: task-board-cleanup
description: Audit, reframe, merge, and debloat Omega execution boards so each item is one end-to-end sample outcome a single agent can finish in one loop. Use for task-list cleanup, reframing stage-sliced tasks into end-to-end pipeline samples, merging over-split tasks by similarity, stale-task audits, or a board-wide cleanup campaign. Not routine task updates during implementation, new-task mining, or swarm scheduling.
---

# Task Board Cleanup

Clean the requested board so an item can be assigned without rediscovering the
problem, and so a single agent can carry each item through every compiler
stage without handoffs or split-off fragments.

Read [AGENTS.md](../../../AGENTS.md), especially [the sample loop](../../../AGENTS.md#the-sample-loop)
and its board and testing rules, and follow its worktree, claims and landing
procedures. The request sets scope and whether you commit; paths here are
suggestions under AGENTS.md. A review-only request produces findings, not edits.

## How tasks enter and live on the board

Every board task follows three steps:

1. **Someone finds a problem or missing capability**: a spec requirement, a
   customer blocker, or a construct that fails checking, lowering or execution.
2. **The task is written as an end-to-end outcome.** It names the existing
   runnable sample or customer program, its negative twins (see the
   [glossary](../../../AGENTS.md)), and what must work through the lowered-form
   checker, Terminal Psi, and native/Psi execution (`omega run --both`). Assign
   repairs to the current stage owners in [AGENTS.md](../../../AGENTS.md#law-of-completion);
   the old tree checker is removed, so never route a repair to it. Do not list
   places to cover one by one ([sample loop](../../../AGENTS.md#the-sample-loop)).
   `samples/cli/facts/` is frozen. A new regression program goes beside the
   changed code's test under `tests/fixtures/`, normally one positive/negative
   pair, within the per-PR file limit.
3. **It is merged with similar tasks into one agent loop.** Fold related test
   programs, caller groups and migration steps that share a mechanism,
   representation or subsystem into one outcome, so one agent finishes them
   together while it holds the context
   ([When to fold subagents into your main agent loop](https://claude.com/resources/articles/building-agents-when-to-fold-subagents-into-your-main-agent-loop)).

## Audit before compressing

Fetch current `main` and read recent changes in the affected lane, unless the
request names a revision. Work through the requested board or section in groups
of related items. For a whole board, track which original items you have
covered and work every section.

For each item, find its governing requirement, implementing code and proving
test or program. Read the relevant test bodies and their harness. Where it
matters, tell apart a feature that is only declared, checked, kept in the
lowered program, independently verified, or actually run. Commit messages and
repeated reports are leads; running the acceptance is proof.

Investigate only what would change the item's fate. Reuse results whose inputs
still apply; run a small probe when reading cannot settle it. If you cannot
verify status, write down the exact check that would.

## Reframe and merge for one agent loop

Once an agent understands a mechanism, it can carry it through every stage and
across neighboring cases. Splitting a capability into `split-of:` pieces,
per-stage tickets, or one item per caller group loses context at each handoff,
duplicates learning, and produces competing partial fixes.

- **Rewrite stage-specific or internal tasks around end-to-end samples.** An
  item phrased as one internal pass edit, representation change, or single-stage
  job ("restore X in compile-time evaluator", "wire Y into backend plan")
  becomes an outcome around the runnable sample or customer program that proves
  the requirement with `omega run --both` and its negative twins. Internal
  symbols and stage owners stay as notes, not as the definition of done. Only
  development-health problems (`ITERATION-PERFORMANCE`,
  `VALIDATION-ENVIRONMENT`), bootstrap proofs, and external host or release
  gates may be accepted without a sample.
- **Merge items an agent would solve together**, because solving one leaves the
  context, invariants and files for the others open:
  - Collapse `split-of:` chains, sequential migration phases (e.g., seed ->
    cutover -> per-family restoration -> closure), and paired compiler and
    source migrations into one parent outcome, its runnable programs listed in
    the order they should start working.
  - Group siblings that touch the same language rule, ownership or borrow
    change, layout or ABI mechanism, or stage boundary, so one agent
    generalizes the operation once and makes their programs pass together.
  - Fold separate reproductions and customer-visible gaps into the item owning
    the mechanism, keeping each required program and expected result in its
    acceptance list.
- **Split only where work is really independent**: the mechanisms do not
  overlap (parallel agents would not relearn and fight over the same code), or
  a piece waits on a real outside prerequisite (`OWNER_QUESTIONS.md` or
  unavailable target hardware). Never split by pipeline stage, by producing
  versus consuming crate, or by test program just to create entries.

## Keep the actual remaining assignment

- **Preserve `## Priority (human-set only)` order and membership.** Only a human
  adds, reorders, or moves items into or out of the top `## Priority (human-set only)`
  section in `TASKS.md`. Cleanup may delete completed sub-bullets, delete an
  item there once its full `Acceptance:` is proven on `main`, and fold
  duplicates into an item already there, but must never demote, reorder, or
  move items into or out of `## Priority (human-set only)`. Keep ratified
  language-design and syntax migrations that touch many `.omg` files above
  ordinary feature and library tasks, so agents finish them before writing more
  `.omg` source in retired syntax.
- Delete completed or superseded work once its acceptance is accounted for.
  Remove revoked or out-of-scope assignments, citing the deciding ruling.
  Missing implementation is unfinished work, and stays.
- Question dependencies: a test needing a concurrent runtime is separate from
  model work that can proceed without one. Hard engineering and old ownership
  notes are engineering, not design blockers.
- Unsettled language or trust decisions go to `OWNER_QUESTIONS.md`;
  implementation gaps stay on the board.
- Flag proposals that change program source to dodge a compiler gap, duplicate
  a pipeline path, or misplace ownership. Keep the valid program requirement
  and correct the assignment; the repair is the advance skill's work.

Each kept item states its required capability, owning code and spec, real
dependencies, remaining sub-bullets, and final `Acceptance:` gate. Write one
sub-bullet per line, each one unfinished program or missing mechanism, with its
reproduction or deciding symbol or test link when that saves rediscovery. Tests
beside the changed code hold regressions; existing samples hold whole programs.
`TASKS.md` holds only what is unbuilt or failing. A PR tests its repair and a
small area around it, not every case of the capability; a failure there calls
for investigating it, not a wider run or a replacement corpus.
Separate bullets let an agent that finishes one program delete its line instead
of rewriting prose or moving it into a
`"preserving / retaining / keeping / now reserves / now passes"` regression list.

When merging, put items under the parent outcome ID and update references
(including `TASKS*.md`, sample READMEs, and active claim notes when present) so
agents can trace the merged ID. Active/Ready/Parked are views of the same
items; a live claim names who is responsible, not locked files. Performance and
test-environment problems are valid outcomes with measured acceptance and a
stopping point; standing duties are not invitations to optimize forever.

Replace accumulated history with the current state once: remove dated landing
logs, expired claims, repeated rationale, test totals and copied repository
rules. Keep rationale that would change the next implementation decision, and
every real dependency. Where evidence supports it, turn "another agent should
investigate" into the actual remaining repair or check. Mark unverified leads
as unverified.

## Publish an audited batch

Check the diff against the original items: every required acceptance remains,
every kept compiler task rests on an end-to-end sample, over-split siblings are
merged, every status is verified, and every assignment traces to an original
item. Keep referenced task IDs where practical and update references to merged
or removed items. Run `cargo xtask check-rules --base origin/main` when cited
paths or task IDs move. Leave bootstrap and optimizer work on their own boards,
and historical records alone.

Check links, ownership, dependency references and consistency. Reconcile
concurrent board edits by what each item still requires; never union-merge
history. Commit and publish coherent batches when authorized, following the
landing procedure.

Report the important merges, rewrites, deletions, open questions and remaining
coverage. Continue while the campaign is unfinished; an audit is complete when
every in-scope item is accounted for. Add a finding to the board only when it
keeps a real requirement or explains an existing task.
