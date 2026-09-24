# ProofCheckElision Promotion

- Exact rule: ProofCheckElision
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/04_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; live-identity fact and fuel custody in `tests/native-differential/tests/abstract_publication/proof_check_elision.rs::proof_check_elision_projects_live_exact_identity_with_fact_and_fuel_custody`
- Differential evidence: `tests/native-differential/tests/abstract_publication/proof_check_elision.rs::proof_check_elision_projects_dead_exact_work_and_retains_evidence` and the live-identity family in `tests/native-differential/tests/abstract_publication/proof_check_elision.rs` replay elision projections for dead and live proof-certified work
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/04_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/04_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: target-independent Psi-phase rule; `tests/native-differential/tests/abstract_publication/proof_check_elision.rs::proof_check_elision_projects_signed_remainder_by_negative_one_to_both_targets` and `tests/native-differential/tests/abstract_publication/proof_check_elision.rs::proof_check_elision_projects_exact_signed_negative_one_shift_right_to_both_targets` lower optimized projections to linux_x64 and linux_arm64 target operations
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-2c860c734e64.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with ProofCheckElision disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::proof_check_elision_rollback_rejoins_exact_ordinary_path_on_every_target` — a `ProofCheckElision`-selected native build under `--disable-optimization ProofCheckElision` rejoins byte-identical semantic, proof, object, and image artifacts on `HOSTED_NATIVE_TARGETS`; `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::native_rollback_rejects_products_that_do_not_enter_native_realization` and `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::empty_rollback_request_leaves_no_release_receipt` pin the overlay's product gating and empty-request custody
- Rollback: --disable-optimization ProofCheckElision

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review and an approved status. Backticked
evidence pointers cite repository artifacts as `path` or `path::subject`; the
architecture gate resolves every one of them.
