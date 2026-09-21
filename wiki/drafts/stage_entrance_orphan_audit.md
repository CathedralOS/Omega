# Stage-entrance orphan audit

Mechanical sweep of every `omega-rust/{psi,omega}/pipeline/*` crate at
`d3d3193d59` (21 crates): enumerate each crate's public `pub fn` surface at
top-level `src/` files, then resolve every external caller — path-qualified
`crate::fn` references, `use crate::fn` imports, and bare calls in files that
import the crate. An orphan is a designed stage entrance with no caller
outside its own crate; integration-test-only and internally-routed entries are
cataloged, not flagged.

## Catalog

21 crates, 61 top-level `pub fn`s enumerated. Resolved without further sweep:

- Single-entrance crates whose one entry is wired (`source-files-to-tokens`
  exposes none at top level — `Lex`/`Parse` flow through typed re-exports —
  plus `lowered-psi-to-lowered-psi`, `checked-trees-to-lowered-psi`,
  `terminal-psi-to-abstract-operations`, `assembled-syntax-to-checked-compilation`,
  `register-homes-to-post-allocation-machine`,
  `selected-instructions-to-register-homes`,
  `target-operations-to-selected-instructions`,
  `source-files-to-assembled-syntax`).
- Entrances consumed only through `use` imports (bare calls): the
  `build-time-evaluation` consumers of `resolve_const_argument_selection`,
  `resolve_numeric_probe`, `prepare_const_initializer_selection`, and
  `requires_const_initializer_evaluation`; `optimize_abstract_operations`
  (native-realization's abstract-optimization module);
  `validate_optimized_layout_independent_selected_form_encoding`,
  `execute_resolved_layout_optimization`, and both
  `selected-form-encoding-to-resolved-layout` validators (machine-emission +
  native-differential pipeline-ownership fixtures).
- Test-only kept variants: none. **Corrected 2026-09-21:** this row named
  `lower_symbol_resolved_trees_owned`
  (symbol-resolved-trees-to-typed-trees), which does not exist anywhere in the
  tree. The `authored_declaration_selection_ledger` test imports the ordinary
  entrance `lower_symbol_resolved_trees` (`src/lowerer.rs:16`), so there is no
  alternate entrance to keep. The crate's only `pub fn lower_*` entrances are
  that one plus `lower_symbol_resolved_trees_to_seeded_base` and
  `lower_seeded_extension` (`src/lowerer/seeded_continuation.rs:59,73`).
- Internal plumbing re-exported at crate root but not a stage entrance:
  `normalize_open_index_identities` (typed-trees-to-checked-trees; three
  internal calls in `checking.rs`) and
  `lower_to_target_operations_and_native_callbacks`
  (abstract-operations-to-target-operations; the `lower_to_target_operations`
  entrance delegates to it).
- Documented unsequenced family: `unsequenced_spill_stages/` under
  selected-instructions-to-register-homes — the README names them
  intentionally-unwired compiler-private boundaries validated by
  native-differential `register_allocation` tests and the architecture
  ladders; their sequencing question is the spill-family board cluster's, not
  an accidental orphan.

## Finding (repaired in this change)

`checked-compilation-to-terminal-artifact::validate_lowered_integer_comparison_custody`
was a true orphan: built, unit-tested (`integer_comparisons/tests.rs`), and
re-exported at the crate root, but invoked by no caller anywhere — while its
documented counterpart `validate_lowered_ieee_float_comparison_custody` is
re-exported through `compiler` and invoked in the `inspect-terminal` route
(`omega/src/inspection/mod.rs`). The integer-comparison custody rejoin the
validator exists to perform therefore ran in unit tests only, never at the
inspection boundary it was written for.

Disposition taken: wired beside its counterpart —
`compiler/src/lib.rs` re-export + one call in `inspect_terminal` after the
IEEE check. Witnessed: `omega inspect-terminal --machine Main::main
tests/omega/pass/arithmetic/bounded_guarded_remainder/main.omg` completes with
integer-comparison operations present (`IntegerLessOrEqual`, `IntegerLessThan`)
on this base.

## Post-allocation and spill-family entrances (POC sweep)

Second sweep at `36ffc8af87` over the post-allocation chain and the
`unsequenced_spill_stages/` family — the stages the POC board cluster
(rewrite catalog, spill sequencing/disposition, wrapper placement) orbits.
Result: **no orphans**.

- Post-allocation chain crates, all wired into the executable route:
  `stage_optimized_post_allocation_machine_plan`
  (native-realization `physical_pipeline`, native-differential
  `optimizer_corpus`/`terminal_byte_views`),
  `stage_optimized_layout_independent_selected_form_encoding` +
  `validate_optimized_layout_independent_selected_form_encoding`
  (machine-emission `function_realization`),
  `stage_optimized_resolved_selected_form_layout` +
  `validate_optimized_resolved_selected_form_layout` +
  `admit_resolved_machine_layout` (machine-emission `exit_contract`,
  native-differential layout stages), and
  `execute_resolved_layout_optimization`
  (machine-emission `function_realization`). Each also carries an
  `optimizer_source_organization` executable-route coordination marker.
- `selected-instructions-to-selected-instructions`:
  `optimize_selected_instructions` and
  `optimize_analyzed_selected_instructions` are invoked by
  native-realization's physical pipeline and register-allocation internals.
- `unsequenced_spill_stages/`: all 19 stage modules (abstract/generalized/
  recursive spill insertion, spill recovery actions/choice/worklist,
  reload value homes, stack slot coloring, logical spill operations,
  spill pseudo instructions, synthetic reload values, abstract spill
  access constraints and memory effects) expose a
  `validate_*`/`schedule_*`/`plan_*`/`assign_*`/`derive_*` entrance triple;
  every one is re-exported at the crate root and driven by the
  native-differential `register_allocation` suite plus architecture
  entrance gates. The per-stage `*_identity` helpers are internally routed:
  called by their own stage's validator and by the generalized sibling
  modules that compose base identities; they are plumbing, not entrances.
- `logical_spill_operations::{encode, decode}` and
  `stack_slot_coloring::{encode, decode}` match a codec naming convention;
  their plain names collide with unrelated `encode`/`decode` across the
  workspace, so caller attribution required path-qualified greps — both are
  consumed by the native-differential layout stages and crate-internal
  plumbing.

## Residual risk

The sweep covers `pub fn` entrances at top-level `src/*.rs`; the POC sweep
additionally enumerated each spill-stage module's public surface — codec
names that collide workspace-wide (`encode`, `decode`) need
qualified-identity conventions before an automated entrance gate can
classify them unambiguously. Deep re-export alias chains below a module
root stay approximate, matching the caller-scan convention.

The module-level leg is now a repeatable gate:
`tests/architecture/stage_crate_ownership.rs::stage_root_public_modules_have_external_consumers`
enumerates every stage crate's root `pub mod` and requires each to be
reached by an external qualified path or to contribute a name to the
root re-export surface; `INTERNAL_MODULES` catalogs the `source`
vocabulary exception (its types ride in `AssembledSyntax`'s public
fields). Its first live catch was `source-files-to-assembled-syntax`'s
`frontend` — loading/lexing/parsing machinery public at the crate root
but consumed only by `crate::source_assembly` — narrowed to `pub(crate)`.
