# Compiler integration CI

The owner authorized production rollout after the [isolated queue experiment](pilot-observations.md).
The canonical author procedure is [landing](../landing.md). GitHub owns integration;
authors do not rebuild or repeat review just because main advances.

## Admission

`.github/workflows/compiler.yml` runs on main PRs, `merge_group`, and manual
dispatch. Its required status, **compiler-check**, runs
`python tools/fmt.py --check` and `cargo clippy --locked -p omega -- -D warnings`
on Linux. Clippy type-checks the same crates `cargo check -p omega` did and
rejects warnings in them. `.github/workflows/repository-lint.yml` runs the full
`--workspace --all-targets` clippy, which also covers test and example targets,
as the second required status, **workspace-clippy**, on each PR push and in the
merge queue, and again after each main push and nightly.
`.github/workflows/lint-when-mergeable.yml` starts it for a PR whose base
changes to main or that is marked ready when its head has no run yet. Default checkout selects the PR merge
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

## Post-merge check

Pull requests merge on the admission check alone, which builds no compiler.
`.github/workflows/post-merge-check.yml` runs on every push to main, one run
at a time, and judges the batch of commits main gained since the last run that
reached a verdict. A push that arrives while a run is in progress waits, and a
newer push replaces it, so a burst of merges is judged once.

- `post-merge-check` checks dependency boundaries, lints the workspace, runs
  one sample natively and in the interpreter, and runs the lowered-route tests.
- `plan` finds the batch and chooses the corpus rows its changed paths reach
  (`cargo xtask corpus-gate plan`, the selection the corpus gate used).
- `build` compiles `omega` and the corpus runner once
  (`cargo xtask corpus-gate build --out <dir>`) and uploads them with the
  producer's receipt.
- Up to four `shard` jobs at a time download the runner and check their rows
  against the ledger (`corpus-gate check --runner <dir>`). A shard refuses a
  runner whose receipt names another commit or tree, or whose bytes differ,
  and never compiles one itself.
- `report` writes the verdict to the run summary: each failed part, the rows
  that moved, and the batch's commits. A batch of several commits is named as
  a whole; the run does not bisect it.

- `ledger` starts from the last published ledger, splices in every measured
  row whose outcome differs, better or worse
  (`corpus-gate sweep-report --records <dir>`), and publishes the result as the
  asset `corpus_outcomes.txt` of the release `corpus-ledger`, replaced on each
  run, with notes naming the measured commit. Pull requests do not edit the
  ledger and main accepts no direct push, so the file in the repository is
  left as it is.

A run started by hand with the input `full` set (Actions, Post-merge check,
Run workflow) also runs every Rust test in the workspace in `build`, and
measures the whole corpus instead of the batch's rows: 48 shards, four at a
time, each running `corpus-gate sweep-shard` on the prebuilt runner. `ledger`
then compares every measured row with the ledger, publishes it, and `report`
lists the rows that differ, accepted must-reject programs first. It is meant
for after a large wave of merges; pushes run as before. Linux only.

The run reverts nothing; that is a later step of
`wiki/drafts/designs/ci_rebuild_plan.md`.

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

Reports, captured Terminal/native artifacts, the exact corpus runner and its build-profile receipt upload as
`compiler-regression-OS-SHA-attempt`,
retained seven days. `summary.json` separates build and test timing and identifies
the producer. The 45-minute job does not cancel a running predecessor; pending
push runs coalesce to the newest revision. This is a bounded Linux smoke, not
all supported hosts, the full corpus, or Cathedral UEFI boot. Host/customer and
release qualification remain explicit separate acceptance.

Manual dispatch with `cleanup=true` instead runs the existing CML4 cleanup
controls on both Ubuntu and Windows. `tools/ci/compiler_cleanup.py` selects
ordered parent/child hooks, partial construction, replacements and the nine
returned-linear-child places with their reject twins; it uses the same corpus
execution and report validator as smoke. Authored output must be observed by
both engines. It does not change admission, scheduled smoke or any golden.
The macOS/Linux roster also preserves the existing descriptor output controls
for C pointer/count pairs and separate stdout/stderr.

```text
gh workflow run compiler-regression.yml --ref BRANCH -f cleanup=true
python tools/ci/compiler_cleanup.py
```

Read each host's `build/compiler-cleanup/summary.json`, `checking.json` and
`both.json`; a completed job and its actual producer identify that host's
measurement. Pending, cancelled and failed jobs do not establish qualification.

A regression belongs to the author of the regressing change; the first observer
claims triage until attribution is established. Repair or revert promptly through
the same queue. Report failure and time-to-repair honestly; never relabel a failed
run green or silently weaken a test. A queue compile pass is not a runtime claim.

Cargo dependency/output caches are restored by toolchain/lock/config identity;
Cargo still checks freshness. Admission has a separate cache from executable
smoke. Neither a cache hit nor a successful prior run replaces this run's check.
PR updates cancel superseded PR jobs; queue jobs do not cancel one another.

