# Landing on main

Finish the change, run its focused checks, and squash-merge its pull request
when `admission` passes. The session that did the work lands it and confirms
it arrived; main is checked after merge.

Follow [AGENTS.md](../../AGENTS.md#sessions-and-subagents) for ownership and the
[CI rebuild plan](../../wiki/drafts/designs/ci_rebuild_plan.md) for the direction of
CI work; implemented behavior is in [.github/workflows/](../../.github/workflows/).
There is no merge queue. Workers review and land their own work; a
coordinator may review a change, but its approval is not required.

## Prepare and publish

Use an isolated detached `worker-N` pool slot, acquired with
`mbx xtask worktree acquire <lane>-<topic>`. Keep its warm build cache. Never
develop in the primary checkout or create local named branches.

Run the checks that prove the changed behavior. Compiler changes need the
affected crates' unit tests and Clippy, plus the tests on the small area around
the change and the known affected samples. Documentation changes need a
consistency and link check. Do not widen these into a whole-workspace run. There
is no fixture corpus to run and no ledger.

Review the actual diff using
[coordinator-review](../../.agents/skills/coordinator-review/SKILL.md), including
when working alone. Keep soundness, the intended program and the programs it must still reject.
Meet the assigned acceptance and any explicit review requirement. Do not wait
for coordinator approval unless told to.

Write the description using [Explain the change](../../AGENTS.md#explain-the-change):
start with the problem and resulting behavior in ordinary language, then explain
the mechanism, validation and material limits. Generate the required `Agent:`
line with `mbx xtask provenance` and end both the commit body and PR description
with it. Compiler-lane descriptions also need `Gate:`, `Spec:` and `Proof:` lines.
The audit covers the worktree's work interval, which starts at acquisition or
at `mbx xtask worktree begin` before the next task. An already active checkout
adopts a provable local start once. Peer fetches, checkpoints and rebases
never move it; a missing or ambiguous start refuses. An interval with no
measurements never quietly becomes a whole-session audit; use
`--full-session` only to count the whole session on purpose.

Commit, then check `git status --short` before pushing. Put
multiline descriptions in a UTF-8 file; do not paste prose into shell
commands. For example:

```text
git rev-parse HEAD
git push origin HEAD:refs/heads/work/NAME
mbx xtask check-rules --base origin/main --title "lane: statement" --body-file PR_BODY
gh pr create --base main --head work/NAME --title "lane: statement" --body-file PR_BODY
```

`check-rules --base` names the PR target or an ancestor commit. Diff rules use
its single merge base with `HEAD`, fixed once, so later commits on the target
branch do not count as the topic's changes. An ancestor base,
including admission's `HEAD^1` merge parent, gives the same scope. Missing,
unrelated or ambiguous base history is an error; fetch the needed history or
name an ancestor commit. Worktree-wide ratchets and title/body checks keep
their inputs.

Open the PR ready when the work is complete. Reuse an existing PR for the same
branch; mark an existing draft ready with `gh pr ready NUMBER` when it is
finished. Record the submitted revision, commands and results, known limits and the
claim ticket. Keep evidence in the PR and existing artifacts;
do not create releases or tags for it.

## Squash-merge after admission

`admission` checks repository rules and formatting, and type-checks the merge
commit (`mbx check --locked --workspace --all-targets`) when Rust or Cargo files
changed. That catches two pull requests whose Rust merges without a Git
conflict but no longer compiles. It does not build the compiler, run Clippy or
test runtime behavior, so the author must finish the focused checks before
merging.

Inspect the checks and submitted head:

```text
gh pr checks NUMBER
gh pr view NUMBER --json state,headRefOid,isDraft,mergeable
```

When `admission` has passed and the PR can merge, squash-merge the exact head
you reviewed. Pass the PR body as the squash commit body so the explanation and
model-audit line survive on main:

```text
gh pr merge NUMBER --squash --match-head-commit FULL_HEAD_SHA --body-file PR_BODY
```

If GraphQL is unavailable, use GitHub's REST merge endpoint through
`gh api --method PUT repos/CathedralOS/Omega/pulls/NUMBER/merge --input MERGE_JSON`.
The JSON file supplies `sha` with the reviewed full head, `merge_method` with
`squash`, the PR title as `commit_title`, and its exact body as `commit_message`.
Check admission first and read back the result. If the response is unclear,
check the PR before another merge request.

Resolve actual conflicts or failed checks. Do not rebase, rebuild or rerun checks
only because main moved. Rerun the affected checks after source changes,
conflict resolutions or changed assumptions. Never use administrator
bypass or change repository settings to land a PR.

## Confirm publication and continue

After a successful merge request, verify the result on the remote:

```text
gh pr view NUMBER --json state,headRefOid,mergeCommit,url
git fetch origin main
git merge-base --is-ancestor PUBLISHED_SHA origin/main
```

Confirm `MERGED`, the submitted `headRefOid`, and the squash commit on fetched
main. Keep the source and published IDs with the PR URL. After a squash, the
local source head need not be an ancestor of main.
Only then note the result and release the work claim.

Watch the `post-merge-check` workflow, which checks main. It batches pending
pushes, so a run canceled for a newer one measured nothing. Investigate real
failures and fix regressions in a focused PR. Keep older results tied to
the source and host that produced them; merging does not make them fresh
results on main.

Continue the same feature's next requirement on the warm worktree. Keep uncommitted or unsubmitted work, saved producers and evidence;
never clean another session's files or shared caches.

## Resuming an older session

Read the current [AGENTS.md](../../AGENTS.md) at a safe checkpoint and keep the
existing source, tests and evidence. The old `compiler-check`, `workspace-clippy`,
`corpus-gate`, Post-merge smoke and merge queue are retired. Do not wait for,
dispatch or recreate them, and do not use `xtask land` or the old queue helpers.
Finish the authorized work through admission and a squash merge instead.
