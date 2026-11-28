# Compiler integration CI

A pull request merges by squash once its `admission` check passes.
[Landing](../../wiki/development/landing.md) is the author procedure; there is
no merge queue. No workflow builds, tests or lints the compiler: heavy checks run on
the LAN VMs ([AGENTS.md](../../AGENTS.md#heavy-checks-run-on-the-lan-vms)).

## Admission

[Admission](../../.github/workflows/admission.yml) checks the paths in tracked
Cargo manifests and Rust sources before merge. Every tracked archive file
belongs under the repository-root [`_recycling/`](../../_recycling/). Active workspaces, dependencies,
Cargo targets and Rust modules or literal file includes must resolve outside
`_recycling/`. The check follows literal `#[path]` attributes, including in
`cfg_attr`, and the `include!`, `include_bytes!` and `include_str!` macros. It
also evaluates literal `concat!` arguments and `env!("CARGO_MANIFEST_DIR")`.
Generated source, arbitrary build-script execution and the dependency
frontmatter of single-file Cargo scripts are outside its scope.

Run `python3 -B tools/ci/check_recycling.py` from a checkout with Python 3.11 or
newer. `python3 -B tools/ci/test_check_recycling.py` runs its regression checks.
Admission also requires the shared specification under `wiki/spec/`. It
also checks Omega run examples in Markdown and Omega source, and checks the
arguments passed by application integration fixtures for retired selectors.
It compiles and tests no Rust code.

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
