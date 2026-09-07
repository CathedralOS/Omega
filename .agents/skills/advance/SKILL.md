---
name: advance
description: >-
  Deliver one bounded Omega compiler improvement from the execution boards or
  an existing customer example. Use for advance, continue the compiler, work the
  boards, or pick the next compiler task. Not for a named bug fix, code question,
  review, or repository-wide health check.
---

# Advance the Omega compiler

One invocation delivers a bounded compiler improvement through publication.
A blocked or paused task does not end an unrestricted invocation: select other
actionable work. Reconfirming a known blocker is verification-only, not a
completed advance. Follow [AGENTS.md](../../../AGENTS.md) for repository
ownership, validation, scope checkpoints, board hygiene, and publication.

## Choose the next customer outcome

Record the start time, starting checkout, and branch. Fetch main and inspect
status and recent lane commits. If the starting checkout is clean and on main,
fast-forward it with `git merge --ff-only origin/main` before reading its boards.
Otherwise preserve it and read from an isolated worktree based on fetched main.

For a continuation, retain the existing customer program or required proof
obligation until acceptance passes, a real dependency blocks it, a scope checkpoint
requires a pause, or the user changes priority. Do not duplicate another session's
active change or switch to an easier unrelated helper. Without prior continuity,
choose from `TASKS.md`, `TASKS_BOOTSTRAP.md`, or `TASKS_OPTIMIZER.md`; honor a named
board. Skip already-blocked or paused tasks during fresh selection. If a selected
customer becomes blocked or its strategy requires a pause, preserve its resume
evidence and select independent actionable work within the user's allowed scope.
Do not resume the paused strategy under another helper name. A named board limits
selection to that board; an explicitly named customer limits it to that customer
and its useful dependencies. State the customer, missing dependencies, and bounded
acceptance condition.

