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

Rust tests whose subject is one crate remain beside that crate in its `tests/`
directory (or its internal `#[cfg(test)]` modules). Those directories are local
Cargo test targets, not additional repository test roots.
