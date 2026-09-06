---
name: advance
description: Drive the Omega Rust reference compiler toward completion by working the execution boards (TASKS.md, TASKS_BOOTSTRAP.md, TASKS_OPTIMIZER.md) or obvious unspecified work in the same direction. Use when asked to advance, continue, drive, or make progress on the compiler, to work the boards, or to pick up the next task. Not for a specific named bug, a question about existing code, or a review.
---

# Advance the Omega compiler

One invocation delivers one bounded compiler improvement: choose it, reproduce
it, implement it, validate its affected behavior, and land it. Repository-wide
health is a separate job. Follow `AGENTS.md` for ownership, commands, validation
scope, board hygiene, and publication.

## Choose a useful slice

Record the starting checkout path and branch. Fetch main and inspect status;
when that checkout is clean and on main, run `git merge --ff-only origin/main`
before reading its boards. Fetch alone does not update checked-out files.
Otherwise preserve it and read the boards from an isolated worktree based on
fetched `origin/main`. Inspect recent commits in the relevant lane. An unnamed
invocation may choose from `TASKS.md`, `TASKS_BOOTSTRAP.md`, and
`TASKS_OPTIMIZER.md`. Leave other sessions' work alone.

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

Use an isolated worktree with a short path on Windows. Generated linker paths
can exceed MAX_PATH even when the source path looks reasonable. Measure the
failing path before blaming mbx or changing compiler architecture.

A single bounded fix normally needs one agent. Delegate only independent useful
work; agree on disjoint edit ownership and avoid concurrent builds on the same
host. Delegation must not add duplicate full validation passes.

Trace the shared implementation and its callers, then make the smallest change
that fixes the behavior. Preserve the Psi/Omega firewall and proof, custody, and
trust checks. Do not add a new representation or provider mechanism when the
existing path can carry the required behavior.

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

## When the slice cannot close

Engineering difficulty is not an owner decision. Design questions belong in
`OWNER_QUESTIONS.md` under its existing criteria. When the selected slice exposes
a larger dependency, preserve useful work and name the next acceptance condition.
Choose a smaller demonstrable milestone or report the blocker; do not turn one
invocation into an open-ended sequence of new implementations and gate repairs.
