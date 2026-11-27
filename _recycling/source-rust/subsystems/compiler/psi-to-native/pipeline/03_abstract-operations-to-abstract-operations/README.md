# Archived native stage 03: abstract-operations optimizer

This crate optimized the abstract program between native stages 02 and 04. It
never ran a pass in the shipped compiler. The native driver handed it
post-Terminal optimization selections, which refuse every Psi-phase
optimization, and no optimization has the `AbstractOperations` phase. Every
compile therefore took its empty-selection path. That path rebuilt and
revalidated the unit, hashed it twice and compared the published plan with its
input before passing the plan on unchanged. The receiving native lowerer is not
meant to rerun Psi passes at all
([Terminal Psi product](../../../../../../../wiki/spec/terminal-psi/product.md#optimization-ordering)).
The live Psi optimizer is
[`publication/optimization.rs`](../../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/publication/optimization.rs).

Stage 04 now builds the optimization unit itself
([`abstract_input.rs`](../../../../../../../source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/abstract_input.rs)).
That module also carries this crate's evidence-seal checks. Stage 04 issues the
same identity records the empty run produced
([`OptimizedAbstractProgram::unoptimized`](../../../../../../../source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/unoptimized.rs)).

The passes are kept to be ported as Psi-to-Psi passes:
- global value numbering (`global_value_numbering.rs`);
- control-flow cleanup (`control_flow_cleanup.rs`, `control_flow_analyses.rs`);
- proof-check elision (`proof_check_elision.rs`);
- SCCP, countdown and ranked rewrites, and specialization (`sccp.rs`,
  `countdown_analyses.rs`, `ranked_rewrites.rs`, `specialization.rs`).

They depend on validators that were cut from the live crates and archived beside
them, at the same paths under `_recycling/`:
- `representations/abstract-operations/optimization-unit-semantics/src/`: the
  rewrite-candidate replay (`candidates.rs`), the whole-unit validator
  (`unit_validation.rs`), ownership-frontier replay (`current_ownership.rs`) and
  value-range checks (`current_value_ranges.rs`).
- `representations/abstract-operations/optimization-unit/src/`: the
  rewrite-candidate records (`rewrite.rs`) and the analysis helpers
  (`structural_places.rs`, `observation.rs`, `range.rs`).

The archived `unit_validation.rs`, `current_ownership.rs`, `lib.rs`,
`error.rs` and `operation_metadata.rs` are complete copies from before the cut.
The live files at the same paths keep only what stages 04 and 15 query.
