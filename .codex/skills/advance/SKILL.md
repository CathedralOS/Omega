---
name: advance
description: Drive the Omega Rust reference compiler toward completion by working the execution boards (TASKS.md, TASKS_BOOTSTRAP.md, TASKS_OPTIMIZER.md) or obvious unspecified work in the same direction. Use when asked to advance, continue, drive, or make progress on the compiler, to work the boards, or to pick up the next task. Not for a specific named bug, a question about existing code, or a review.
---

# Advance the Omega compiler

One invocation delivers one bounded compiler improvement, or an evidence-backed
scope pause when the existing plan no longer justifies implementation. For an
improvement: choose it, reproduce it, implement it, validate its affected
behavior, and land it. Repository-wide health is a separate job. Follow
`AGENTS.md` for ownership, commands, validation scope, board hygiene, and publication.

Reconfirming a known failure or refreshing its board revision is verification-only
work, not a compiler improvement. Count it as an incomplete advance in the run
report. Publish a board-only checkpoint only when it changes the next action or
corrects materially stale evidence; a newer revision with the same diagnosis
usually belongs in the conversation.

## Choose a useful slice

Record the start time, starting checkout path, and branch. Fetch main and inspect
status;
when that checkout is clean and on main, run `git merge --ff-only origin/main`
before reading its boards. Fetch alone does not update checked-out files.
Otherwise preserve it and read the boards from an isolated worktree based on
fetched `origin/main`. Inspect recent commits in the relevant lane. An unnamed
invocation may choose from `TASKS.md`, `TASKS_BOOTSTRAP.md`, and
`TASKS_OPTIMIZER.md`. Leave other sessions' work alone.

For an unnamed continuation, resume the previous concrete customer program or
required proof obligation from the conversation and its owning board item.
Keep that customer across invocations until its acceptance passes, a real
dependency blocks it, the scope checkpoint requires a pause, or the user changes
priority. If another session owns the next change, do not duplicate it. When
continuity is unavailable, choose from the boards and state the customer once.
Do not switch lanes merely because another helper is easier to finish.

### Work from a should-be-working example

Use examples to choose the next problem and verify progress. The language and
architecture contracts govern the solution and its full acceptance criteria.
An example exposes a missing capability; it does not define that capability's
limits or justify sample-specific semantics, intrinsics, or compiler paths.
Implement the smallest design-consistent capability needed by the example,
without adding speculative generality. A passing example is a milestone, not
proof that the capability or compiler is complete; retain the broader contract's
required cases and evidence.

When the route exposes an unsettled language or architecture choice, use the
existing `OWNER_QUESTIONS.md` criteria before implementing that choice. When the
design already answers it, continue implementation; difficulty or a cross-stage
dependency alone is not a design question.

Use the customer's actual command as the outer loop, including the shipped CLI,
package preparation, publication, and execution. A compiler-library test is a
useful inner loop but may bypass those dependencies. Trace backward from the
expected output and exit through the real producer/consumer route. Keep the
route in the sample's existing README or owning design document, with the
current first failure on its existing board item; do not add a parallel tracker.

Distinguish the first observed failure from downstream gaps found by reading
code. Record which command reached which boundary and where it left artifacts.
After each milestone, rerun the outer command when its inputs changed and report
whether its failure moved. A passing helper with an unchanged customer failure
is dependency progress, not a working example. Never simplify the customer's
program or bypass checking merely to move the marker.

Use existing failure logs and board evidence to rank useful work. Do not start
an unfiltered corpus run just to choose a task. If evidence is stale, probe one
representative fixture. Prefer a bounded improvement with observable acceptance
over the largest blocker when that blocker needs a much broader implementation.

Run the smallest relevant program or filtered test before editing. Confirm the
actual failure and choose an acceptance condition that this iteration can meet.
Reaching checked trees is a valid milestone when native production remains
blocked; report that boundary accurately. If a probe passes, remove the stale
board claim. Do not spend the session repeatedly scouting larger alternatives.

## Work in isolation

