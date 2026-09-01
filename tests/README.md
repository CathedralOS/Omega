# Repository tests

This tree owns validation whose subject is the repository, the Omega language,
or a multi-package build rather than one Rust crate.

- `architecture/` is the test-only Cargo package for cross-crate dependency and
  semantic-shape guards.
- `alpha/`, `beta/`, `gamma/`, and `delta/` contain tests whose subject is one
  bootstrap language or the compiler accepting it. A language directory is
  added only when it has a real test.
- `proof-checker/` contains tests and the independent reference for the
  authoritative checker; the checker source and artifact remain under
  `source/alpha/checker/`.
- `bootstrap/` contains only tests spanning more than one bootstrap rung.
- `omega/` contains Omega-language pass, fail, pending, and execution cases.
- `fixtures/` contains reusable package and Terminal Psi inputs.

Rust tests whose subject is one crate remain beside that crate in its `tests/`
directory (or its internal `#[cfg(test)]` modules). Those directories are local
Cargo test targets, not additional repository test roots.
