# Terminal Psi publication

[publish_artifact.rs](src/publish_artifact.rs) seals the validated optimization
result into canonical semantics, optimization provenance, proof and optional
debug products. It does not consume checked source. The
[Terminal product contract](../../../../wiki/spec/terminal-psi/product.md)
defines separate consumption and the distinction between semantics and receipts.

[Boundary-operator custody](src/boundary_operator_custody.rs) binds
a checked demand roster to that exact artifact. Its entrypoint checks semantic
identity, replays occurrences, enforces one-to-one application/operation joins
and complete retained FMA coverage, then constructs the private receipt.

The replay mechanisms have distinct authored roles:

- [Local initializers](src/boundary_operator_custody/local_initializers.rs)
  join selected calls and FMA occurrences at exact state and call coordinates.
- [Structural returns](src/boundary_operator_custody/structural_returns.rs)
  join selected return calls to their expression-role applications.
- [Float comparisons](src/boundary_operator_custody/float_comparisons.rs)
  check the complete Terminal comparison roster, selected meaning, operand
  format and exact application, including implicit match-arm comparisons.

Receipt fields remain private. Source custody cannot replace canonical coverage
or grant downstream realization authority. The product coordinator in
[terminal-production](../../compiler/terminal-production/README.md) sequences
optimization, artifact publication and source receipt construction.