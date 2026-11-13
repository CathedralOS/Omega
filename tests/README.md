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
- `omega/` is the retired Omega language corpus. Nothing runs it and nothing
  may be added to it.
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

`omega/` held about 10,700 fixture programs in `pass/`, `fail/`, `run/` and
`pending/`, the outcome ledgers `corpus_*.txt`, and the runner and gate that
compiled every fixture and compared the result with the ledger. The owner
retired it on 2026-11-13: it had served to reveal the compiler's patterns, and
a change is now proved by tests on the changed code and a very small area
around it. The runner (`corpus_runner`), `cargo xtask corpus`,
`cargo xtask corpus-gate`, the post-merge row measurement and the published
ledger are gone, and nothing replaces them.

The programs that tests, the task board's named witnesses and current
instructions still read are kept in `fixtures/`, in the same
`<tier>/<group>/<name>/` layout and at the same depth, because a fixture's
`build.omg` reaches `source/library/std` by a relative path. A test names the
fixture it reads by that path; nothing walks the directory.

Everything else stays in git history. `bd6ee1a5c45` is a commit that has the
whole corpus, its ledgers and its runner:

```bash
# Read one file of a fixture.
git show bd6ee1a5c45:tests/omega/pass/calls/runtime_static_entry_context_exit/main.omg
# List a group.
git ls-tree --name-only bd6ee1a5c45 tests/omega/fail/borrows/
# Bring a fixture back as a kept fixture beside the tests.
git archive bd6ee1a5c45 tests/omega/pass/calls/runtime_static_entry_context_exit | tar -x -C /tmp
```
