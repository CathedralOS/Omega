# Terminal production

Public contract: [Terminal Psi product](../../../wiki/spec/terminal-psi/product.md).
This crate sequences production; it does not own another executable IR.

Enter [terminal_production.rs](src/terminal_production.rs). The sequence is
`checked-trees-to-lowered-psi` -> `lowered-psi-to-lowered-psi` ->
`lowered-psi-to-terminal-psi`. `LoweredPsi` belongs to representations.
Publication consumes the validated optimization-stage result, not an unoptimized
producer-private shortcut.

[Optimization ownership](../pipeline/06_lowered-psi-to-lowered-psi/README.md)
keeps shared retention policy separate from pass execution.
[Publication ownership](../pipeline/07_lowered-psi-to-terminal-psi/README.md)
separates canonical sealing from checked boundary-operator occurrence replay.

`TerminalProductionRequest` carries the checked trees, the
`TerminalMachineSelection` naming the machine, and the exact Psi optimization
selection. Its one `produce` entry takes a `TerminalProductionCustody`: an
optional ProgramEntry source-signature identity, the caller's opaque callback
sidecar, and the stage-timing carrier. Every product runs the same ledger,
lowering, optimization, publication, and boundary-scope legs; an entry identity
adds the ProgramEntry receipt legs, which check the unique Unit entry before
publication and rejoin the published semantic identity afterward. The
`ProducedTerminalArtifact` returns the artifact beside that retained custody,
and rejection returns the original opaque sidecar without cloning it.

The [entry receipt owner](src/terminal_production/entry_receipt.rs) checks the
unique Unit entry, records its identity and delegates source receiver eligibility
to its own child before publication.

ProgramEntry also retains source-derived receiver eligibility when the exact
attached record is ZII-valid and needs no executable nominal cleanup. The
source `Self` reference names the machine; its resolved attachment names the
owned data. Keep both identities distinct when joining the exact Terminal self
parameter. An absent eligibility row means free or unsupported, not permission
to infer validity from layout. The shared cleanup query also serves ordinary
checked cleanup planning. This receipt is checked-source correspondence, not a
source-free proof, a storage grant, or Fused service establishment; native entry
must still supply those independent obligations.

The product keeps checked boundary-operator application scope and selected
floating-point occurrences beside the canonical artifact. Callback custody is
an opaque owned sidecar: carrying it does not interpret placement or grant
registration, invocation, address, or lifetime authority. Native realization
must rejoin the exact source/build receipts under its own authority.

## Lowering owners

This coordinator does not own the lowerers' feature dispatch. Start with
[checked execution selection](../pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/mod.rs),
then [machine lowering](../pipeline/05_checked-trees-to-lowered-psi/src/machine_lowering.rs).
[Source custody](../pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/source_custody/mod.rs)
prepares expressions and places; the
[call catalog](../pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/call_catalog.rs)
owns shared call lowering.

### Partial ownership and cleanup

[Cleanup lowering](../pipeline/05_checked-trees-to-lowered-psi/src/unit/unit_cleanup.rs)
and its partial-cleanup children own residual destruction.
The [ownership contract](../../../wiki/spec/terminal-psi/ownership.md) defines
what must survive into Terminal Psi; successful helper tests do not establish
every source ownership shape's end-to-end coverage.

### Structural predicate production

[Contract predicates](../pipeline/05_checked-trees-to-lowered-psi/src/proofs/contract_predicates.rs)
lower checked facts under the
[structural-predicate contract](../../../wiki/spec/terminal-psi/structural_predicates.md).
Claim identity, projection path, and equal scalar values are not interchangeable.

### Structural access and stores

[Structural source custody](../pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/source_custody/structural/mod.rs)
prepares accesses; [store emission](../pipeline/05_checked-trees-to-lowered-psi/src/emission/structural_scalar_store.rs)
consumes prepared subjects.
The [structural-access contract](../../../wiki/spec/terminal-psi/structural_access.md)
is authoritative, not the current bounded producer vocabulary.

### Structural results and suspension

[Structural returns](../pipeline/05_checked-trees-to-lowered-psi/src/returns/structural_return.rs)
and [suspension retention](../pipeline/05_checked-trees-to-lowered-psi/src/retention/suspension_call_plan.rs)
own these lowering responsibilities. Native task planning remains downstream;
a retained call plan alone does not establish native execution coverage.

## Coverage and validation

Use [the testing guide](../../../tools/testing.md) for current commands and
[TASKS.md](../../../TASKS.md) for unfinished work. Stage-local tests live beside
the lowering owners; compiler corpus cases establish end-to-end coverage.
The [scalar-call migration note](../../../wiki/drafts/reference/scalar_call_lowering.md)
preserves historical algorithm context, not a current capability list.
