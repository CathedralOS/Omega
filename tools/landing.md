# Landing on main

Finish the change, run its focused checks, and squash-merge its pull request
when `admission` passes. The session doing the work owns getting it onto main
and confirming that it arrived; main is checked after merge.

Follow [AGENTS.md](../AGENTS.md#sessions-and-subagents) for ownership and the
[CI rebuild plan](../wiki/drafts/designs/ci_rebuild_plan.md) for the direction of
CI work; implemented behavior is in [.github/workflows/](../.github/workflows/).
There is no merge queue. Workers review and land their own work; a
coordinator may review a change without becoming an additional approval gate.

## Prepare and publish

Use an isolated detached `worker-N` pool slot, acquired with
`mbx xtask worktree acquire <lane>-<topic>`. Keep its warm build cache. Never
develop in the primary checkout or create local named branches.

Run the checks that establish the changed behavior. Compiler changes need the
affected crates' unit tests and Clippy, plus one focused corpus run over the
known affected rows and samples. Documentation changes need a consistency and
link check. Do not expand these into a full corpus or workspace sweep. Authors
do not run `corpus --record` or edit `tests/omega/corpus_outcomes.txt`; post-merge
verification owns the ledger.

Review the actual diff using
[coordinator-review](../.agents/skills/coordinator-review/SKILL.md), including
when working alone. Preserve soundness, the intended program and its rejection
controls. Follow the assigned acceptance and any explicit review requirement.
Do not wait for a separate coordinator approval by default.

Write the description using [Explain the change](../AGENTS.md#explain-the-change):
start with the problem and resulting behavior in ordinary language, then explain
the mechanism, validation and material limits. Generate the required `Agent:`
line with `mbx xtask provenance` and end both the commit body and PR description
with it. Compiler-lane descriptions also need the prescribed `Gate:` line.

Keep the source committed and inspect `git status --short` before pushing. Use a
UTF-8 file for multiline descriptions; do not interpolate prose into shell
commands. For example:

```text
git rev-parse HEAD
git push origin HEAD:refs/heads/work/NAME
mbx xtask check-rules --base origin/main --title "lane: statement" --body-file PR_BODY
gh pr create --base main --head work/NAME --title "lane: statement" --body-file PR_BODY
```

`check-rules --base` names the PR target or an ancestor commit. Diff rules use
its unique merge base with `HEAD`, resolved once to an immutable commit, so
later additions on the target branch stay outside the topic's changes. An
ancestor base, including admission's `HEAD^1` merge parent, keeps the same scope.
Missing, unrelated or ambiguous base history is an error; fetch the needed
history or name an ancestor commit. Worktree-wide ratchets and title/body gates
retain their existing inputs.

Open the PR ready when the work is complete. Reuse an existing PR for the same
branch; mark an existing draft ready with `gh pr ready NUMBER` when it is
finished. Record the submitted revision, actual commands and results, known
limits and claim ticket. Keep evidence in the PR and existing artifacts rather
than creating releases or tags for it.

## Squash-merge after admission

`admission` checks repository rules and formatting. It does not build the
compiler, run Clippy or establish runtime behavior, so the author must finish
the focused validation before merging.

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
Check admission first and read back the result. An uncertain response calls for
observation before another merge request.

Resolve actual conflicts or failed checks. Do not rebase, rebuild or repeat
validation solely because main moved. Source changes, conflict resolutions or
changed assumptions require the affected checks again. Never use administrator
bypass or change repository settings to land a PR.

## Confirm publication and continue

A successful merge request is followed by remote verification:

```text
gh pr view NUMBER --json state,headRefOid,mergeCommit,url
git fetch origin main
git merge-base --is-ancestor PUBLISHED_SHA origin/main
```

Confirm `MERGED`, the submitted `headRefOid`, and the published squash commit's
presence on fetched main. Preserve both source and published IDs with the PR
URL. Squashing means the local source head need not be an ancestor of main.
Only then note the result and release the work claim.

Observe the current `post-merge-check` workflow for main verification. It batches
pending pushes, so a superseded canceled run is unmeasured. Inspect real failures
and repair regressions through a focused PR. Keep historical observations tied
to the source and host that produced them; publication does not turn those into
fresh runtime results on main.

Continue the parent capability's next remaining requirement on the warm
worktree. Preserve dirty or unsubmitted work, retained producers and evidence;
never clean another session's files or shared caches.

## Resuming an older session

Read current [AGENTS.md](../AGENTS.md) at a safe checkpoint and preserve existing
source, tests and evidence. The old `compiler-check`, `workspace-clippy`,
`corpus-gate`, Post-merge smoke and merge queue are retired. Do not wait for,
dispatch or recreate them, and do not use `xtask land` or the old queue helpers.
Finish the authorized work through admission and a squash merge instead.
