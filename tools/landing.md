# Landing on main

Use a pull request and GitHub's native merge queue. The submitter validates the
change; GitHub integrates it with current main, checks that candidate, and
publishes it. There is no local publication lease, build receipt, or repeated
author rebase/review merely because main advanced.

## Open a draft on the first push

Work in an isolated detached-HEAD worktree (`git worktree add --detach .agents/worktree/<short-name> origin/main`).
Never develop or edit files in the primary repository checkout, and never create local named branches.
The first push of a branch opens its pull request as a draft. Later pushes update
that same draft. `compiler-check` runs on the draft. A draft cannot enter the
merge queue, and it is not a place to leave the work: the session that opened
it still lands it, and does not leave it across unrelated landings.

```text
git status --short
git rev-parse HEAD
git push origin HEAD:refs/heads/work/NAME
python tools/check_rules.py --title "lane: statement" --body-file PR_BODY
gh pr create --draft --base main --head work/NAME --title "lane: statement" --body-file PR_BODY
```

Reuse the existing pull request for the branch. Do not open a second one.
Record the actual revision, commands, results, known limitations and claim ticket in the PR.
Keep routine validation evidence in the PR and existing CI artifacts. Do not
create GitHub releases or tags to host evidence; release publication is separate
product-distribution work and requires an explicit user request. Existing
historical evidence links remain valid and need not be removed.

## Mark ready and enqueue when the change can merge

