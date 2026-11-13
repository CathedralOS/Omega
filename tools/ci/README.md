# Compiler integration CI

The owner authorized production rollout after the [isolated queue experiment](pilot-observations.md).
The canonical author procedure is [landing](../landing.md). GitHub owns integration;
authors do not rebuild or repeat review just because main advances.

## Admission

`.github/workflows/admission.yml` is the only check a pull request needs. It
builds no compiler: a prebuilt `xtask` runs `check-rules` on the title, body
and diff, then `fmt --check` when Rust files changed, which runs one `rustfmt`
per CPU. A pull request can restore a cache only from its own runs or from
main's, so the same workflow runs on pushes to main that change `tools/xtask`,
`Cargo.lock` or the toolchain, and saves the built `xtask` there for every
pull request to restore.

## Post-merge check

Pull requests merge on the admission check alone, which builds no compiler.
`.github/workflows/post-merge-check.yml` runs on every push to main, one run
at a time, and judges the batch of commits main gained since the last run that
reached a verdict. A push that arrives while a run is in progress waits, and a
newer push replaces it, so a burst of merges is judged once.

- `post-merge-check` checks dependency boundaries, lints the workspace and
  runs the lowered-route tests.
- `sample` runs one sample natively and in the interpreter, building `omega`
  from main's restored warm build.
- `report` writes the verdict to the run summary: each failed part and the
  batch's commits. A batch of several commits is named as a whole; the run
  does not bisect it.

A run started by hand with the input `full` set (Actions, Post-merge check,
Run workflow) also runs every Rust test in the workspace, in the `tests` job.
It is meant for after a large wave of merges; pushes run as before. Linux only.

There is no fixture corpus, no corpus gate and no ledger: the run measures no
rows and publishes nothing. The jobs that build the compiler restore main's
warm build with `tools/ci/session_host.sh`.

The run reverts nothing; that is a later step of
`wiki/drafts/designs/ci_rebuild_plan.md`.

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

## Production cutover observed: 2026-10-25 UTC

Main ruleset **25803202** is active with no bypass actors. The old publication
queue was empty at cutover. The isolated pilot ruleset **25802197** and its
workflow are disabled; their definitions and observations remain historical.

- [Default-main admission/cache seed](https://github.com/CathedralOS/Omega/actions/runs/40148269956)
  passed in 1m48s.
- [Cold production runtime smoke](https://github.com/CathedralOS/Omega/actions/runs/40148228977)
  passed in 12m44s: three native/Terminal positives and four intended rejections.
- [Migration PR #21](https://github.com/CathedralOS/Omega/pull/21) passed
  [PR admission](https://github.com/CathedralOS/Omega/actions/runs/40149107230)
  in 32s (Cargo 2.51s), then
  [queue admission](https://github.com/CathedralOS/Omega/actions/runs/40149184976)
  with Cargo 3.48s. Both restored the default-main cache.
- Source stayed `3b29d843632aab7898812d7927d411990551ff84` throughout submission;
  the queue squash-published `4f1915ed5b2347f3636764e9440d2ff43629a323` at 01:05:41 UTC.
  No author rebase/rebuild was needed. An attempted direct fast-forward of that
  valid source returned HTTP 422 requiring the queue, leaving main unchanged.

These are measured jobs, not a performance guarantee for compiler-changing PRs.
The compile-error refusal control remains in the isolated pilot record; no broken
source was pushed to main to demonstrate it.

Manual dispatch with `foreign_records=true` instead measures the nine-place
libc `lldiv` scalar-record result row and its nine copy-custody rejects on
Ubuntu and Windows. `tools/ci/compiler_foreign_records.py` uses the same corpus
validator, profiles and retained products as smoke. On Windows it additionally
loads the exact SDK API-set DLL/export, observes real two-i64 C return values,
and records the resolved image digest. This is Windows indirect-result evidence
only after actual execution; macOS/Linux return this pair in registers. It
never records goldens or grants unmeasured Linux ARM64 acceptance.

```text
gh workflow run compiler-regression.yml --ref BRANCH -f foreign_records=true
```

Read `build/compiler-foreign-records/{summary,checking,both}.json` and the
Windows-only `windows-lldiv.json`; failures and absent engine facts remain red.


Manual dispatch with `runtime_value_generics=true` selects the runtime Value
acceptance cohort on the existing Ubuntu x86_64, Windows x86_64, macOS ARM64,
and Ubuntu ARM64 hosted labels:

```text
python tools/ci/compiler_runtime_value_generics.py --list
gh workflow run compiler-regression.yml --ref BRANCH -f runtime_value_generics=true
```

Pin BRANCH to the reviewed source commit. The checkout SHA must match
`GITHUB_SHA`; a dirty checkout refuses qualification. The cohort requires all
nine places and intended-invalid twins for each authored row, plus the named
runtime witnesses and static-only controls. Checking-only fixtures are reported
separately and cannot satisfy required execution witnesses. Each runtime fixture
must build and execute through native and Terminal on that runner with matching
exit and authored stream observations. Target image generation alone supplies
no execution evidence.

Read each architecture's uploaded
`build/compiler-runtime-value-generics/{summary,checking,both}.json` and retained
producer profile. A missing historical host golden is reported explicitly;
authored execution and diagnostic oracles remain mandatory. This mode uses the
existing read-only workflow permissions and adds no runner or service.
