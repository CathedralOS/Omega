# Repository tests

This tree holds one directory per language, plus `bootstrap/` for tests that
span more than one bootstrap rung.

- `alpha/` contains Alpha conformance and reference tests.
- `beta/` contains trusted Beta compiler reconstruction and differential tests.
- `gamma/` contains the Gamma evaluator and compiler-customer gates.
- `delta/` contains measured candidate-language experiments; it does not define
  a canonical Delta edge.
- `epsilon/` contains Epsilon checking, evaluation, and runtime gates.
- `bootstrap/` contains only tests spanning more than one bootstrap rung.
- `fixtures/` holds the Omega programs that Rust tests read, kept from the
  former corpus; see [the former corpus](#the-former-corpus).
- `psi/` contains black-box gates for the Omega-written Psi product source in
  `source/psi/`; each gate is its own package beside, not inside, that root.
- `product/` holds `gate.sh`, the review, build and run driver every gate of
  the Omega-written product compiler shares.

Rust tests whose subject is one crate remain beside that crate in its `tests/`
directory (or its internal `#[cfg(test)]` modules). Those directories are local
Cargo test targets, not additional repository test roots.

Standalone failing Build fixtures may declare `build_resource_limits.txt` with
four positive integers: evaluator work, staging entries, total logical bytes,
and maximum object extent. The fixture harness's checked request supplies these host
sponsors and fresh private staging; Omega code cannot select its own budget.
Other evaluator ceilings are non-binding in these controls. This keeps refusal
tests small rather than exhausting production-sized allowances.

With that policy, `build_output_alias.txt` may name two relative filenames.
The harness seeds one physical file and a hard link in private output staging,
so the authored artifact-only Build exercises actual output-capture alias
refusal. Default Build has no link operation. These files configure the test
host through the existing request API; the shipped CLI does not read them.

## The former corpus

`tests/omega/` held about 10,700 fixture programs in `pass/`, `fail/`, `run/`
and `pending/`, the outcome ledgers `corpus_*.txt`, `golden/`, `packages/` and
`interpreter_migration.json`: about 52,000 files. A runner compiled every
fixture and a gate compared the result with the ledger. The owner retired it
on 2026-11-13: it had served to reveal the compiler's patterns, and a change is
now proved by tests on the changed code and a very small area around it. The
directory, the runner (`corpus_runner`), `cargo xtask corpus`,
`cargo xtask corpus-gate`, the post-merge row measurement and the published
ledger are deleted, and nothing replaces them. `cargo xtask check-rules`
refuses a new file under `tests/omega/`.

The programs that tests, the task board's named witnesses and current
instructions still read are kept in `fixtures/`, in the same
`<tier>/<group>/<name>/` layout and at the same depth, because a fixture's
`build.omg` reaches `source/library/std` by a relative path. A test names the
fixture it reads by that path; nothing walks the directory.

Everything else stays in git history. `fd75a9baacc` has the
fixtures and their ledgers, with the kept ones already under `tests/fixtures/`;
`bd6ee1a5c45` has every fixture in place and also the
runner and the gates:

```bash
# Read one file of a fixture.
git show fd75a9baacc:tests/omega/fail/borrows/affine_array_projection_copy/main.omg
# List a group.
git ls-tree --name-only fd75a9baacc tests/omega/fail/borrows/
# Bring a fixture back as a kept fixture: restore it, then move it.
git restore --source fd75a9baacc -- tests/omega/fail/borrows/affine_array_projection_copy
mv tests/omega/fail/borrows/affine_array_projection_copy tests/fixtures/fail/borrows/
```
