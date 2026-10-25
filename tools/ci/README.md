# Compiler integration CI

The owner authorized production rollout after the [isolated queue experiment](pilot-observations.md).
The canonical author procedure is [landing](../landing.md). GitHub owns integration;
authors do not rebuild or repeat review just because main advances.

## Admission

`.github/workflows/compiler.yml` runs on main PRs, `merge_group`, and manual
dispatch. Its only required status, **compiler-check**, runs
`cargo check --locked -p omega` on Linux. Default checkout selects the PR merge
candidate or merge-group commit, not just the source head. No path filters leave
the required status absent. Permissions are read-only; checkout credentials are
not retained. Compilation runs on ephemeral GitHub-hosted runners, not the owner's
machine. No repository secrets are passed to compiler tests.

`main-ruleset.json` specifies main's native queue: no bypass actors, no force push
or deletion, required PR, linear history, and the check bound to GitHub Actions
(application 15368). One integration candidate builds/merges at a time; squash
publication means one main commit per checked PR result. There is no mandatory
approval count or stale-review dismissal. Required semantic review remains
assignment-specific and concerns the submitted change, not every integration SHA.

The compile gate does not establish runtime correctness or check independent
workspaces. Authors still run focused validation appropriate to their changes.
Full suites, all-host validation and customer boots are not admission gates.

## Post-merge and nightly regression

`.github/workflows/compiler-regression.yml` runs on main pushes, nightly at
09:23 UTC, and manual dispatch. It builds optimized executables and invokes the
existing corpus gate for three positive programs, four intended rejections, and
actual native/Terminal execution agreement. The smoke adapter requires exact
observation rosters, revision, clean source, intended rejection fragments and
authored results. Missing observations, wrong results and infrastructure failures
are red, not success. It never records new goldens.

The adapter retains its original `compiler_pilot.py` filename; it is shared with
the historical pilot, not a second compiler/test path. Run locally with:

```text
python tools/tests/test_compiler_pilot.py -v
python tools/ci/compiler_pilot.py
```

Reports and the exact corpus runner upload as `compiler-regression-SHA-attempt`,
retained seven days. `summary.json` separates build and test timing and identifies
the producer. The 30-minute job does not cancel a running predecessor; pending
push runs coalesce to the newest revision. This is a bounded Linux smoke, not
all supported hosts, the full corpus, or Cathedral UEFI boot. Host/customer and
release qualification remain explicit separate acceptance.

A regression belongs to the author of the regressing change; the first observer
claims triage until attribution is established. Repair or revert promptly through
the same queue. Report failure and time-to-repair honestly; never relabel a failed
run green or silently weaken a test. A queue compile pass is not a runtime claim.

Cargo dependency/output caches are restored by toolchain/lock/config identity;
Cargo still checks freshness. Admission has a separate cache from executable
smoke. Neither a cache hit nor a successful prior run replaces this run's check.
PR updates cancel superseded PR jobs; queue jobs do not cancel one another.

## Administration and rollout

Inspect existing rulesets before applying `main-ruleset.json`; update the named
ruleset instead of creating duplicates. Admin access is needed only to configure
rules. Ordinary write access suffices to submit PRs. See GitHub's
[merge queue documentation](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue).

The rollout installs workflows before protection, seeds the default-main cache,
drains old publication reservations, then enables the main ruleset and submits
the workflow migration itself through the native queue. Verify its merged source
head, published squash commit, and post-merge run. Disable only the owned isolated
pilot ruleset after main succeeds. The old lease/build-receipt client and
`lane_land` driver are retired; do not run a parallel direct-main publisher.

`landing.py` calls `enqueuePullRequest` with an exact expected source head.
During the pilot `gh pr merge` tried to enable repository auto-merge and failed;
the native queue mutation succeeded without changing that repository setting,
overriding checks or jumping the queue. Use the documented helper or GitHub UI.

If the queue itself breaks, inspect its required check and workflow first.
Changing protection or disabling the queue is an explicit owner/admin operation,
not an author fallback. Preserve failed runs and PRs for diagnosis.
