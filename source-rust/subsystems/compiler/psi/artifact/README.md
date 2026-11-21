# Psi artifacts

A producer publishes one portable program. A consumer can reopen its saved `.psi`, check the identity of the bytes and decide whether to trust the producer. The program itself is defined by [Psi](../psi.rs).

| Responsibility | Owner |
| --- | --- |
| Encode structural domains, access plans and quotient declarations used in the artifact and in native program identities | [Declaration encoding](declaration-encoding/src/lib.rs) |
| Encode and decode the semantic program and its named sections, including the optional debug map | [Canonical wire format](wire/README.md) |
| Seal the exact program and build or reopen its artifact envelope and manifest | [Artifact codec](codec/README.md) |
| Retain source declaration custody, machine provenance and a checked entry receipt for source-issued programs | [Publication records](publication/publication.rs) |
| Record the requested optimizations carried by the artifact | [Optimization selections](optimization-selections/) |
| Apply the receiving host's producer trust decision to the exact immutable artifact | [Authorization](authorization/authorization.rs) |