Read the owning design and [completion contract](../../../wiki/releases/rust_compiler_completion_contract.md).
For bootstrap work, also read [whole-chain minimization](../../../wiki/design_briefs/bootstrap_minimization.md).
Apply [scope checkpoints](../../../AGENTS.md#scope-checkpoints) using recent
milestones across invocations and delegated work. Required customer behavior and
human-auditable proof closure justify work; a board item or passing helper alone
does not. Compare simpler alternatives before adding machinery.

## Work from a should-be-working example

Use the customer's actual command as the outer loop, including CLI, package
preparation, publication, and execution where applicable. Trace expected output
and exit status backward through the producer and consumers. Existing logs and
one focused probe are enough to locate the first failure; do not run the entire
corpus merely to rank work.

The language and architecture govern the solution. An example exposes a missing
capability without defining its limits or authorizing sample-specific semantics,
intrinsics, relaxed checks, or compiler paths. Implement the smallest capability
consistent with the design and retain its broader required cases and evidence.

Witness the failure before editing. Read the generated phase artifacts described
in AGENTS.md before instrumenting compiler code. Distinguish the observed boundary
from downstream gaps inferred from source. A checked-tree milestone can be useful
while native production remains blocked, but report that boundary accurately.

Keep the outer command in the sample's README or owning design document and the
current first failure in its existing board item. After a milestone, rerun the
command when its inputs changed. A passing inner test with an unchanged customer
failure is dependency progress, not a working example. Never simplify the
customer's program merely to move the failure.

## Implement and validate

Use an isolated worktree under `<repository>/.codex/worktrees/<short-name>`.
Keep its name short; if a generated Windows path fails, measure the actual path
before relocating work or changing compiler architecture.

Trace the shared implementation and callers, preserving the Psi/Omega firewall
and proof, custody, and trust checks. Prefer the existing representation and
provider route. A single bounded change normally needs one agent; delegate only
independent useful work that justifies its briefing and review cost.

Select checks from the affected behavior under
[validation scope](../../../AGENTS.md#validation-scope). A bug fix needs a witnessed
regression. Read the harness before filtering: fixture-path filters and nextest
test-name filters select different things. Include affected crate and integration
checks, plus architecture checks when their ownership or source-reader inputs
change. Reuse successful results on unchanged inputs, including at landing.

Attribute unexpected failures using a focused baseline comparison or dependency
and source-reader evidence. Confirmed unrelated failures remain outside the repair
scope; retain their command, revision, and attribution. New or unexplained affected
failures block landing. A new worktree does not require a full baseline. Use
[testing](../../../wiki/testing.md) for coverage and
[test-cycle measurements](../../../wiki/testing_performance.md) for build diagnosis.
Avoid concurrent host builds and duplicate checks.

## Delegate a bounded assignment

Read [agent delegation](../../../AGENTS.md#agent-delegation) for current model
routing and assignment requirements. Keep that policy in AGENTS.md. Give the worker
its exact worktree/revision and canonical skill path, objective, design anchors,
edit ownership, dependencies, acceptance, and escalation conditions. Refine the
owning board only when it lacks context needed for the assignment.

Use the actual agent tool and returned ID before reporting a worker as launched.
If unavailable, continue locally where possible and report the limitation.
Monitor through completion or a concrete blocker, respond to findings, and inspect
the returned diff and evidence before integration. Worker IDs belong in the
conversation, not execution boards.

Distinguish implementation completed, verification-only completed, diagnosed
blocker, and work continuing. Evidence identifies revision, host, command, exit,
and remaining dependencies. A prerequisite check does not establish the assigned
acceptance; cross-emission does not establish target runtime behavior. Diagnosis
is intermediate for an assigned fix: continue implementation when the design
answers it, or hand off the exact missing dependency and next acceptance.

## Land and restore the checkout

Invoking advance authorizes committing and publishing the bounded improvement
through [the landing protocol](../../../tools/landing.md). Editing or evaluating
this skill is not an invocation to advance the compiler.

Prepare and validate before entering the queue. Follow the protocol for the local
wait, claim, rebase, exact verified candidate, and release on an affected failure.
Do not bypass reservations or hold a lease while doing further development.

Remove completed board acceptance without adding a changelog. If customer
acceptance remains open, retain compact resume evidence on the owning item:
tested revision, repository-relative command and environment/host, observed result,
owning implementation path, and next acceptance. Label an unrun probe explicitly.
A board-only checkpoint is useful only when it changes the next action or corrects
materially stale evidence; an unchanged diagnosis at a newer revision is not an
improvement.

After publication, return to the recorded starting checkout and recheck its
branch and status. If it is still clean and on main, fetch and run
`git merge --ff-only origin/main`, then verify HEAD equals fetched `origin/main`.
This fast-forward creates no merge commit; the candidate rebase above integrates
local work. Never reset, auto-stash, switch branches, or rebase unrelated local
commits to synchronize the starting checkout. If synchronization cannot proceed,
report why and distinguish published remote main from the local checkout state.

Then remove the temporary worktree and local branch created by this invocation.
From the starting checkout, confirm the exact worktree path, that it is unlocked
and clean (including untracked files), and that its HEAD is an ancestor of
fetched `origin/main`. Stop any processes this invocation started there, run
`git worktree remove <path>`, then `git branch -d <branch>` if it has a branch.
Do not force removal or branch deletion. Preserve pre-existing worktrees, the
starting checkout, and anything dirty, locked, unpublished, or still in use;
report any retained temporary worktree and the reason. Verify the removed path
is absent from `git worktree list` and the branch is gone before reporting cleanup.

## Measure the cycle

Record the start time before orientation and include one compact measurement
line in the final report: customer, starting and resulting revisions, acceptance
before/after, elapsed seconds, validation wall time, and agents actually used.
Use tool timestamps and command durations; identify overlapping checks rather
than summing them as wall time. Include run-attributable tokens or cost only when
the runtime exposes them, including delegated usage or explicitly labeling its
absence. Unavailable measurements are unknown, not zero; account-wide usage is
not this run's cost. Keep these records in the conversation's run reports, not
the execution boards or a new telemetry service.

After five measured advances in the continuing conversation, compare customer
progress, elapsed/validation time, and available usage before proposing another
workflow change. Include blocked and paused attempts so the comparison does not
count only successes. Use existing reports; if earlier records are unavailable,
state the smaller sample. Do not claim a savings percentage without comparable
baseline measurements. A scheduled invocation uses this same skill and its
task-local pauses; scheduling alone does not override a pause or expand the
user's allowed scope. Independent task selection within that scope remains required.

## When the slice cannot close

When existing design answers the question, decompose cross-stage work into the
smallest dependency that advances observable behavior or a named proof obligation
for the same customer. Scaffolding and relaxed admission guards do not qualify.
Engineering difficulty alone is not an owner decision.

For a blocker, name the attempted slice, missing contract or unavailable dependency,
why existing mechanisms cannot deliver acceptance, and the next executable step.
For a scope pause, cite the applicable AGENTS.md checkpoint, preserve useful work,
and record why that strategy cannot continue. Do not evade it by changing helpers
or manufacturing an owner question. Actual language/architecture decisions follow
`OWNER_QUESTIONS.md` and the ratified decision process; reference the dependency
on the owning task and continue with independent actionable work. Ordinary
engineering choices remain the agent's responsibility.

Stop without an improvement only when no actionable work remains within the user's
allowed scope or an unavailable operational prerequisite prevents proceeding.
Report the candidates considered, concrete blockers, and next input needed; one
blocked task or a reread of an existing pause is not sufficient evidence. Do not
run broad suites merely to establish this selection result. For a restricted
assignment, explain the restriction rather than silently choosing another board
or customer. A task-local pause is not a veto on independent work.

Report customer behavior, added or removed complexity, commit/publication state,
checks actually run, remaining dependencies, and unrelated failures. Include the
cycle measurements above. Stop after this bounded improvement; do not turn it into
an open-ended repair or full-health campaign.

For skill maintenance, use the [evaluation guide](evals/README.md).
