# Repository tests

This tree owns validation whose subject is the repository, the Omega language,
or a multi-package build rather than one Rust crate.

- `architecture/` is the test-only Cargo package for cross-crate dependency and
  semantic-shape guards.
- `alpha/` contains Alpha conformance and reference tests.
- `beta/` contains trusted Beta compiler reconstruction and differential tests.
- `gamma/` contains the Gamma evaluator and compiler-customer gates.
- `delta/` contains measured candidate-language experiments; it does not define
  a canonical Delta edge.
- `bootstrap/` contains only tests spanning more than one bootstrap rung.
- `omega/` contains Omega-language pass, fail, pending, and execution cases.
- `fixtures/` contains reusable package and Terminal Psi inputs.
- `native-differential/` checks source-to-native behavior, ABI transport, and
  artifact replay. Owned-record call composition (including an empty helper
  before forwarding) is exercised by:

  ```text
  cargo nextest run -p omega-native-differential-test --test scalar_case_results --no-fail-fast -E 'test(record_reads::parameters)'
  ```

  These cases publish for their listed targets and execute on a supported
  matching host; cross-target publication is not a runtime result.

Rust tests whose subject is one crate remain beside that crate in its `tests/`
directory (or its internal `#[cfg(test)]` modules). Those directories are local
Cargo test targets, not additional repository test roots.
