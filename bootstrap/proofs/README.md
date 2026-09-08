# Bootstrap proofs

This is sidecar proof work for the [bootstrap chain](../README.md), not another
language rung. Its programs are ordinary Gamma executed by the selected
[Gamma evaluator](../2_gamma/README.md).

- [`checker/`](checker/README.md) owns the generic ground-equality checker:
  conservative theory formation, explicit derivation rules, and owner-root
  checking under one resource ledger. Start at
  [`checker.gamma`](checker/implementation/checker.gamma).
- [`beta_encoding/`](beta_encoding/README.md) owns artifact-specific Beta
  definitions and the eventual independently reconstructed encoding root and
  certificate. Start at [`theory.gamma`](beta_encoding/theory/theory.gamma).
  The current definitions cover byte classification, nibble conversion,
  fixed-width serialization, checked increment, and unsigned ordering.

The full certificate for the selected Gamma evaluator's Beta source and Alpha
tape, and artifact admission, remain unfinished. Generic proof success under a
supplied theory does not establish that theory's artifact authority. An encoding
equation also does not prove Gamma evaluator semantics.

[Whole-chain minimization](../MINIMIZATION.md) governs retention. Replacing the
checker must preserve bounded input custody, checked premises, and independently
owned subjects; replacing the partial Beta definitions must retain the obligation
for a faithful complete theory and independently reconstructed encoding root.
The [complete encoding contract](beta_encoding/ACCEPTANCE.md)
owns that acceptance target; [TASKS_BOOTSTRAP.md](../../TASKS_BOOTSTRAP.md)
tracks the remaining work and strategy pauses.