## Corpus gate

Before this gate no CI job ran the corpus, so a compiler change could break
ledger rows and land green; #2370 broke more than 100 and nobody saw it until a
sweep. `.github/workflows/corpus-gate.yml` runs, for every pull request that is
not a draft, the corpus rows the change can
affect, and fails when one of them gets worse than
`tests/omega/corpus_outcomes.txt` records:

- a `pass/` or `run/` row stops checking (or loses a matched native rejection),
- a `fail/` row is now accepted, or
- a `fail/` row still rejects but no longer with its pinned reason.

A new diagnostic wording, timing or fact on a row that keeps its verdict is not
a failure. A fixture with no ledger row is held to its tier.

**Which rows.** `cargo xtask corpus-gate plan --base HEAD^1` reads the diff. A
pull request that changes only corpus ledgers and Markdown skips, by #2375's
rule: it changes nothing the corpus reads. Otherwise the plan takes, in order,
every touched fixture with its twins in the other tiers (and the fixtures whose
`build.omg` names a touched `samples/` package), the fixed smoke set in
`corpus_gate_smoke.txt` (the fastest green row of every tier/group), the
families `corpus_gate_paths.txt` maps the touched paths to, and, when any path
can reach the compiler, a sample spread round-robin over every tier/group in a
fixed hash order. Paths under `.github/`, `tools/`, `wiki/`, `samples/`,
`bootstrap/` and `.agents/` cannot reach the compiler; every other path can,
including ones nobody listed. Families get at most two thirds of the budget.
The runner selects by substring, so a chosen `pass/a/x` also runs `pass/a/x_l`;
the plan names those rows and judges them too.

**How much.** Rows are packed into at most 8 shards by recorded time, 2200
recorded seconds each, which took 6 to 10 minutes of fixtures at 4 jobs on a
4-CPU host in the replays of #2370 and #2461. A compiler change got 1,550 to
2,130 rows there, 9 to 12% of the corpus.
The broad sample skips fixtures recorded above 40 seconds; touched and mapped
fixtures do not.

**Whose regression.** A row worse than the ledger is first run once more at
the candidate; a row that recovers is reported as flaky and not counted. A
confirmed row whose ledger line the pull request did not change is then
excused when main already shows the same loss: in the nightly sweep's
observations (the `observed.txt` asset of the `corpus-sweep-main` release) or
in a run of that row at the base commit, which the shard builds only when it
needs it. Whatever remains fails the shard. Each shard's summary tables every
row by class, with the ledger and observed outcome and the first diagnostic,
and uploads its evidence as `corpus-gate-shard-N-attempt`.

**Build.** Shards run in `ubuntu:24.04` as root at `/home/user/Omega`, the
container and path the warm-build job builds main in, and restore that build
with `tools/ci/corpus_host.sh`; only the crates the candidate changed compile.

**Merge queue.** A merge-queue entry runs only the last job, which reports
success without planning or checking rows, because the pull request's own run
judged the same diff. Running a plan and eight shards for each of the eight
groups the queue builds at once left the required checks waiting about an hour
for a runner on 2026-11-12. A row that breaks only in combination with what
main gained after the pull request's run is therefore not caught in the queue;
post-merge smoke and the nightly sweep see it.

**Required status.** The last job, `corpus-gate`, is the one to require: it is
green when the plan skips or every shard passes, and red when the plan or any
shard fails or does not finish. In the merge queue it is always green. Requiring it is an owner change to the main
ruleset (`main-ruleset.json`), not something this workflow does.

Run a shard locally with the same code:

```bash
mbx xtask corpus-gate plan --base origin/main --out build/corpus-gate/plan.json
mbx xtask corpus-gate check --plan build/corpus-gate/plan.json --shard 0 < /dev/null
```

### Nightly sweep

`.github/workflows/corpus-sweep.yml` runs every fixture on main's head at 02:41
UTC in 64 shards (`cargo xtask corpus-gate sweep-shard`), then
`cargo xtask corpus-gate sweep-report` splices every row whose outcome differs
from the ledger with `--record`'s rules (the measured commit as the stamp, and
`regressed-from:` on a pass/run row that lost a success) and opens or replaces
one ledger-only pull request from `work/tests-nightly-corpus-sweep`. Its
description starts with the soundness alerts, every `fail/` fixture the
compiler now accepts, and then tables the worse rows by first diagnostic. A
ledger-only pull request may record red rows, so a reviewer must read the
alerts before it lands; recording an accepted must-reject program keeps the
ledger honest and does not make it acceptable. The sweep also publishes its
observations for the gate.

Owner settings: a pull request opened with the default `GITHUB_TOKEN` starts
no workflows, so its required checks never report. Store a token that can push
a branch and open pull requests as the `CORPUS_SWEEP_TOKEN` secret. Without it
the sweep falls back to `GITHUB_TOKEN`, which needs "Allow GitHub Actions to
create and approve pull requests" enabled and leaves the pull request without
checks until someone pushes to it.

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
