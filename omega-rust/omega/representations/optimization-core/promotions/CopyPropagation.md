# CopyPropagation Promotion

- Exact rule: CopyPropagation
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; call-result effect custody in `tests/native-differential/tests/abstract_publication/copy_propagation.rs::copy_propagation_preserves_scalar_call_result_effect_and_custody`; corrupted projection rejection in `tests/native-differential/tests/abstract_publication/corruption.rs::independent_validation_rejects_block_offset_corruption`
- Differential evidence: `tests/native-differential/tests/abstract_publication/copy_propagation.rs::copy_propagation_projects_shortened_blocks_and_rewritten_edges` replays the optimized projection and lowers it into target operations
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: target-independent Psi-phase rule; `tests/native-differential/tests/abstract_publication/copy_propagation.rs::copy_propagation_projects_shortened_blocks_and_rewritten_edges` lowers the optimized projection to linux_x64 target operations
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-0b35b22c4221.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with CopyPropagation disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: PENDING — the byte-identical rejoin under `--disable-optimization CopyPropagation` was witnessed by compiler rollback tests deleted with the compiler test tree on 2026-09-25; a corpus-level rollback check has to replace them.
- Rollback: --disable-optimization CopyPropagation

This is a staged record: the inventory row remains `Experimental` and opt-in
until the workspace gate passes. The `PENDING` fields name the promotion-contract
evidence still missing — owner review and an approved status. Backticked
evidence pointers cite repository artifacts as `path` or `path::subject`; the
architecture gate resolves every one of them.
