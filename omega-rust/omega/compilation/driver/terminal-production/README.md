# Terminal production

Public contract: [Terminal Psi product](../../../../../wiki/spec/terminal-psi/product.md).
This product-driver package sequences Psi stages; it owns neither language
judgments nor another executable IR. Both ordinary compilation and package
compilation use this sequence.

Enter [terminal_production.rs](src/terminal_production.rs). The sequence is
`checked-trees-to-lowered-psi` -> `lowered-psi-to-lowered-psi` ->
`lowered-psi-to-terminal-psi`. `LoweredPsi` belongs to representations.
Publication consumes the validated optimization-stage result, not an unoptimized
producer-private shortcut.

[custody.rs](src/custody.rs) owns retained products and the callback sidecar's
success/rejection contract; [stage_timings.rs](src/stage_timings.rs) owns sequence
observations. Neither implements Psi judgments.

[Optimization ownership](../../../../psi/compilation/pipeline/06_lowered-psi-to-lowered-psi/README.md)
keeps shared retention policy separate from pass execution.
[Publication ownership](../../../../psi/compilation/pipeline/07_lowered-psi-to-terminal-psi/README.md)
separates canonical sealing from checked boundary-operator occurrence replay.

`TerminalProductionRequest` carries the checked trees, the
`TerminalMachineSelection` naming the machine, and the exact Psi optimization
selection. Its one `produce` entry takes a `TerminalProductionCustody`: an
optional ProgramEntry source-signature identity, the caller's opaque callback
sidecar, and the stage-timing carrier. Every product runs the same
lowering, optimization, publication, and boundary-scope legs; an entry identity
adds the ProgramEntry receipt legs, which check the unique Unit entry before
publication and rejoin the published semantic identity afterward. The
`ProducedTerminalArtifact` returns the artifact beside that retained custody,
and rejection returns the original opaque sidecar without cloning it.

Permission-ledger validation belongs to the stage-05 lowering entrance, not
this coordinator. Its work is included in the lowering timing row; there is
no separate caller-owned ledger-check leg.

Terminal production owns its observation labels and failure classification.
`TerminalProductionStage::observation_labels` supplies stable report names and
input/output labels; the Omega caller only merges their measured rows.
`TerminalArtifactProductionError::diagnostic_message` distinguishes missing
implementation, incomplete proof search and ordinary rejection. Neither requires
a dependency on the Omega reporting accumulator.

The [stage-07 entry receipt owner](../../../../psi/compilation/pipeline/07_lowered-psi-to-terminal-psi/src/entry_receipt.rs) checks the
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
[checked execution selection](../../../../psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/mod.rs),
then [machine lowering](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/machine_lowering.rs).
[Source custody](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/source_custody/mod.rs)
prepares expressions and places; the
[call catalog](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/call_catalog.rs)
owns shared call lowering.

### Partial ownership and cleanup

[Cleanup lowering](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/unit/unit_cleanup.rs)
and its partial-cleanup children own residual destruction.
The [ownership contract](../../../../../wiki/spec/terminal-psi/ownership.md) defines
what must survive into Terminal Psi; successful helper tests do not establish
every source ownership shape's end-to-end coverage.

Whole owned Linear values retain their existing claim identities and typed paths
through local binding moves, structural state arguments including swaps, and
ordinary unconditional terminal calls. An explicit consuming call may finish
through the exact owner-designated `T::drop` plan for the admitted scalar-record
carrier. The cleanup edge consumes
that live claim while the hook receives only a temporary exclusive loan; an
ordinary borrowed method does not gain disposition authority. Source checking
and Terminal/Omega replay retain the hook's exact nominal designation and require
a normal, infallible, nonblocking, non-suspending contract without process exit
or additional runtime authority.

This coverage does not include joins that choose or rename claim lineages,
claim-bearing result joins, or disposition of erased or nested Linear children.
An owner hook cannot erase a child's independent obligation. Missing owner
permission, duplicate transfer, and live-owner overwrite remain rejected.
The maintained [local-move and terminal-call fixture](../../../../../tests/omega/pass/calls/runtime_linear_terminal_owner_disposition_exit/main.omg)
and [state-swap fixture](../../../../../tests/omega/pass/calls/runtime_linear_state_owner_disposition_exit/main.omg)
execute the complete disposition path in both engines.

### Structural predicate production

[Contract predicates](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/proofs/contract_predicates.rs)
lower checked facts under the
[structural-predicate contract](../../../../../wiki/spec/terminal-psi/structural_predicates.md).
Claim identity, projection path, and equal scalar values are not interchangeable.

### Structural access and stores

[Structural source custody](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/source_custody/structural/mod.rs)
prepares accesses; [store emission](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/emission/structural_scalar_store.rs)
consumes prepared subjects.
The [structural-access contract](../../../../../wiki/spec/terminal-psi/structural_access.md)
is authoritative, not the current bounded producer vocabulary.

### Structural results and suspension

[Structural returns](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/returns/structural_return.rs)
and [suspension retention](../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/retention/suspension_call_plan.rs)
own these lowering responsibilities. Native task planning remains downstream;
a retained call plan alone does not establish native execution coverage.

## Coverage and validation

Use [the testing guide](../../../../../tools/testing.md) for current commands and
[TASKS.md](../../../../../TASKS.md) for unfinished work. Stage-local tests live beside
the lowering owners; compiler corpus cases establish end-to-end coverage.
The [scalar-call migration note](../../../../../wiki/drafts/reference/scalar_call_lowering.md)
preserves historical algorithm context, not a current capability list.
