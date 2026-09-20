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
- Test-only kept variants: `lower_symbol_resolved_trees_owned`
  (symbol-resolved-trees-to-typed-trees) — invoked only by the
  `authored_declaration_selection_ledger` integration test; retained as a
  deliberately-owned alternate entrance, not an orphan.
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

## Residual risk

The sweep covers `pub fn` entrances at top-level `src/*.rs`; it does not
measure orphan *modules* below the crate root beyond the documented
unsequenced family, nor `pub` re-export chains that alias names at deeper
paths. A repeatable form of this audit belongs in
`tests/architecture/` once the entrance naming convention is stated formally.
