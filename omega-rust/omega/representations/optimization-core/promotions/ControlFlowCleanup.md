# ControlFlowCleanup Promotion

- Exact rule: ControlFlowCleanup
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; ledger/roster custody in `tests/native-differential/tests/abstract_publication/control_flow.rs::private_machine_pruning_projects_exact_roster_and_ledger_custody`
- Differential evidence: `tests/native-differential/tests/abstract_publication/control_flow.rs::non_adjacent_block_merges_replay_and_lower_in_both_target_families` replays the optimized projection and lowers it in both target families; corpus selection case `tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry`
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: target-independent Psi-phase rule; `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::rollback_to_empty_selection_rejoins_exact_ordinary_path_on_every_target` retains byte-identical artifacts under the exact-rule rollback on `HOSTED_NATIVE_TARGETS` (linux_x86_64, linux_arm64, macos_arm64, windows_x86_64)
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-bacb0af6ca52.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with ControlFlowCleanup disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::rollback_to_empty_selection_rejoins_exact_ordinary_path_on_every_target` — a `ControlFlowCleanup`-selected native build under `--disable-optimization ControlFlowCleanup` rejoins byte-identical semantic, proof, object, and image artifacts on `HOSTED_NATIVE_TARGETS`; `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::native_rollback_rejects_products_that_do_not_enter_native_realization` and `omega-rust/omega/compiler/compiler/tests/no_selection_golden/rollback.rs::empty_rollback_request_leaves_no_release_receipt` pin the overlay's product gating and empty-request custody
- Rollback: --disable-optimization ControlFlowCleanup

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review and an approved status. Backticked
evidence pointers cite repository artifacts as `path` or `path::subject`; the
architecture gate resolves every one of them.
