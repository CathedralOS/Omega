# Psi

[`Psi`](psi.rs) is the portable program shared by source publication, the interpreter and native lowering. The public name [`TerminalModule`](vocabulary.rs) refers to the same type.

## From source to an authorized program

1. Source stage 07 publishes the checked program once. Its consumers share the canonical artifact in memory. [Source publication](../source-to-psi/pipeline/07_checked-program-to-psi/src/source_publication.rs) · [Portable program](../source-to-psi/pipeline/07_checked-program-to-psi/src/custody.rs)
2. A saved `.psi` carries the artifact envelope. On reopening it, the codec decodes the canonical sections and reconstructs the manifest. [Artifact envelope](codec/src/canonical_artifact.rs) · [Semantic wire format](wire/src/semantic_module.rs)
3. The receiving host authorizes the exact artifact using its trust decision for the producer. Native stage 00 checks that the provider demand refers to that artifact. [Authorization](authorization/authorization.rs) · [Native admission](../psi-to-native/pipeline/00_psi-to-admitted-native-inputs/admit.rs)

## Program records

The program holds an [identity table](identity/table.rs), an entry and its [machines](machines/machine.rs). A machine contains [blocks](machines/block.rs) with [operations](machines/operation.rs) and a [terminator](machines/terminator.rs).

| Program field | Contents |
| --- | --- |
| [`types`](types/declarations.rs) | Structural types, domains, qualifications and quotients |
| [`scalars`](types/scalars.rs) | Scalar applications, folded constants and predicates |
| [`interfaces`](boundary/interfaces.rs) | Services, entries, providers and callbacks |
| [`access`](ownership/declarations.rs) | Access views, lifecycle plans and reborrows |
| [`execution`](machines/execution.rs) | Callables, dispatch, suspension and task activation |
| [`proofs`](evidence/program_proofs.rs) | Theorems, evidence and checked invariants |
| [`layouts`](types/layouts/declarations.rs) | Target layout applications and native layouts |
