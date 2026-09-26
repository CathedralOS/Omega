# ControlFlowCleanup Promotion

- Exact rule: ControlFlowCleanup
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; the custody and corruption legs it also cited from the Rust native differential tests were removed on 2026-09-25
- Differential evidence: the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter); corpus selection case `tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry`
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/control_flow_cleanup.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: PENDING — target-independent Psi-phase rule; its per-target byte-identity witness was deleted with the compiler test tree on 2026-09-25.
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-bacb0af6ca52.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with ControlFlowCleanup disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: PENDING — the byte-identical rejoin under `--disable-optimization ControlFlowCleanup` was witnessed by compiler rollback tests deleted with the compiler test tree on 2026-09-25; a corpus-level rollback check has to replace them.
- Rollback: --disable-optimization ControlFlowCleanup

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review and an approved status. Backticked
evidence pointers cite repository artifacts as `path` or `path::subject`; the
architecture gate resolves every one of them.
