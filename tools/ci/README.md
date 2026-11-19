# Compiler integration CI

A pull request merges by squash once its `admission` check passes, and main is
checked after the merge. [Landing](../docs/landing.md) is the author procedure;
there is no merge queue.

## Admission

[Admission](../../.github/workflows/admission.yml) checks the paths in tracked
Cargo manifests and Rust sources before merge. Active workspaces, dependencies,
Cargo targets and Rust modules or literal file includes must resolve outside
`_recycling/`. The check follows literal `#[path]` attributes, including in
`cfg_attr`, and the `include!`, `include_bytes!` and `include_str!` macros. It
also evaluates literal `concat!` arguments and `env!("CARGO_MANIFEST_DIR")`.
Generated source and arbitrary build-script execution are outside its scope.

Run `python3 -B tools/ci/check_recycling.py` from a checkout with Python 3.11 or
newer. `python3 -B tools/ci/test_check_recycling.py` runs its regression checks.
Both commands run while the root Cargo workspace is under repair.

Changes to the standalone V2 workspace or its CI toolchain also run
`mbx test --locked --workspace` under `omega-rust-v2/`.
[mbx setup](../docs/testing.md#checksum-capable-mbx-setup)

## Post-merge check

`.github/workflows/post-merge-check.yml` runs on every push
to main, one run at a time. It judges the batch of commits main gained since
the last run that reached a verdict. A push that arrives during a run waits,
and a newer push replaces a waiting one, so a burst of merges is judged once.

- `post-merge-check` checks dependency boundaries, lints the workspace and
  runs the lowered-route tests.
- `sample` runs one sample natively and in the interpreter, building `omega`
  from main's restored warm build.
- `report` writes the verdict to the run summary: each failed part and the
  batch's commits. A batch of several commits is named as a whole; the run
  does not bisect it.

A run started by hand with the input `full` set (Actions, Post-merge check,
Run workflow) also runs every Rust test in the workspace, in the `tests` job,
on Linux only. Use it after many merges; push-started runs skip that job.

There is no fixture corpus, no corpus gate and no ledger. The jobs that build the compiler restore
main's warm build with `tools/ci/session_host.sh`.

A failed run does not revert any commit; automatic reverts are a later step of
`wiki/drafts/designs/ci_rebuild_plan.md`.

## Branch cleanup

`.github/workflows/branch-cleanup.yml` deletes stale branches; it runs only
when started by hand (Actions, Branch cleanup, Run workflow) and by default
only reports. Uncheck `dry_run` to delete. It exists because cloud agent
sessions cannot delete refs. `mbx xtask branch-cleanup`
(`tools/xtask/src/branch_cleanup.rs`) holds the rules: it
keeps protected branches, branches with an open pull request and branches
newer than `min_age_hours`, and deletes a branch whose merged or closed pull
request still ends at its tip, or whose tip is already in main. Branches with
unmerged commits and no pull request are deleted only with
`include_unmerged_no_pr`. The run summary and uploaded artifact list every
branch by category.

## Administration

Branch and ruleset protection for `main` lives in GitHub repository settings,
not in this directory. Before applying a ruleset, list the existing ones (for
example with `gh api repos/:owner/:repo/rulesets`) and update the named ruleset
instead of creating a duplicate. Configuring rules needs admin access; opening
and merging pull requests needs only write access. Only the owner or an admin
changes protection; an author must not change it to get a pull request merged.
