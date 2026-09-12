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
provenance, boundary adapters, and task activations. Preserve that order and
each independent sidecar when moving coordinator code; duplicating or reordering
settlement is not a new pipeline stage.

`SelectedDispatchSourceEdits` privately seals replaced expression/statement nodes
and their operand, binding, and type custody. Source queries reverse replay the
settlements into one scratch typed tree. Replay preserves unrelated edits and
generated declarations; it neither trusts the selected execution tree as original
source nor resets the entire tree to an old snapshot. Mutation review still
requires exact authored write-frame equality.

[Terminal production](../../../psi/compiler/terminal-production/README.md#boundary-source-custody-at-the-compiler-handoff)
documents the transitional inverse-edit handoff. It is implementation custody,
not permission to serialize selected providers as authored requirement calls.
