---
name: task-board-cleanup
description: Audit and debloat Omega execution boards into actionable unfinished work. Use for task-list cleanup, stale-task audits, deduplication, or a board-wide cleanup campaign. Not routine task updates during implementation, new-task mining, or swarm scheduling.
---

# Task Board Cleanup

Make the requested board trustworthy enough to assign work without rediscovering
the problem. Shorter is useful only when the remaining requirements stay clear.

Read [AGENTS.md](../../../AGENTS.md), especially its workflow and validation
rules. Follow its worktree, claims and publication procedures. The invocation
determines scope, persistence and any overlap permission; this skill grants no
standing exception. A review-only request produces findings, not board edits.

## Audit before compressing

For repository cleanup, fetch current main and inspect recent changes in the
affected lane unless the request pins a revision. Keep supplied-artifact reviews
within their provided evidence. Identify the requested board or section and work
in coherent groups of related items. For a whole-board assignment, retain coverage
of the original items across checkpoints in session context; do not create another
tracked ledger. Do not reread the whole board on every continuation or silently
stop after the easiest section.

For each item, locate the governing requirement, owning implementation and
acceptance evidence. Read relevant test bodies and their harness, not just names
or counts. Distinguish declaration, checked source, retained representation,
independent verification and actual execution when the task depends on them.
Commit messages, mined findings and repeated reports are leads, not proof of
completion. Source absence alone does not make a required feature obsolete.

Investigate the uncertainty that changes the disposition. Reuse evidence whose
inputs remain applicable; run a focused probe when inspection cannot resolve it.
Do not establish a full compiler baseline just to edit a task. If status cannot
be verified, retain the precise verification needed rather than asserting that
the work is complete or blocked.

## Keep the actual remaining assignment

- Delete completed or superseded work only when its acceptance is accounted for.
  Remove revoked or out-of-scope assignments using the governing decision, not
  an agent's preference. Missing implementation is not grounds for deletion.
- Consolidate duplicates under the owner of the missing capability. Preserve
  distinct acceptance cases and dependencies; do not merge unrelated work merely
  because it touches the same file. Split only when it enables independent work,
  not once per feature permutation or landed substep.
- Challenge dependencies. Separate an executable concurrency witness from model
  work that can proceed without a runtime, for example. Engineering difficulty
  and an old ownership stamp are not design blockers.
- Distinguish implementation gaps from genuinely unsettled language or trust
  decisions. Use `OWNER_QUESTIONS.md` under the existing escalation criteria;
  ordinary engineering decisions stay with the implementer.
- Flag proposed source-shape workarounds, duplicated pipeline paths or misplaced
  ownership. Keep the valid customer requirement while correcting the assignment;
  do not implement the repair during a board cleanup.

Each retained item should explain the missing behavior, owning code/specification,
necessary context, real dependencies and concrete acceptance. Include a current
reproduction or decisive symbol/test link when it saves rediscovery. Keep relevant
failure-path and independent-checking requirements. Use natural prose or bullets,
not a compulsory field template or word quota.

Replace accumulated history with the current state once. Remove dated landing
logs, expired claims, repeated rationale, test totals and copied repository rules.
Keep rationale that would change the next implementation decision. Do not erase
a real dependency merely to make the item look immediately actionable.

## Publish an audited batch

Check the diff against the original items: no required acceptance disappeared,
no uncertain status became a fact, and no new assignment was invented to replace
deleted noise. Preserve referenced task identities where practical and update
active references to consolidated or removed items. Keep bootstrap and optimizer
work with their owning boards; leave historical records alone.

Validate links, ownership, dependency references and consistency. Reconcile
incoming board edits by their meaning; neither union-merge obsolete paragraphs
nor overwrite new requirements with a stale snapshot. Commit and publish coherent
batches when authorized, following the repository landing protocol.

Report the meaningful deletions/consolidations, unresolved questions and the
coverage still remaining. Counts are supporting information, not the objective.
Continue when the requested campaign remains unfinished; claim a complete audit
only after all in-scope items are accounted for. New findings belong on the board
only when needed to preserve a real requirement or explain an existing task, not
as a fresh speculative mining campaign.
