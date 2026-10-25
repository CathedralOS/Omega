---
name: task-board-cleanup
description: Audit, reframe, merge, and debloat Omega execution boards into cohesive end-to-end sample outcomes sized for a single agent loop. Use for task-list cleanup, reframing stage-sliced tasks into end-to-end pipeline samples, merging over-split tasks by similarity, stale-task audits, or a board-wide cleanup campaign. Not routine task updates during implementation, new-task mining, or swarm scheduling.
---

# Task Board Cleanup

Make the requested board trustworthy enough to assign work without rediscovering
the problem, and shape each item so a single capable agent can carry a complete
path end to end without stage handoffs or fragmented sub-tasks.

Read [AGENTS.md](../../../AGENTS.md), especially [the sample loop](../../../AGENTS.md#the-sample-loop)
and the board and validation rules, and follow its worktree, claims and
publication procedures. The invocation determines scope and persistence; paths
are advisory under AGENTS.md. A review-only request produces findings.

## How tasks enter and live on the board

Board tasks follow one flow:

1. **An issue or capability gap is found** (a spec requirement, a customer
   blocker, or a construct that fails checking, lowering, or execution).
2. **It is framed as an end-to-end sample outcome:** the task names the
   executable sample witness (`samples/cli/facts/<fact>/` across the nine
   transport places with fact-sensitive reject twins, or the named customer
   program with its invalid controls) that demonstrates the requirement and
   carries it through the entire pipeline—from source checking and proof
   (`Psi 00–04`) through Terminal Psi lowering and interpretation (`Psi 05–07`)
   to target realization, layout/calling plans, instruction selection, register
   homes, encoding, and native image execution agreement (`Omega 00–09`,
   `omega run --both`).
3. **It is merged with similar tasks into one main agent loop:** related
   witnesses, caller families, and migration steps that share the same
   mechanism, representation, or subsystem context are folded into one cohesive
   outcome so a single agent can complete them alongside each other in one
   continuous loop ([When to fold subagents into your main agent loop](https://claude.com/resources/articles/building-agents-when-to-fold-subagents-into-your-main-agent-loop)).

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

## Reframe and merge for one agent loop

Capable agents carry a mechanism across every compiler layer and across adjacent
cases once they hold the context in working memory. Over-splitting a capability
into `split-of:` shards, stage-local tickets, or one item per caller family
creates handoff loss, duplicate ramp-up, and competing partial fixes.

- **Reframe stage-sliced or internal tasks around end-to-end samples.** If an
  item is phrased as an internal pass edit, intermediate representation change,
  or single-stage obligation ("teach stage 04 to...", "restore X in compile-time
  evaluator", "wire Y into backend plan"), reframe it around the runnable sample
  or `samples/cli/facts/<fact>/` row that proves the requirement end to end
  through `omega run --both` and its reject twins. Keep internal symbols and
  stage owners as implementation context inside that outcome, not as the
  definition of done. Non-sample acceptance stays only for genuine
  development-health incidents (`ITERATION-PERFORMANCE`,
  `VALIDATION-ENVIRONMENT`), bootstrap proofs, or external host/release gates.
- **Merge by similarity along the path an agent walks.** Fold items together
  when an agent solving one already has the context, invariants, and files open
  to solve the others in the same loop:
  - Collapse `split-of:` chains, sequential migration phases (e.g., seed ->
    cutover -> per-family restoration -> closure), and compiler-plus-source
    migration pairs into one parent outcome ordered as a ladder of executable
    witnesses.
  - Group sibling items that touch the same semantic rule, ownership/loan
    transition, layout/ABI mechanism, or pipeline boundary so one agent
    generalizes the operation once and discharges its witnesses together.
  - Fold distinct reproductions and customer-exposed gaps into the capability
    item that owns the underlying mechanism, keeping each required witness and
    oracle in its acceptance list.
- **Split only at real independence boundaries.** Keep separate items only when
  the mechanisms are genuinely independent (so parallel agents would not be
  re-learning the same subsystem and fighting over the same transition) or when
  a piece is blocked on a real external prerequisite (`OWNER_QUESTIONS.md` or
  unavailable target hardware). Never split a task by pipeline stage, producer
  vs. consumer crate, or individual witness just to create more board entries.

## Keep the actual remaining assignment

- Delete completed or superseded work when its acceptance is accounted for.
  Remove revoked or out-of-scope assignments using the governing decision.
  Missing implementation is unfinished work, and stays.
- Challenge dependencies: separate an executable concurrency witness from model
  work that can proceed without a runtime, for example. Engineering difficulty
  and an old ownership stamp are engineering, not design blockers.
- Distinguish implementation gaps from unsettled language or trust decisions;
  the latter go to `OWNER_QUESTIONS.md`.
- Flag proposed source-shape workarounds, duplicated pipeline paths or misplaced
  ownership, keeping the valid program requirement while correcting the
  assignment. The repair itself is the advance skill's work.

Each retained item states the capability requirement and its executable
end-to-end sample witnesses (`samples/cli/facts/<fact>/` or named customer
programs plus invalid controls), how the requirement travels through the
pipeline, the owning code/specification, real dependencies, and a current
reproduction or decisive symbol/test link when it saves rediscovery, in natural
prose or bullets.

When merging items, fold them under the canonical parent outcome ID and update
cross-references (including `TASKS*.md`, sample READMEs, and active claim notes
when applicable) so active agents can trace the merged ID cleanly. Active/Ready/Parked
are views of the same items; live claims record DRIs, not file exclusions.
Performance and test-environment incidents are valid outcomes with measured
acceptance and a stop condition; dormant standing duties are not invitations to
optimize forever.

Replace accumulated history with the current state once: remove dated landing
logs, expired claims, repeated rationale, test totals and copied repository
rules. Keep rationale that would change the next implementation decision and
every real dependency. Rewrite "another agent should investigate" as the actual
remaining repair or verification when the evidence supports it. Mark unverified
leads as unverified.

## Publish an audited batch

Check the diff against the original items: every required acceptance is still
present, every retained compiler task is framed around end-to-end sample
evidence across the pipeline, over-split sibling items are merged into cohesive
single-agent outcomes, every status is verified, and every assignment traces to
an original item. Preserve referenced task identities where practical and update
active references to consolidated or removed items. Run
`python tools/board_paths.py --board TASKS.md` when cited paths move. Keep
bootstrap and optimizer work with their owning boards; leave historical records
alone.

Validate links, ownership, dependency references and consistency. Reconcile
incoming board edits by their remaining requirements, never union-merge history.
Commit and publish coherent batches when
authorized, following the repository landing protocol.

Report the meaningful merges, reframings, deletions, unresolved questions and
the coverage still remaining. Continue while the requested campaign remains
unfinished; a complete audit means all in-scope items are accounted for. New
findings go on the board when they preserve a real requirement or explain an
existing task.
