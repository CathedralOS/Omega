# Checked-phase settlement custody

Psi owns [source checking](../../../../wiki/spec/language/state_contracts.md).
This coordinator closes build/target inputs around that phase; it does not add
target policy to the portable Terminal module. Enter
[phase_transitions.rs](src/pipeline/phase_transitions.rs).

[Provider selection](src/pipeline/provider/selection.rs) closes the final typed
target roster before checking: target defaults and via bindings, candidate
validation, selection, fused erasure authorization, synchronous-cycle validation,
external bindings, and selected facts/provenance, in that order. Its result owns
the existing plan evidence, not the typed program. The checked coordinator then
evaluates deferred constants and enters typed-to-checked settlement; no provider
selection or build execution is repeated at that boundary.

`CheckedProgramSurface` retains checked Psi with exact predecessor facts:
selected provider plans/grants, callback placements, accepted template
classifications, and contract-entailment stand-downs. `TypedToCheckedSettlementInput`
closes callback and provider receipts transactionally before checked ownership
is shared. A compact identifier is not a replacement for these retained inputs.

Selected execution then produces `SelectedExecutionSettlementSurface`. Its
ordered work closes component-entry progress, operator/float selections,
provenance, boundary-adapter associations, and task activations. Preserve that order and
each independent sidecar when moving coordinator code; duplicating or reordering
settlement is not a new pipeline stage.

Boundary calls remain canonical in checked trees. Selected execution records
exact receiver/requirement/entry-state associations and whether the receiver is
forwarded. The interpreter consumes those associations before host dispatch;
neither selection nor evaluation fabricates replacement source calls. Terminal
publication borrows the checked program directly. Package review reads the same
authored boundary calls and independently checks their write frames.

Operator/FMA settlement still rebuilds selected lowering plans and changes
operator expressions. Its expression-only `SelectedDispatchSourceEdits` guards
operand, binding and type custody for package source queries. This separate
operator mechanism is not used to restore boundary calls. No statement undo
journal or boundary restoration tree remains.