Before implementation, apply [scope checkpoints](../../../AGENTS.md#scope-checkpoints).
Read recent milestones for the same customer, not just the current board line.
State the customer, missing dependency, smallest useful outcome, and simpler
alternative before adding machinery. Bootstrap work must also read
[whole-chain minimization](../../../wiki/design_briefs/bootstrap_minimization.md).
Do not turn an unavailable downstream compiler into speculative upstream
generality, or treat private resource limits as immutable language laws.

The infrastructure-only checkpoint spans invocations and delegated work. When
it requires a pause, report evidence and ask for scope/prioritization direction;
do not evade it by selecting another helper, adding tests, or calling the audit
an owner-blocked language issue. Resume feature work only after that direction.
At handoff, report customer progress and added/removed complexity, not just
commit and test counts.

Use an isolated worktree under `<repository>/.codex/worktrees/<short-name>`.
That directory is repository-local and ignored; do not place advance worktrees
under a user-level Codex directory. Keep the name short. Generated linker
paths can exceed MAX_PATH even when the repository path looks reasonable, so
measure the failing path before changing compiler architecture or relocating
the worktree.

A single bounded fix normally needs one agent. Delegate only when independent
useful work can shorten the critical path enough to justify briefing, review,
and integration; follow the delegation steps below. Avoid concurrent builds on
the same host and duplicate full validation passes; check for existing runs and reuse valid
results before launching another check.

Trace the shared implementation and its callers, then make the smallest change
that fixes the behavior. Preserve the Psi/Omega firewall and proof, custody, and
trust checks. Do not add a new representation or provider mechanism when the
existing path can carry the required behavior.

## Delegate a bounded assignment

Apply [AGENTS.md](../../../AGENTS.md#agent-delegation) for model routing and
ownership. Refine the existing board item only where its scope, design rationale,
dependencies, or acceptance are insufficient for assignment. Keep transient
worker IDs and progress in the conversation, not on the execution boards.

Slice selection, cross-stage diagnosis, and deciding whether implementation can
proceed require Astra judgment. Luna may execute settled edits or specified
checks; an unexpected dependency returns to Astra with the failing command,
revision, owning code, and unanswered implementation question. If that routing
is unavailable, report the limitation and incomplete outcome; do not turn it
into an owner decision or claim that no safe implementation exists.

Give each worker a self-contained assignment with the context required by
AGENTS.md, its worktree/revision, and whether it owns implementation, read-only
verification, or investigation. Resolve dependencies before assigning dependent
implementation; independent inspection can proceed while a build runs. If a
worker cannot call for stronger reasoning itself, it returns concrete evidence
to the coordinator for reassignment.

Call the available spawn tool and retain its returned agent ID before announcing
dispatch. If no callable tool is available, report that limitation and continue
locally where possible; a proposed assignment or terminal command is not a
launched worker. Monitor actual workers through completion or a concrete blocker,
respond to findings, and reconcile overlapping changes before integration.

Inspect each returned diff, command result, or artifact against its acceptance.
Reports identify the revision, host, exact command and exit status, failures,
remaining dependencies, and whether work continues. Label the outcome as
implementation completed, verification-only completed, diagnosed blocker, or
work continuing. Verification must cover the assigned objective: package format
or prerequisite checks do not complete assigned workspace tests or establish a
baseline. Cancelled runs are not failed validation. Cross-emission is not target
runtime validation; execute on available real target hosts and name missing hosts.

For an assigned fix, diagnosis is an intermediate result. Continue with the same
worker or hand off its evidence when the existing design permits implementation.
If a dependency makes the bounded fix impossible, report that precise boundary
and the next acceptance condition under "When the slice cannot close" below.
Keep duplicate findings on one owning board item, and use the existing owner
question process only for unresolved owner-level decisions. The coordinator
reports the inspected outcome and remaining work, not merely worker confidence.

## Validate the change

Use the scoped validation policy in `AGENTS.md`. Before running checks, name the
behavior being established and select the relevant tests. Routine advancement
does not require a fresh full baseline, even in a new worktree.

- Reuse a regression where possible. For a bug fix, observe the relevant check
  fail without the fix and pass with it; register new corpus fixtures so they
  actually run. One meaningful red/green check is enough; do not duplicate it
  at every layer merely to accumulate evidence.
- Read the harness before filtering. `OMEGA_PASS_CANARY_FILTER` and
  `OMEGA_FAIL_CANARY_FILTER` select fixture paths; nextest filters select Rust
  test names. Use `--no-fail-fast` and report platform skips explicitly.
- Run affected crate checks and relevant integration tests. Include architecture
  checks when ownership, dependencies, representations, or their source-reading
  rules are affected. Do not run workspace check in every edit loop.
- Reuse successful checks on unchanged inputs. When a verified base exists,
  inspect `tools/test_affected.py --base VERIFIED_COMMIT --plan` before running
  it. A conservative all-library fallback is a selection limitation, not by
  itself a reason to turn a narrow task into a full-baseline campaign. Review
  the actual inputs and justify a manual scoped selection when appropriate.
- Full corpus and full workspace runs belong to explicit health/release work
  or changes whose impact cannot reasonably be bounded. State the reason before
  starting one. Never launch them automatically for ranking or repeat them
  simply because another commit arrived on main.

A failure is evidence to attribute, not permission to expand the task. Use a
focused baseline comparison or dependency/source-reader evidence. Fix failures
caused by the change. Record confirmed unrelated failures with their command,
revision, and evidence; they do not block this change and are not this task's
repair queue. An unexplained failure in an affected path still blocks landing;
preserve the checkpoint and report the uncertainty rather than claiming success.
Do not repeatedly rerun a broad suite to attribute one failure.

## Land the checkpoint

Invoking advance authorizes committing and publishing the bounded improvement
to main through `tools/landing.py`. Keep unrelated fixes out of the checkpoint.
Update the owning board only for remaining execution state; remove completed
acceptance conditions without adding a test-count or history log.

Before landing, rerun the selected customer probe when its inputs changed.
If customer acceptance remains open, replace its next-step evidence in the
owning board item with the tested revision, repository-relative command (with
any required environment settings and host), observed diagnostic or result,
owning implementation path, and next acceptance condition. Keep this compact;
link existing design detail instead of copying it. A passing helper test does
not establish customer acceptance. If the next probe was not run, say so rather
than inventing the next failure. This is current resume state, not a run history.

Prepare and validate before entering the landing queue. Use the local FIFO wait,
claim the nonrenewable lease, and rebase onto its returned base. Inspect incoming
changes and rerun only checks whose inputs or acceptance evidence changed. An
unchanged candidate does not need a second full pass merely for publication.
Publish the exact candidate whose applicable checks are established; never bypass
reservation ownership or push directly to main. If more development or lengthy
validation is needed, release the reservation and finish it outside the queue.

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

Report the resulting behavior, commit, checks actually run, remaining limitations,
and any unrelated failures. Do not imply a scoped pass establishes whole-repository
health. Stop after the bounded improvement lands; broader validation and unrelated
repairs require a separate task.

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
baseline measurements. A scheduled invocation uses this same skill and its scope
pauses; scheduling alone does not authorize a new lane or override a pause.

## When the slice cannot close

Engineering difficulty is not an owner decision. Design questions belong in
`OWNER_QUESTIONS.md` under its existing criteria. A dependency spanning several
stages is work to decompose, not by itself a reason to stop. When the design
already answers the question and no scope checkpoint requires a pause,
identify and implement the smallest dependency
slice with observable behavior or a named proof obligation as acceptance.
Keep it tied to the same customer; scaffolding and relaxed admission guards
alone do not qualify.

Before reporting that no bounded slice can close, name the concrete slice
considered, the missing contract or unavailable dependency preventing it, and
why existing mechanisms cannot deliver its acceptance. Distinguish a diagnosed
blocker from a scope pause: the latter cites the applicable AGENTS.md checkpoint,
explains why the plan no longer earns its cost, and asks for prioritization
direction. Preserve useful work and the next acceptance condition. Do not turn
one invocation into an open-ended sequence of new implementations and gate repairs.
