# Compiler integration CI

A pull request merges by squash once its `admission` check passes, and main is
checked after the merge. [Landing](../docs/landing.md) is the author procedure;
there is no merge queue.

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

## Administration

`main-ruleset.json` still describes the retired merge-queue ruleset, which
required `compiler-check`; it is not the live protection on main and is kept
here until it is replaced. Inspect the repository's existing rulesets before
applying any definition, and update the named ruleset rather than creating a
duplicate. Admin access is needed only to configure rules; ordinary write
access suffices to open and merge pull requests. Changing protection is an
owner/admin operation, not an author fallback.
