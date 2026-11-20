# Bootstrap proof experiments

These tests run proof checkers on the [Gamma evaluator](../../../bootstrap/gamma/interpreter/README.md). The [bootstrap chain](../../../bootstrap/README.md) builds independently of this suite.

- [`checker/`](checker/README.md) owns the generic ground-equality checker:
  conservative theory formation, explicit derivation rules, and owner-root
  checking under one resource ledger. Start at
  [`checker.gamma`](checker/implementation/checker.gamma).
- [`beta_encoding/`](beta_encoding/README.md) owns artifact-specific Beta
  definitions and the eventual independently reconstructed encoding root and
  certificate. Start at [`theory.gamma`](beta_encoding/theory/theory.gamma).
  The current definitions cover byte classification, nibble conversion,
  fixed-width serialization, checked increment, and unsigned ordering.

Run the [comparison tests](derivation-comparison/README.md) for the checker or the [Beta encoding tests](beta-encoding-theory/README.md) for the emitted definition package.

The full certificate for the selected Gamma evaluator's Beta source and Alpha
tape, and artifact admission, remain unfinished. Generic proof success under a
supplied theory does not establish that theory's artifact authority. An encoding
equation also does not prove Gamma evaluator semantics.

[Whole-chain minimization](../../../bootstrap/MINIMIZATION.md) governs retention. Replacing the
checker must preserve bounded input custody, checked premises, and independently
owned subjects. Replacing the partial Beta definitions must retain the obligation
for a faithful complete theory and independently reconstructed encoding root.
The [complete encoding contract](beta_encoding/ACCEPTANCE.md)
owns that acceptance target. [TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md)
tracks the remaining work and strategy pauses.

To audit the proof manifests and their test helper for Rust producer dependencies,
run this from the repository root with `xtask` built:

```sh
sh tools/rust_producer_omission.sh \
  --manifest tests/bootstrap/proofs/checker/implementation/implementation.gamma.sources \
  --manifest tests/bootstrap/proofs/beta_encoding/theory/theory.gamma.sources \
  --steps tests/bootstrap/proofs/sources_env.sh
```
