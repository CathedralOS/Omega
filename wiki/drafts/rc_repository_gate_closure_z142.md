# RC-REPOSITORY-GATE-CLOSURE — linux x86-64 leg

Record of the `RC-REPOSITORY` release gate block from
[rust_compiler_completion.md](rust_compiler_completion.md#release-matrix) run
on linux x86_64 (swarm VM, cargo — `mbx` unavailable) at commit
`a1daf35f2e` (origin/main, 2026-09-20). This leg re-measures the gate after
`f1e9a3733d` ([rc_repository_baseline_linux_x86_64.md](rc_repository_baseline_linux_x86_64.md))
and `a9fa1a4fe6` (the `RC-REPOSITORY-CLOSURE.` board row) and repairs the
unfenced fmt drift.

**Gate does not close: all five commands are still red at this head.**
Every remaining failure sits inside a live sibling claim; nothing unattributed
was left unfixed by this leg.

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | 1 | 59 hunks across 24 files at measurement; this leg formatted the 8 unclaimed files — residual is exactly the 16 claim-fenced files below |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 101 | 1 lint error: `clippy::permissions_set_readonly_false` (unchanged) |
| 3 | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | 100 | 573 tests: 560 passed, 13 failed |
| 4 | `cargo check --workspace --all-targets` | 101 | hard errors in `omega-native-differential-test` `pipeline_ownership` (4× E0308 + 1× E0004) |
| 5 | `cargo nextest run --workspace --lib --no-fail-fast` | 100 | 15,934 tests in 1112.5s: 15,846 passed (31 slow), 88 failed, 2 skipped |

## 1. fmt drift — 24 files at measurement

Unclaimed (8) — **formatted by this leg** (commit on branch
`zergling/z142-rc-repository-gate-closure`):

`compilation-report/src/terminal_product.rs`,
`compiler/tests/canary_suite/task_runtime.rs`,
`compiler/tests/module_machine_indices.rs`,
`compiler/tests/package_compilation_inputs/module_constants/lexical_aggregate_values.rs`,
`checked-trees-to-lowered-psi/tests/integer_policy_realization.rs`,
`typed-trees-to-checked-trees/src/execution/scalar/tests/record_locals.rs`,
`validation/src/declarations/operators/applications/tests.rs`,
`validation/src/proof_contracts/contract_entailment/specification_calls.rs`.

Claim-fenced residual (16) — left untouched:

- `external-roots` interrupt_table {member_admissions, tests,
  member_admission_and_publication} + stack_demand,
  `compiler/tests/layout_plans/interrupt_descriptor_tables.rs`,
  `module_machine_indices/comparisons.rs`,
  `calling-conventions/src/lib.rs`,
  t2c2 `checks/termination/progress/origins{,/tests,/tests/references}.rs`
  → RC-REPOSITORY-BASELINE-GREEN
- `terminal-psi-to-abstract-operations` structural_scalar_fields.rs
  → RANKED-NATIVE-ADMISSION
- t2c2 `checks/multiplicity/{borrowed_windows,linear_obligations}.rs`
  → ADDRESS-TRANSLATION-CANARY
- t2c2 `execution/unit/structural_scalar_store/tests/mod.rs`
  → PSI-NATIVE-FIELD-STORES
- t2c2 `tests/token_bound_machine_calls.rs`
  → CORPUS-RED-FAMILY-CASTSEED
- `validation/src/value_custody/expression_types/match_dispatch.rs`
  → MATCH-SELECTIVE-LOWERING

## 2. clippy

`packages/sources/acquisition/src/tree/capture/traversal.rs:513`
`writable.set_readonly(false)` — `permissions_set_readonly_false`, in the
`package-source` lib test. File is fenced to BUILD-PACKAGES-GATE; unchanged
since `f1e9a3733d`.

## 3. architecture — 13 failures

`glob_self_imports_never_grow_per_crate` (ratchet; sibling glob legs in
flight), `representation_ownership
exit_replay_checks_claimed_records_without_reentering_the_producer`, and
11 `validation_integration` recast-witness/provider-receiver rows
(boundary_ensures_*, symbolic_walk_*, admitted_provider_receiver_receipt,
checked_progress_retains_provider_receiver) — the in-flight RECAST lane.

## 4. check — `pipeline_ownership` compile errors

`tests/native-differential/tests/pipeline_ownership`: 4× E0308 mismatched
`validate_optimized_selection_custody` argument types
(stages/realization/structural_units/structural_return.rs:33,
stages/selection/custody.rs:62, validation.rs:400, validation.rs:409) and
E0004 non-exhaustive `&mut LegalizedScalarTerminator::Crash` at
fixtures/ordinary_graph_controls.rs:32 — harness drift behind a signature
change. All four files fenced to STRUCTURAL-UNIT-CALL-GRAPH-JOINS.

## 5. workspace lib run — 88 failures, 2 skipped

Failure clusters (deduplicated): checked-trees-to-lowered-psi 33,
selected-dispatch 32, abstract-operations-to-abstract-operations 7,
package-manager 6, validation 2, source-files-to-assembled-syntax 2,
calling-conventions 2, register-environment 1, native-realization 1,
external-roots 1, abstract-operations-to-target-operations 1 — same cluster
families as `f1e9a3733d` (94 fails) and `61da8491a2` (95 fails); the count
trends down but remains red.
