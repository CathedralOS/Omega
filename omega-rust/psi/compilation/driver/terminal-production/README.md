# Terminal production

Public contract: [Terminal Psi product](../../../../../wiki/spec/terminal-psi/product.md).
This Psi-owned operation sequences checked-program lowering, optimization and
canonical publication. Product consumers call it directly; there is no Omega
Terminal-production wrapper or alternative stage sequence.

Enter [terminal_production.rs](src/terminal_production.rs):
`checked-trees-to-lowered-psi` -> `lowered-psi-to-lowered-psi` ->
`lowered-psi-to-terminal-psi`. `LoweredPsi` belongs to representations.
Publication consumes the validated optimization-stage result.

`TerminalProductionRequest` carries the checked trees, the
`TerminalMachineSelection` naming the machine, and the exact Psi optimization
selection. Its one `produce` entry takes a `TerminalProductionCustody` and a
caller-owned observer. Product callers supply their own timing adapter and
composition policy; Psi retains no Omega reporting dependency. The custody
holds ordered application rows, each with optional ProgramEntry
source-signature identity and an optional composition admission that a product
caller fills from its `TerminalBehaviorExclusions` policy. Every shared
entry closure runs the same
lowering, optimization and publication legs; an entry identity
adds the ProgramEntry receipt legs, which check the unique Unit entry before
publication and rejoin the published semantic identity afterward. The
`ProducedTerminalProgram` returns one artifact beside each application's
refusal or requested entry receipt. A refusal does not reject sibling applications,
and successful artifact production alone admits none of them. No lowering or
unoptimized clone leaves production. Before optimization,
each policy borrows the same Terminal module and a diagnostic attribution
companion. Exclusion admission uses exact service owners retained in boundary
declarations, not checked trees or a callable-name lookup. The same operation
admits program entries and callback thunks; an admission cannot mutate the
module, replace a stage or choose whether optimization runs.

Permission-ledger validation belongs to the stage-05 lowering entrance, not
this coordinator. Its work is included in the lowering timing row; there is
no separate caller-owned ledger-check leg.

Terminal production owns its observation labels and failure classification.
`PsiCompilationStage::observation_labels` supplies stable report names and
input/output labels; the Omega caller only merges their measured rows.
`TerminalArtifactProductionError::diagnostics` distinguishes missing
implementation, incomplete proof search and ordinary rejection. Neither requires
a dependency on the Omega reporting accumulator.

The [stage-07 entry receipt owner](../../pipeline/07_lowered-psi-to-terminal-psi/src/entry_receipt.rs) checks the
unique Unit entry, records its identity and delegates explicit context eligibility
to its own child before publication.

ProgramEntry retains source-derived eligibility for the exact declared context
parameter when its record is ZII-valid and needs no executable nominal cleanup.
Keep machine, parameter and declared data-type identities distinct. The former
`Self`/attachment/receiver join is migration debt; the
[machine contract](../../../../../wiki/spec/language/machines.md#static-declarations-and-explicit-state)
requires ordinary explicit parameters, including in dynamic callers.
An absent eligibility row is not permission to infer validity from layout.
The shared cleanup query also serves ordinary
checked cleanup planning. This receipt is checked-source correspondence, not a
source-free proof, a storage grant, or Fused service establishment; native entry
must still supply those independent obligations.

Boundary application demands, checked body candidates, selected comparison
demands and callback applications live in the canonical artifact. Source-call
records do not cross publication. Callback applications identify their registrar,
requirement and checked contracts; target-owned placement and body receipts
still supply the distinct ABI and installation evidence.

## Lowering owners

This coordinator does not own the lowerers' feature dispatch. Start with
[checked execution selection](../../pipeline/04_typed-trees-to-checked-trees/src/execution/terminal_unit/mod.rs),
then [machine lowering](../../pipeline/05_checked-trees-to-lowered-psi/src/machine_lowering.rs).
[Source custody](../../pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/mod.rs)
prepares expressions and places; the
[call catalog](../../pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit/call_catalog.rs)
owns shared call lowering.

### Partial ownership and cleanup

[Cleanup lowering](../../pipeline/05_checked-trees-to-lowered-psi/src/unit/unit_cleanup.rs)
and its partial-cleanup children own residual destruction.
The [ownership contract](../../../../../wiki/spec/terminal-psi/ownership.md) defines
what must survive into Terminal Psi; successful helper tests do not establish
every source ownership shape's end-to-end coverage.

Whole owned Linear values retain their existing claim identities and typed paths
through local binding moves, structural state arguments including swaps, and
ordinary terminal calls, including selected conditional and guarded arms with
scalar or Unit completion. Each arm retains its exact authored invocation and
transfers the inherited owner only on that executed path. An explicit consuming
call may finish
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
execute the complete disposition path in both engines. The
[conditional scalar fixture](../../../../../tests/omega/pass/calls/runtime_linear_guarded_owner_disposition_exit/main.omg)
and [guarded Unit fixture](../../../../../tests/omega/pass/calls/runtime_linear_guarded_unit_owner_disposition_exit/main.omg)
exercise both alternatives without duplicating or renaming the inherited claim.

### Structural predicate production

[Contract predicates](../../pipeline/05_checked-trees-to-lowered-psi/src/proofs/contract_predicates.rs)
lower checked facts under the
[structural-predicate contract](../../../../../wiki/spec/terminal-psi/structural_predicates.md).
Claim identity, projection path, and equal scalar values are not interchangeable.

### Structural access and stores

[Structural source custody](../../pipeline/05_checked-trees-to-lowered-psi/src/expression_preparation/bindings/structural_paths.rs)
prepares accesses; [store emission](../../pipeline/05_checked-trees-to-lowered-psi/src/emission/structural_scalar_store.rs)
consumes prepared subjects.
The [structural-access contract](../../../../../wiki/spec/terminal-psi/structural_access.md)
is authoritative, not the current bounded producer vocabulary.

### Structural results and suspension

A structural or scalar result is an ordinary body lowered through the
[Unit closure](../../pipeline/05_checked-trees-to-lowered-psi/src/unit/attached_unit.rs).
[Suspension retention](../../pipeline/05_checked-trees-to-lowered-psi/src/retention/suspension_call_plan.rs)
owns retained call plans. Native task planning remains downstream;
a retained call plan alone does not establish native execution coverage.

## Coverage and validation

Use [the testing guide](../../../../../tools/testing.md) for current commands and
[TASKS.md](../../../../../TASKS.md) for unfinished work. Stage-local tests live beside
the lowering owners; compiler corpus cases establish end-to-end coverage.
The [scalar-call migration note](../../../../../wiki/drafts/reference/scalar_call_lowering.md)
preserves historical algorithm context, not a current capability list.
