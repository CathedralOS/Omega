# ControlFlowCleanup Promotion

- Exact rule: ControlFlowCleanup
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `pass_manager/tests/evidence_matrix/control_flow_cleanup.rs` in `abstract-operations-to-abstract-operations` — positive, negative, boundary, disabled, malformed-carrier, and forged-run replay legs through `publish_optimization_run`; ledger/roster custody in `tests/native-differential/tests/abstract_publication/control_flow.rs`
- Differential evidence: `tests/native-differential/tests/abstract_publication/control_flow.rs` replays the optimized projection and lowers it in both target families; corpus selection case `tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry`
- Determinism and bounded-work evidence: `repeated_runs_are_deterministic` and `measured_budget_admits_exact_usage_and_refuses_one_less` in the same evidence-matrix file
- Target matrix evidence: target-independent Psi-phase rule; `no_selection_golden/rollback.rs::rollback_to_empty_selection_rejoins_exact_ordinary_path_on_every_target` retains byte-identical artifacts under the exact-rule rollback on `HOSTED_NATIVE_TARGETS` (linux_x86_64, linux_arm64, macos_arm64, windows_x86_64)
- Measurement evidence: PENDING — no versioned compile-time or output-quality benchmark recorded
- Rollback: --disable-optimization ControlFlowCleanup

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review, an approved status, and a versioned
measurement.
