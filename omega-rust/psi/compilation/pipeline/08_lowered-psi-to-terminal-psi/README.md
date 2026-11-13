# Terminal Psi publication

[publish_artifact.rs](src/lib.rs) seals the validated optimization
result into canonical semantics, optimization provenance, proof and optional
debug products. It consumes the optimization result and transfers its semantic
module into the artifact; only source-join records remain alongside it. Artifact
reads borrow established storage and cannot trigger a decode or replay. External
envelope loading still validates its sections before exposing that storage.
Publication does not consume checked source. The
[Terminal product contract](../../../../../wiki/spec/terminal-psi/product.md)
defines separate consumption and the distinction between semantics and receipts.

[Entry receipts](src/lib.rs) establish checked-source correspondence
for the optimized module's unique Unit entry and rejoin its published semantic
identity. [Receiver eligibility](src/lib.rs) derives
whether the exact source receiver can start zero-initialized without executable
nominal cleanup. This is a separate source-aware judgment, not part of canonical
sealing or a grant of native entry authority.

Boundary applications and checked candidate bodies are canonical Terminal
semantics. Publication emits no checked-application scope or source-index
companion for native realization. The unused checked-source result-policy audit
is removed with its old-checker dependency. Canonical decoding and Terminal
semantic checks remain responsible for the artifact they consume.

Receipt fields remain private. Source custody cannot replace canonical coverage
or grant downstream realization authority. The Psi production owner in
[terminal-production](../../psi-compiler/terminal-production/README.md) sequences
optimization, artifact publication and source receipt construction. Its test-only
dependency here exercises that real sequence; production dependencies stay in Psi.
