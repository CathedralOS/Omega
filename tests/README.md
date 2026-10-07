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
- `omega/` contains the Omega language corpus and the package projects the
  package manager tests build.
- `psi/` contains black-box gates for the Omega-written Psi product source in
  `source/psi/`; each gate is its own package beside, not inside, that root.
- `product/` holds `gate.sh`, the review, build and run driver every gate of
  the Omega-written product compiler shares.

Rust tests whose subject is one crate remain beside that crate in its `tests/`
directory (or its internal `#[cfg(test)]` modules). Those directories are local
Cargo test targets, not additional repository test roots.

Standalone failing Build fixtures may declare `build_resource_limits.txt` with
four positive integers: evaluator fuel, staging entries, total logical bytes,
and maximum object extent. The corpus checked request supplies these host
sponsors and fresh private staging; Omega code cannot select its own budget.
Other evaluator ceilings are non-binding in these controls. This keeps refusal
tests small rather than exhausting production-sized allowances.

With that policy, `build_output_alias.txt` may name two relative filenames.
The harness seeds one physical file and a hard link in private output staging,
so the authored artifact-only Build exercises actual output-capture alias
refusal. Default Build has no link operation. These files configure the test
host through the existing request API; the shipped CLI does not read them.
