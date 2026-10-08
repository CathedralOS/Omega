# Terminal Psi publication

[publish_artifact.rs](src/publish_artifact.rs) seals the validated optimization
result into canonical semantics, optimization provenance, proof and optional
debug products. It consumes the optimization result and transfers its semantic
module into the artifact; only source-join records remain alongside it. Artifact
reads borrow established storage and cannot trigger a decode or replay. External
envelope loading still validates its sections before exposing that storage.
Publication does not consume checked source. The
[Terminal product contract](../../../../../wiki/spec/terminal-psi/product.md)
defines separate consumption and the distinction between semantics and receipts.

[Entry receipts](src/entry_receipt.rs) establish checked-source correspondence
for the optimized module's unique Unit entry and rejoin its published semantic
identity. [Receiver eligibility](src/entry_receipt/receiver_eligibility.rs) derives
whether the exact source receiver can start zero-initialized without executable
nominal cleanup. This is a separate source-aware judgment, not part of canonical
sealing or a grant of native entry authority.

[Boundary-operator custody](src/boundary_operator_custody.rs) binds
producer-retained checked application indices to that artifact. Exact requirement
and argument rows already live in canonical Terminal semantics; publication does
not reconstruct them or replay comparison emission. The source-index companion
remains temporary input to native provider realization. The separate
[result-policy check](src/boundary_operator_custody/result_policies.rs) still
checks source-call/adapter correspondence and has not migrated to portable input.

Receipt fields remain private. Source custody cannot replace canonical coverage
or grant downstream realization authority. The product coordinator in
[terminal-production](../../../../omega/compilation/driver/terminal-production/README.md) sequences
optimization, artifact publication and source receipt construction. Its test-only
dependency here exercises that real sequence; production dependencies stay in Psi.
