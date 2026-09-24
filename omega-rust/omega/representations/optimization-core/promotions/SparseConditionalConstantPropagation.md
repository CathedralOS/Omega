# SparseConditionalConstantPropagation Promotion

- Exact rule: SparseConditionalConstantPropagation
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; corrupted commit custody rejection in `tests/native-differential/tests/abstract_publication/corruption.rs::candidate_replay_rejects_corrupted_commit_custody`
- Differential evidence: `tests/native-differential/tests/abstract_publication/sparse_conditional_constants.rs::proof_certified_exact_fold_projects_and_remains_target_lowerable` replays the optimized projection, retains its obligation-fact custody, and lowers it into target operations
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: target-independent Psi-phase rule; `tests/native-differential/tests/abstract_publication/sparse_conditional_constants.rs::proof_certified_exact_fold_projects_and_remains_target_lowerable` lowers the optimized projection to linux_x64 target operations
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-b5ab8878609b.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with SparseConditionalConstantPropagation disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::sparse_conditional_constant_propagation_rollback_rejoins_exact_ordinary_path_on_every_target` — a `SparseConditionalConstantPropagation`-selected native build under `--disable-optimization SparseConditionalConstantPropagation` rejoins byte-identical semantic, proof, object, and image artifacts on `HOSTED_NATIVE_TARGETS`; `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::native_rollback_rejects_products_that_do_not_enter_native_realization` and `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::empty_rollback_request_leaves_no_release_receipt` pin the overlay's product gating and empty-request custody
- Rollback: --disable-optimization SparseConditionalConstantPropagation

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review and an approved status. Backticked
evidence pointers cite repository artifacts as `path` or `path::subject`; the
architecture gate resolves every one of them.
