# Compiler integration CI

A pull request merges by squash once its `admission` check passes, and main is
checked after the merge. [Landing](../docs/landing.md) is the author procedure;
there is no merge queue.

## Admission

`.github/workflows/admission.yml` is the only check a pull request needs. It
builds no compiler: a prebuilt `xtask` runs `check-rules` on the title, body
and diff, then, when Rust files changed, `fmt --check`, which runs one
`rustfmt` per CPU.

`tools/xtask/bootstrap-mbx.sh` installs `mbx` at the version in
`tools/mbx-version` and verifies its checksum. It lives under `tools/xtask/` so
that `admission.yml`'s `tools/xtask/**` push filter and cache key cover it
along with the `xtask` crate.

A pull request can restore a cache only from its own runs or from main's. So
the same workflow also runs on pushes to main that change `tools/xtask`,
`Cargo.lock` or the toolchain, and saves the built `xtask` there for every pull
request to restore.

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

## Administration

Branch and ruleset protection for `main` lives in GitHub repository settings,
not in this directory. Before applying a ruleset, list the existing ones (for
example with `gh api repos/:owner/:repo/rulesets`) and update the named ruleset
instead of creating a duplicate. Configuring rules needs admin access; opening
and merging pull requests needs only write access. Only the owner or an admin
changes protection; an author must not change it to get a pull request merged.