Finish focused checks and any explicitly required review on the submitted source.
Use the [merge validation budget](testing.md#merge-validation-budget): the
submitter chooses checks for the changed behavior and reuses unchanged results.
Ordinary progress PRs do not wait for full Product/self-host builds, release
matrices, or a new pinned qualification packet. Run those only when the assigned
acceptance explicitly requires them. Stop expanding validation once the scoped
checks pass; keep any remaining extended validation accurately reported.
Never chase main by rebasing, rebuilding, or re-running gates: once the pull
request is ready, GitHub Actions CI executes the integration build and checks in
the merge queue. Required review follows changes to meaning, not each
queue-generated SHA. Resolve actual conflicts or failed checks; do not
repeatedly refresh an otherwise eligible PR against moving main.

Python 3, Git and an authenticated GitHub CLI (`gh`) work on Windows, macOS and
Linux. Commands below use `python`; use `python3` if that is its installed name.

Claude Code cloud sessions reach GitHub through a proxy that refuses GraphQL.
There `landing.py` reads the PR and `compiler-check` through REST and uses the
proxy's REST routes to mark the PR ready and request auto-merge, which on this
merge-queue branch queues a green PR at once. REST exposes no queue object, so
a queue entry is read from its active `gh-readonly-queue/main/pr-<n>-*` CI run.
The auto-merge route cannot fence on the head, so `land` checks the head
immediately before it. Hosts where GraphQL works are unchanged.

When the work is finished and its last commit is pushed, run one command in the
foreground. Replace `NUMBER` and `FULL_HEAD_SHA` with the PR number and the full
output of `git rev-parse HEAD`:

```text
python tools/landing.py land NUMBER --expected-head FULL_HEAD_SHA
```

`land` hands the wait to GitHub. It refuses a red `compiler-check`, a merge
conflict, a closed PR or a head that differs from `--expected-head`, without
changing anything. Otherwise it runs `gh pr ready` (skipped if the PR is already
ready) and either enqueues at once, when the check on that head is already
green, or asks the queue to take the PR when the check passes (GitHub's
auto-merge request, which on this merge-queue branch means "add to the queue when
ready"; the repository allows it since 2026-11-01, and it bypasses nothing). It
returns in seconds and prints the PR, exit 0. This registers native asynchronous
intent, not a confirmed queue entry or publication. GitHub normally queues it
after the check passes, so continuous polling is unnecessary. If a later status
shows a green head still armed without a queue entry, rerun the same fenced
`land` command: it rechecks that head and uses the ordinary enqueue path. No
additional CI permissions or workflow are needed for this foreground recovery.

The [merge retrospective (#1030)](https://github.com/CathedralOS/Omega/pull/1030)
tracks the earlier merge-delay problem. The session for
[#1451](https://github.com/CathedralOS/Omega/pull/1451) later observed a green,
clean, armed PR without a queue entry: repeating `land` returned it unchanged,
while explicit enqueue advanced it. Native intent also queued
[#1446](https://github.com/CathedralOS/Omega/pull/1446) and
[#1452](https://github.com/CathedralOS/Omega/pull/1452) normally. The recovery
above fixes the client's later-call ordering, without removing native intent
or asserting that GitHub never fulfills it.

Arming is the landing decision. A later push to the same PR retains that intent
for the new head, so push only finished work after `land`, or disarm first
with `gh pr merge NUMBER --disable-auto`. A red check leaves the PR out of the
queue: inspect the failed run (`gh pr checks NUMBER`, `gh run view RUN_ID
--log-failed`), fix the source and push; the request stays armed for the new
head.

Verify the landing on a later turn with `python tools/landing.py status NUMBER`
(`state` MERGED and `mergeCommit`), or, where the host lets a command run in the
background, add `--wait-seconds N` (at most 86400) so `land` also waits: it exits
0 for MERGED, 2 at the deadline, and 1 for a red check, a conflict, a closed PR
or removal from the queue. A PR whose newest queue run passed but which still
reads OPEN with no queue entry is merging, not removed: GitHub flips its state
after main moves, so `land` keeps reading it for up to five minutes. Waiting
never re-enqueues a removed entry. A later
`land` leaves merged or queued PRs untouched, retains an armed pending request,
and enqueues an already-armed green head. An armed request alone does not
guarantee unattended publication; verify the final state.

Write `PR_BODY` using [Explain the change](../AGENTS.md#explain-the-change).
Pass multiline prose through a UTF-8 file with `--body-file`, as above, or a
structured API body field. Do not interpolate it into shell command text:
backticks and escape sequences can execute commands or corrupt identifiers.
Check its opening against the submitted diff before creating the PR or marking
it ready; update the whole explanation when the scope changes. Summarize focused
validation after the behavior and mechanism, then link or include the detailed
records. End it with the `Agent:` line that [Explain the change](../AGENTS.md#explain-the-change)
requires; `compiler-check` refuses a description without one. Keep provenance out of commit trailers.
GitHub records the submitter and publication without a separate notes push.
An existing PR for the owned branch is reused, not duplicated. Do not include
unrelated local commits. Keep the candidate clean and committed before push.

`land` is the check and ready steps, then `enqueue` or the queue request, then
optionally `wait`; `enqueue` and `wait` remain for observing or resuming by hand. The enqueue helper uses GitHub's
native `enqueuePullRequest` mutation, and the queue request is
`enablePullRequestAutoMerge`; both are fenced by the expected source head. Neither
bypasses checks, jumps the queue, pushes, rebases or compiles. GitHub refuses
ineligible PRs. The UI's **Merge when ready / Add to merge queue** route is also
supported.

`wait` returns 0 only for MERGED, 1 for a closed/conflicting/unqueued PR or an
error, and 2 while still queued at its deadline. A timeout is not a failed
compile and does not remove the PR. Continue bounded observation with `land`
or `wait`, without holding a local build slot. Inspect actual failed run logs
before changing source or re-enqueueing. An uncertain enqueue response calls for `status`,
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
`mbx xtask fmt --check` and `mbx clippy --locked -p omega -- -D warnings`
on Linux for PR and `merge_group` candidates, so formatting drift and warnings
in the `omega` crate graph are refused. Run both before pushing.
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

The independent **Post-merge smoke** workflow (`.github/workflows/compiler-regression.yml`)
runs executable Linux native/Terminal smoke and rejection controls after merge
and nightly. It finishes its active run and coalesces pending pushes to the
newest revision. Canceled superseded runs are unmeasured, not green, and an
older failed run is resolved once a newer `main` commit succeeds. Its failures
are visible CI failures, not admission failures retroactively assigned to every
queued author. Sessions working on an active capability or sample ignore
unrelated `main` failures and preserve their warm base. The author of the
regressing PR owns repair or revert; otherwise at most one unassigned session
claims the failure (`REGRESSION-<sha>`) in `claims.py` and drives a focused
repair or revert through the queue. Do not disable the check, rewrite its
oracle or call it green.
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
URLs. Only then release the work claim. Worktrees should generally be retained and
reused as warm build workbenches (`git checkout --detach origin/main`) to accelerate
future compilation. Remove task-owned worktrees (`git worktree remove <path>`) only
when explicitly requested or reclaiming disk space. Because development uses
detached-HEAD worktrees, no local branch deletion is needed. Inspect status and ensure
HEAD still equals the merged PR head before any removal. Never clean another session's
work, shared caches, dirty trees or unsubmitted later commits.

Postlanding verification is read-only PR/commit/CI observation; it never requires
a local rebuild or acceptance rerun. Leave the starting checkout at its recorded
base. Do not reset, stash or overwrite dirty/non-main work.

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
