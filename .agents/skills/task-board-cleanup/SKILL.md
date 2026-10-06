---
name: task-board-cleanup
description: Audit and debloat Omega execution boards into actionable unfinished work. Use for task-list cleanup, stale-task audits, deduplication, or a board-wide cleanup campaign. Not routine task updates during implementation, new-task mining, or swarm scheduling.
---

# Task Board Cleanup

Make the requested board trustworthy enough to assign work without rediscovering
the problem.

Read [AGENTS.md](../../../AGENTS.md), especially the board and validation
rules, and follow its worktree, claims and publication procedures. The
invocation determines scope and persistence; paths are advisory under AGENTS.md.
A review-only
request produces findings.

## Audit before compressing

For repository cleanup, fetch current main and inspect recent changes in the
affected lane unless the request pins a revision. Identify the requested board
or section and work in coherent groups of related items. For a whole-board
assignment, track coverage of the original items in session context and work
every section.

For each item, locate the governing requirement, owning implementation and
acceptance evidence. Read relevant test bodies and their harness. Distinguish
declaration, checked source, retained representation, independent verification
and actual execution when the task depends on them. Commit messages and repeated
reports are leads; running the acceptance is proof.

Investigate the uncertainty that changes the disposition: reuse evidence whose
inputs remain applicable, run a focused probe when inspection cannot resolve
it. If status cannot be verified, write the precise verification needed.

## Keep the actual remaining assignment

- Delete completed or superseded work when its acceptance is accounted for.
  Remove revoked or out-of-scope assignments using the governing decision.
  Missing implementation is unfinished work, and stays.
- Consolidate duplicates under the owner of the missing capability, preserving
  distinct acceptance cases and dependencies. Split only when it enables
  independently useful work; retain the parent outcome's integration DRI.
  A next diagnostic or implementation step normally updates the current frontier,
  not the number of tasks. Never mine assignments just to occupy agents.
- Challenge dependencies: separate an executable concurrency witness from model
  work that can proceed without a runtime, for example. Engineering difficulty
  and an old ownership stamp are engineering, not design blockers.
- Distinguish implementation gaps from unsettled language or trust decisions;
  the latter go to `OWNER_QUESTIONS.md`.
- Flag proposed source-shape workarounds, duplicated pipeline paths or misplaced
  ownership, keeping the valid program requirement while correcting the
  assignment. The repair itself is the advance skill's work.

Each retained item explains the missing behavior, owning code/specification,
necessary context, real dependencies and concrete acceptance, with a current
reproduction or decisive symbol/test link when it saves rediscovery, in
natural prose or bullets.

Keep outcome IDs stable for active agents. Active/Ready/Parked are views of the
same items; live claims record DRIs, not file exclusions. Performance and test
environment incidents are valid outcomes with measured acceptance and a stop
condition; dormant standing duties are not invitations to optimize forever.

Replace accumulated history with the current state once: remove dated landing
logs, expired claims, repeated rationale, test totals and copied repository
rules. Keep rationale that would change the next implementation decision and
every real dependency.

Fold distinct reproductions into the existing capability assignment, keeping
its acceptance and integration owner. Rewrite "another agent should
investigate" as the actual remaining repair or verification when the evidence
supports it. Mark unverified leads as unverified.

## Publish an audited batch

Check the diff against the original items: every required acceptance is still
present, every status is verified, and every assignment traces to an original
item. Preserve referenced task identities where practical and update active
references to consolidated or removed items. Keep bootstrap and optimizer work
with their owning boards; leave historical records alone.

Validate links, ownership, dependency references and consistency. Reconcile
incoming board edits by their remaining requirements, never union-merge history.
Commit and publish coherent batches when
authorized, following the repository landing protocol.

Report the meaningful deletions/consolidations, unresolved questions and the
coverage still remaining. Continue while the requested campaign remains
unfinished; a complete audit means all in-scope items are accounted for. New
findings go on the board when they preserve a real requirement or explain an
existing task.
