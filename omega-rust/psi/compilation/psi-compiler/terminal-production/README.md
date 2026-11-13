# Terminal production

Public contract: [Terminal Psi product](../../../../../wiki/spec/terminal-psi/product.md).

No request reaches Terminal Psi through this crate today.
`TerminalProductionRequest::produce`, `BuildProviderProductionRequest::produce`
and the external provider declaration constructors all refuse with
`no product route yet`. The stage that lowered checked trees into Lowered Psi
(Psi 06) is deleted, and the hand-off from the lowered-form checker's published
module (`TASKS.md`, **LOWERED-FORM-CHECKER**) is not built. A refusal is the
whole behavior: nothing here routes a program back through the old checker.

What remains in [lib.rs](src/lib.rs) is the set of contracts that hand-off has
to fill:

- `TerminalProductionRequest`, `TerminalMachineSelection` and
  `TerminalProgramRoots`: what a caller asks to have produced.
- [custody.rs](src/custody.rs): `ProducedTerminalProgram` and
  `PortableTerminalProgram`, the retained program with its Build companions,
  machine provenance and per-application receipts, including their
  compiler-storage records.
- [build_bindings.rs](src/build_bindings.rs): the record shapes of Build
  bindings, requests, queries, filesystem calls and native locators that travel
  beside a program.
- [external_providers.rs](src/external_providers.rs): external provider
  declarations as data, read by provider selection and package review.
- `composition_roots`: `TerminalApplicationCustody`, which attaches entry,
  callback and physical-adapter roots and the optional `ProgramEntry` receipt
  to a sealed artifact after publication.
- `observation`: stage labels for the caller's timing rows.

The [Psi 08 entry receipt owner](../../pipeline/08_lowered-psi-to-terminal-psi/src/entry_receipt.rs)
checks the unique Unit entry in a published artifact and binds its exact
semantic identity.
