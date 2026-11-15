# Repository tests

This tree has one directory per language, plus a few shared ones.

- `alpha/` contains Alpha conformance and reference tests.
- `beta/` contains trusted Beta compiler reconstruction and differential tests.
- `gamma/` contains the Gamma evaluator and compiler-customer gates.
- `delta/` contains measured experiments with candidate languages; it does not
  define the canonical Delta step.
- `epsilon/` contains Epsilon checking, evaluation, and runtime gates.
- `bootstrap/` contains only tests spanning more than one bootstrap rung.
- `fixtures/` holds the Omega programs Rust tests read, kept from
  [the former corpus](#the-former-corpus).
- `psi/` contains black-box gates for the Omega-written Psi source in
  `source/psi/`; each gate is its own package next to that directory, not
  inside it.
- `product/` holds `gate.sh`, the script every gate of the Omega-written
  compiler uses to review, build and run.

Rust tests of a single crate stay in that crate's `tests/` directory or its
`#[cfg(test)]` modules. Those are ordinary Cargo test targets, not more
repository test roots.

A standalone failing Build fixture may include `build_resource_limits.txt` with
four positive integers: evaluator work, staging entries, total logical bytes
and largest object size. The fixture harness passes these as the host's budget,
with fresh private staging, through its checked request; Omega code cannot
pick its own budget. Other evaluator limits do not apply in these tests. This
keeps refusal tests small instead of exhausting production-sized budgets.

Under that policy, `build_output_alias.txt` may name two relative filenames.
The harness creates one real file and a hard link to it in private output
staging, so an artifact-only Build actually hits the refusal of aliased
outputs. Default Build has no link operation. These files set up the test host
through the existing request API; the shipped CLI does not read them.

## The former corpus

`tests/omega/` held about 10,700 fixture programs in `pass/`, `fail/`, `run/`
and `pending/`, the outcome ledgers `corpus_*.txt`, `golden/`, `packages/` and
`interpreter_migration.json`: about 52,000 files. A runner compiled every
fixture and a gate compared each result with the ledger. The owner retired it
on 2026-11-13: it had done its job of showing the compiler's patterns, and a
change is now proved by tests on the changed code and a very small area around
it. The
directory, the runner (`corpus_runner`), `cargo xtask corpus`,
`cargo xtask corpus-gate`, the post-merge row measurement and the published
ledger are deleted, and nothing replaces them. `cargo xtask check-rules`
refuses a new file under `tests/omega/`.

Programs still read by tests, named as examples on the task board or cited in
current instructions are kept in `fixtures/`, with the same
`<tier>/<group>/<name>/` layout and depth, because a fixture's `build.omg`
reaches `source/library/std` by a relative path. A test names the
fixture it reads by that path; nothing walks the directory.

Everything else is in git history. `fd75a9baacc` has the fixtures and their
ledgers, with the kept ones already under `tests/fixtures/`; `bd6ee1a5c45` has
every fixture in its old place, plus the runner and the gates:

```bash
# Read one file of a fixture.
git show fd75a9baacc:tests/omega/fail/borrows/affine_array_projection_copy/main.omg
# List a group.
git ls-tree --name-only fd75a9baacc tests/omega/fail/borrows/
# Bring a fixture back as a kept fixture: restore it, then move it.
git restore --source fd75a9baacc -- tests/omega/fail/borrows/affine_array_projection_copy
mv tests/omega/fail/borrows/affine_array_projection_copy tests/fixtures/fail/borrows/
```
