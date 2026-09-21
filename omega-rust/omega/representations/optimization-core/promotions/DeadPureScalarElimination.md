# DeadPureScalarElimination Promotion

- Exact rule: DeadPureScalarElimination
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`
- Differential evidence: `tests/native-differential/tests/abstract_publication/dead_scalar_elimination.rs::dead_scalar_literal_elimination_replays_transitive_fuel_to_the_terminal` and `tests/native-differential/tests/abstract_publication/dead_scalar_elimination.rs::dead_scalar_suite_removes_total_arithmetic_then_its_dead_operands` replay published runs to the terminal node with provenance and fuel custody
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: target-independent Psi-phase rule; `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::dead_scalar_rollback_rejoins_exact_ordinary_path_on_every_target` retains byte-identical artifacts under the exact-rule rollback on `HOSTED_NATIVE_TARGETS` (linux_x86_64, linux_arm64, macos_arm64, windows_x86_64) for a program carrying dead scalar work
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-2fe10c790188.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with DeadPureScalarElimination disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::dead_scalar_rollback_rejoins_exact_ordinary_path_on_every_target` — a `DeadPureScalarElimination`-selected native build under `--disable-optimization DeadPureScalarElimination` rejoins byte-identical semantic, proof, object, and image artifacts on `HOSTED_NATIVE_TARGETS`; `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::native_rollback_rejects_products_that_do_not_enter_native_realization` and `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::empty_rollback_request_leaves_no_release_receipt` pin the overlay's product gating and empty-request custody
- Rollback: --disable-optimization DeadPureScalarElimination

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review and an approved status. Backticked
evidence pointers cite repository artifacts as `path` or `path::subject`; the
architecture gate resolves every one of them.
