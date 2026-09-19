# ControlFlowCleanup Promotion

- Exact rule: ControlFlowCleanup
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; ledger/roster custody in `tests/native-differential/tests/abstract_publication/control_flow.rs::private_machine_pruning_projects_exact_roster_and_ledger_custody`
- Differential evidence: `tests/native-differential/tests/abstract_publication/control_flow.rs::non_adjacent_block_merges_replay_and_lower_in_both_target_families` replays the optimized projection and lowers it in both target families; corpus selection case `tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry`
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: target-independent Psi-phase rule; `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::rollback_to_empty_selection_rejoins_exact_ordinary_path_on_every_target` retains byte-identical artifacts under the exact-rule rollback on `HOSTED_NATIVE_TARGETS` (linux_x86_64, linux_arm64, macos_arm64, windows_x86_64)
- Measurement evidence: PENDING — no versioned compile-time or output-quality benchmark recorded
- Rollback: --disable-optimization ControlFlowCleanup

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review, an approved status, and a versioned
measurement. Backticked evidence pointers cite repository artifacts as
`path` or `path::subject`; the architecture gate resolves every one of them.
