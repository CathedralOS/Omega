# GlobalValueNumbering Promotion

- Exact rule: GlobalValueNumbering
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; proof-certified CSE and phi fact custody in `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_proof_certified_cse_with_exact_fact_custody` and `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_proof_certified_phi_custody`
- Differential evidence: `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_local_cse_and_return_substitution`, `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_a_non_roster_order_dominating_leader`, and `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::global_value_numbering_projects_phi_translated_join_bindings` replay optimized projections across local CSE, leader dominance, and join bindings
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: target-independent Psi-phase rule; `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::compatible_policy_gvn_projects_and_lowers_with_exact_fact_custody` and `tests/native-differential/tests/abstract_publication/global_value_numbering.rs::compatible_policy_phi_gvn_and_wrapping_shift_identities_project_and_lower` lower optimized projections to linux_x64 and linux_arm64 target operations
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-b936e1ff6607.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with GlobalValueNumbering disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: PENDING — the byte-identical rejoin under `--disable-optimization GlobalValueNumbering` was witnessed by compiler rollback tests deleted with the compiler test tree on 2026-09-25; a corpus-level rollback check has to replace them.
- Rollback: --disable-optimization GlobalValueNumbering

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review and an approved status. Backticked
evidence pointers cite repository artifacts as `path` or `path::subject`; the
architecture gate resolves every one of them.
