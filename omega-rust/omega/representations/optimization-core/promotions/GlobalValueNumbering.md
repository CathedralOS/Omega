# GlobalValueNumbering Promotion

- Exact rule: GlobalValueNumbering
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; proof-certified CSE and phi fact custody in `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_proof_certified_cse_with_exact_fact_custody` and `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_proof_certified_phi_custody`
- Differential evidence: `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_local_cse_and_return_substitution`, `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_a_non_roster_order_dominating_leader`, and `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_phi_translated_join_bindings` replay optimized projections across local CSE, leader dominance, and join bindings
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: target-independent Psi-phase rule; `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::compatible_policy_gvn_projects_and_lowers_with_exact_fact_custody` and `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::compatible_policy_phi_gvn_and_wrapping_shift_identities_project_and_lower` lower optimized projections to linux_x64 and linux_arm64 target operations
- Measurement evidence: PENDING — no versioned compile-time or output-quality benchmark recorded
- Rollback evidence: `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::global_value_numbering_rollback_rejoins_exact_ordinary_path_on_every_target` — a `GlobalValueNumbering`-selected native build under `--disable-optimization GlobalValueNumbering` rejoins byte-identical semantic, proof, object, and image artifacts on `HOSTED_NATIVE_TARGETS`; `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::native_rollback_rejects_products_that_do_not_enter_native_realization` and `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::empty_rollback_request_leaves_no_release_receipt` pin the overlay's product gating and empty-request custody
- Rollback: --disable-optimization GlobalValueNumbering

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review, an approved status, and a versioned
measurement. Backticked evidence pointers cite
repository artifacts as `path` or `path::subject`; the architecture gate resolves
every one of them.
