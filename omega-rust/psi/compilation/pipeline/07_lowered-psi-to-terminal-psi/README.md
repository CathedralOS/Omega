# Terminal Psi publication

[publish_artifact.rs](src/publish_artifact.rs) seals the validated optimization
result into canonical semantics, optimization provenance, proof and optional
debug products. It does not consume checked source. The
[Terminal product contract](../../../../../wiki/spec/terminal-psi/product.md)
defines separate consumption and the distinction between semantics and receipts.

[Entry receipts](src/entry_receipt.rs) establish checked-source correspondence
for the optimized module's unique Unit entry and rejoin its published semantic
identity. [Receiver eligibility](src/entry_receipt/receiver_eligibility.rs) derives
whether the exact source receiver can start zero-initialized without executable
nominal cleanup. This is a separate source-aware judgment, not part of canonical
sealing or a grant of native entry authority.

[Boundary-operator custody](src/boundary_operator_custody.rs) binds
a checked demand roster to that exact artifact. Its entrypoint checks semantic
identity, replays occurrences, enforces one-to-one application/operation joins
and complete retained FMA coverage, then constructs the private receipt.

The replay mechanisms have distinct authored roles:

- [Source-call occurrences](src/boundary_operator_custody.rs)
  join retained calls to their exact source applications and Terminal operations.
- [Float comparisons](src/boundary_operator_custody/float_comparisons.rs)
  check the complete Terminal comparison roster, selected meaning, operand
  format and exact application, including implicit match-arm comparisons.

Receipt fields remain private. Source custody cannot replace canonical coverage
or grant downstream realization authority. The product coordinator in
[terminal-production](../../../../omega/compilation/driver/terminal-production/README.md) sequences
optimization, artifact publication and source receipt construction. Its test-only
dependency here exercises that real sequence; production dependencies stay in Psi.
