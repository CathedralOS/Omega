# Repository tests

This tree owns validation whose subject is the repository, the Omega language,
or a multi-package build rather than one Rust crate.

- `architecture/` is the test-only Cargo package for cross-crate dependency and
  semantic-shape guards.
- `alpha/` contains Alpha conformance, reference, and off-chain tape-tool tests.
  A Beta, Gamma, or Delta directory is added only when that language has a real
  executable subject.
- `bootstrap/` contains only tests spanning more than one bootstrap rung.
- `omega/` contains Omega-language pass, fail, pending, and execution cases.
- `fixtures/` contains reusable package and Terminal Psi inputs.

Rust tests whose subject is one crate remain beside that crate in its `tests/`
directory (or its internal `#[cfg(test)]` modules). Those directories are local
Cargo test targets, not additional repository test roots.
