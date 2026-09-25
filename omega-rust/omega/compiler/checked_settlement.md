# Checked-phase settlement custody

Psi owns [source checking](../../../wiki/spec/language/state_contracts.md).
This coordinator closes build/target inputs around that phase; it does not add
target policy to the portable Terminal module. Enter
[phase_transitions.rs](src/checked/checking/phase_transitions.rs).

[Provider selection](../build/provider-planning/src/provider_planning/selection.rs) closes the final typed
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
ordered work closes component-entry progress, selected-dispatch settlement,
provenance, boundary-adapter associations, and task activations. Preserve that order and
each independent sidecar when moving coordinator code; duplicating or reordering
settlement is not a new pipeline stage.

Boundary calls remain canonical in checked trees. Selected execution records
exact receiver/requirement/entry-state associations and whether the receiver is
forwarded. The interpreter consumes those associations before host dispatch;
neither selection nor evaluation fabricates replacement source calls. Terminal
publication borrows the checked program directly. Package review reads the same
authored boundary calls and independently checks their write frames.

Settlement changes no checked body, so package source queries read the
settled trees directly. An operator application whose selected provider is a
checked adapter or a compiler-known float realization keeps naming the
operator; until Terminal Psi carries a requirement-level operator application
for Omega to install, the machine applying it has no Unit plan and the
omission reports `unimplemented:`.
