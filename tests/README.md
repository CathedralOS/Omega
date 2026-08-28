# Repository tests

This tree owns validation whose subject is the repository, the Omega language,
or a multi-package build rather than one Rust crate.

- `architecture/` is the test-only Cargo package for cross-crate dependency and
  semantic-shape guards.
- `omega/` contains Omega-language pass, fail, pending, and execution cases.
- `fixtures/` contains reusable package and Terminal Psi inputs.
- `lattice/` contains inputs exercised across compiler-lattice edges.

Rust tests whose subject is one crate remain beside that crate in its `tests/`
directory (or its internal `#[cfg(test)]` modules). Those directories are local
Cargo test targets, not additional repository test roots.
