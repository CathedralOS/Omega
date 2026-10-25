# Compiler merge queue pilot

The owner authorized an isolated GitHub-native queue pilot on 2026-10-24.
`integration-pilot` is its base; `ci-pilot/*` branches and PRs are permitted
only for this experiment. Main continues through `tools/landing.py`. Do not
point agents at the new route until the acceptance below is demonstrated.

The native queue, not an agent, combines an eligible PR with the current base,
runs CI on the resulting candidate, and publishes it. The pilot admits one
integration build at a time, uses rebase merges, and has no bypass actors.
Later submissions do not take over an active candidate's publication slot.
Genuine conflicts or failing checks return to the author; they are not retried
indefinitely or promoted by treating missing evidence as success.

## CI contract

`.github/workflows/compiler-pilot.yml` runs on pilot PRs, pilot `merge_group`
events, and manual dispatch. Default checkout semantics select the PR merge
candidate or merge-group commit, never just the contributor's head. There are
no path skips that leave a required status absent. Permissions are read-only,
checkout credentials are not retained, and untrusted code runs on an ephemeral
GitHub-hosted Linux runner, not the owner's machine. No repository secrets are
passed to compiler tests.

The job builds the optimized CLI and corpus runner, then uses the existing
corpus gate for three positives, four intended rejections, and actual native
and Terminal execution of the three positives. `compiler_pilot.py` also requires
the exact observation roster, source revision, clean tree, expected rejection
fragments, and both-engine agreement. Golden movement alone may pass; source
rejection, missing execution, wrong results, missing rows, and infrastructure
failures cannot. Nothing records new goldens.

The report and exact corpus runner are uploaded under `compiler-pilot-SHA-attempt`.
`summary.json` separates build and test timings and names the producer digest.
Cargo dependencies/build outputs are cached, but Cargo still checks freshness
on every candidate. A cache hit never substitutes for executing the tests.
The job has a 30-minute timeout; a cold build is measured separately from warm
queue behavior. PR revisions cancel their superseded PR jobs; queue jobs do not
cancel one another.

This is a Linux smoke gate for proving integration, **not** the full compiler
acceptance, cross-host coverage, or Cathedral UEFI acceptance. Existing
assignment-specific tests and required semantic review remain the author's
responsibility. Do not silently replace them with these three programs.

## Activate and exercise the isolated queue

Repository admin access is needed for the ruleset; write access suffices for
submitting PRs once configured. The repository is public and organization-owned,
which is eligible for GitHub's native queue. See GitHub's
[merge queue documentation](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue).

1. Land the workflow/tooling through the existing main publication path.
2. Create `integration-pilot` from that published commit. Do not overwrite an
   existing pilot branch. Dispatch the workflow there once to measure the cold
   baseline and populate a cache visible to its PRs and merge groups.
3. Apply `pilot-ruleset.json` with the repository rulesets API. Inspect an
   existing named ruleset before updating it; do not create duplicates. The
   required `compiler-pilot` check is bound to the GitHub Actions application
   (ID 15368), not an arbitrary user-authored status.
4. Create two independent harmless PRs and one PR that deliberately changes a
   smoke fixture's expected exit to the wrong value. These changes stay on the
   isolated pilot branches; the bad oracle must never reach main.
5. Observe the two good PRs' checks, enqueue them normally, and observe their
   `merge_group` checks and resulting pilot history. Do not rebase their source
   branches when the first merges. The bad PR must report its actual runtime
   mismatch and remain unmerged. Do not use an admin merge override.

Useful commands (Python 3 and GitHub CLI work on macOS and Windows):

```text
python tools/tests/test_compiler_pilot.py -v
python tools/ci/compiler_pilot.py
gh workflow run compiler-pilot.yml --ref integration-pilot
gh api --method POST repos/CathedralOS/Omega/rulesets --input tools/ci/pilot-ruleset.json
gh pr create --base integration-pilot --head ci-pilot/CHANGE
gh pr checks NUMBER
gh pr merge NUMBER --match-head-commit FULL_HEAD_SHA
gh run list --workflow compiler-pilot.yml
```

## Acceptance and cutover

Record the real run URLs, candidate SHAs, merge-group events, published commits,
failed-control diagnostic, and cold/warm durations in this document after the
experiment. Local adapter tests alone do not prove a working queue.

Cutover requires both good PRs to merge through successful integration checks,
the bad oracle to prevent merging, and the second PR to survive the base moving
without an author-side rebase. Also demonstrate a subsequent submission reuses
the warm base cache. Record any extra queue rebuild rather than claiming exactly
one build when GitHub legitimately recreates an affected group.

Before moving agents, select the required production checks from actual compiler
and customer acceptance, provide supported-host coverage, update the canonical
landing instructions/skills, and retire the superseded main publication loop.
Keep semantic review proportional to the change; do not recreate the old
receipt/rebase loop around GitHub's tested integration candidate.

Rollback leaves main alone: close the pilot PRs and disable only the named pilot
ruleset/workflow. Keep run artifacts and observations. No main branch protection,
release workflow, billing setting, or host permission is changed by this pilot.
