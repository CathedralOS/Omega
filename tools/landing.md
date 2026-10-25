# Landing on main

Use a pull request and GitHub's native merge queue. The submitter validates the
change; GitHub integrates it with current main, checks that candidate, and
publishes it. There is no local publication lease, build receipt, or repeated
author rebase/review merely because main advanced.

## Submit a finished change

Work in an isolated worktree. Finish focused checks and any explicitly required
review on the submitted source. Record the actual revision, commands, results,
known limitations and claim ticket in the PR. Required review follows changes to
meaning, not each queue-generated SHA. Resolve actual conflicts or failed checks;
do not repeatedly refresh an otherwise eligible PR against moving main.

Python 3, Git and an authenticated GitHub CLI (`gh`) work on Windows, macOS and
Linux. Commands below use `python`; use `python3` if that is its installed name.
Choose a unique `work/NAME` branch and replace `NUMBER` and `FULL_HEAD_SHA` with
the PR number and the full output of `git rev-parse HEAD`:

```text
git status --short
git rev-parse HEAD
git push origin HEAD:refs/heads/work/NAME
gh pr create --base main --head work/NAME --title "lane: finished outcome" --body-file PR_BODY
gh pr checks NUMBER --required --watch
python tools/landing.py enqueue NUMBER --expected-head FULL_HEAD_SHA
python tools/landing.py wait NUMBER --expected-head FULL_HEAD_SHA --wait-seconds 60
```

`PR_BODY` is a file containing the change's explanation and focused evidence.
Include actual agent/model provenance there when known, not in commit trailers.
GitHub records the submitter and publication without a separate notes push.
An existing PR for the owned branch is reused, not duplicated. Do not include
unrelated local commits. Keep the candidate clean and committed before push.
The helper must come from a tree that carries this cutover; a checkout that
predates it still has the retired lease CLI, whose `enqueue` rejects
`NUMBER --expected-head`.

The enqueue helper uses GitHub's native `enqueuePullRequest` mutation, fenced by
the expected source head. It does not enable repository-wide auto-merge, bypass
checks, jump the queue, push, rebase or compile. GitHub refuses ineligible PRs.
The UI's **Merge when ready / Add to merge queue** route is also supported.
`gh pr merge` may try to enable auto-merge even for an eligible PR; use the
documented helper instead of changing repository settings.

`wait` returns 0 only for MERGED, 1 for a closed/conflicting/unqueued PR or an
error, and 2 while still queued at its deadline. A timeout is not a failed
compile and does not remove the PR. Continue bounded observation without
holding a local build slot. Inspect actual failed run logs before changing
source or re-enqueueing. An uncertain enqueue response calls for `status`,
not another mutation.

```text
python tools/landing.py status
python tools/landing.py status NUMBER
gh pr checks NUMBER
gh run view RUN_ID --log-failed
```

Queue status returns up to 100 entries and reports `hasNextPage`; never infer
absence from a truncated result. All operations target the inspected remote's
single GitHub push destination. `--repository PATH` selects another worktree.

## Admission is not release qualification

The only required status is `compiler-check` from GitHub Actions. It executes
`cargo check --locked -p omega` on Linux for PR and `merge_group` candidates.
There are no path skips and no required full suite, customer boot, platform
matrix or reviewer approval count. Focused author validation remains necessary
to establish the change's behavior, including independent-workspace checks when
affected; the small integration gate cannot establish runtime correctness.

Main requires the queue, a PR, the check, linear history, and no force pushes or
deletion. No actor has a bypass. The queue squash-merges one PR per published
commit, so intermediate source checkpoints do not become unqualified main
commits. Keep PR titles in the repository's `lane: statement` form.

Source HEAD, temporary integration SHA and published commit are distinct
identities. Never relabel historical runtime observations as runs of a new
compiler. A queue-created SHA alone does not require fresh author tests or
review. Source edits, conflict resolutions and demonstrated changed assumptions
do require affected validation. See [testing](testing.md#main-change-validation).

The independent **Compiler regression** workflow runs executable Linux
native/Terminal smoke and rejection controls after merge and nightly. It
finishes its active run and coalesces pending pushes to the newest revision.
Canceled superseded runs are unmeasured, not green. Its failures are visible CI
failures, not admission failures retroactively assigned to every queued author.
The author of the regressing PR owns repair or revert; until attributed, the
first observing session records the failing run and takes triage ownership in
claims. Prefer a focused repair; revert through the queue when repair is not
promptly tractable. Do not disable the check, rewrite its oracle or call it green.
Full release and other-host qualification remain separate. This initial hosted
regression set is not full corpus, cross-host or Cathedral boot acceptance.

## Confirm publication and clean up

An open or queued PR is not landed. Read its MERGED state, exact `headRefOid` and
`mergeCommit.oid`; verify the source head is the one submitted. Fetch main and
verify the **published** commit is an ancestor of fetched main:

```text
python tools/landing.py status NUMBER
git fetch origin main
git merge-base --is-ancestor PUBLISHED_SHA origin/main
```

Squash publication means the local source HEAD need not be an ancestor of main.
The merged PR binds that source to publication. Preserve both IDs and the PR/run
URLs. Only then release the work claim and remove task-owned, clean worktrees
and branches if no processes or unique evidence still need them. Inspect status
and ensure HEAD still equals the merged PR head before removal. Never clean
another session's work, shared caches, dirty trees or unsubmitted later commits.
For a squash-landed branch, deletion may need `git branch -D` after those checks;
failure of `git branch -d` alone is not proof the work is unpublished.

Fast-forward a clean starting checkout on main with `git merge --ff-only
origin/main`. Do not reset, stash or overwrite a dirty/non-main starting checkout.

## Migrating an old session

The old `enqueue --ticket / claim / verify / publish` commands and `lane_land.py`
are retired. Preserve existing source, tests and qualification artifacts. Fetch
main and read these instructions at the next safe checkpoint; submit that
finished source as a PR. Do not rerun every check just to obtain a retired build
receipt, restart an outcome, or force-push main after a ruleset refusal.
The old `refs/coordination/omega-landing/main` is historical metadata, not an
active queue. Claims remain the separate advisory work registry.

Production rules are in [ci/main-ruleset.json](ci/main-ruleset.json); CI scope,
rollout observations and the isolated pilot are documented in [ci/README.md](ci/README.md).
