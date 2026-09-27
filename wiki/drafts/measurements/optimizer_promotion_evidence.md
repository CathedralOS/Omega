# Incomplete optimizer promotion evidence

Snapshot from the Rust reference tree at `96e1ebbe9f` (2026-09-27).
This preserves evidence pointers for a future exact-rule promotion review;
no rule is approved or made default by this document. The former source-tree
records referred to an architecture gate that is no longer present. Their
claims below are historical evidence pointers, not checks rerun by this sweep.

The [promotion contract](../../spec/build/optimizations.md#release-rollback-and-promotion)
remains authoritative. Delete this draft when these candidates are either
reviewed into completed promotion records or abandoned. Tests and measurement
artifacts stay with their existing owners; do not create a parallel execution board.

## ControlFlowCleanup

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

## CopyPropagation

- Exact rule: CopyPropagation
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; the custody and corruption legs it also cited from the Rust native differential tests were removed on 2026-09-25
- Differential evidence: PENDING — the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/copy_propagation.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: PENDING — target-independent Psi-phase rule; the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-0b35b22c4221.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with CopyPropagation disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: PENDING — the byte-identical rejoin under `--disable-optimization CopyPropagation` was witnessed by compiler rollback tests deleted with the compiler test tree on 2026-09-25; a corpus-level rollback check has to replace them.
- Rollback: --disable-optimization CopyPropagation

## DeadPureScalarElimination

- Exact rule: DeadPureScalarElimination
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`
- Differential evidence: PENDING — the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/dead_pure_scalar_elimination.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: PENDING — target-independent Psi-phase rule; its per-target byte-identity witness was deleted with the compiler test tree on 2026-09-25.
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-2fe10c790188.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with DeadPureScalarElimination disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: PENDING — the byte-identical rejoin under `--disable-optimization DeadPureScalarElimination` was witnessed by compiler rollback tests deleted with the compiler test tree on 2026-09-25; a corpus-level rollback check has to replace them.
- Rollback: --disable-optimization DeadPureScalarElimination

## GlobalValueNumbering

- Exact rule: GlobalValueNumbering
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; the custody and corruption legs it also cited from the Rust native differential tests were removed on 2026-09-25
- Differential evidence: PENDING — the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/global_value_numbering.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: PENDING — target-independent Psi-phase rule; the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-b936e1ff6607.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with GlobalValueNumbering disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: PENDING — the byte-identical rejoin under `--disable-optimization GlobalValueNumbering` was witnessed by compiler rollback tests deleted with the compiler test tree on 2026-09-25; a corpus-level rollback check has to replace them.
- Rollback: --disable-optimization GlobalValueNumbering

## ProofCheckElision

- Exact rule: ProofCheckElision
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; the custody and corruption legs it also cited from the Rust native differential tests were removed on 2026-09-25
- Differential evidence: PENDING — the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/proof_check_elision.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: PENDING — target-independent Psi-phase rule; the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-2c860c734e64.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with ProofCheckElision disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: PENDING — the byte-identical rejoin under `--disable-optimization ProofCheckElision` was witnessed by compiler rollback tests deleted with the compiler test tree on 2026-09-25; a corpus-level rollback check has to replace them.
- Rollback: --disable-optimization ProofCheckElision

## SparseConditionalConstantPropagation

- Exact rule: SparseConditionalConstantPropagation
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs` — positive, negative, boundary, disabled, malformed-carrier (`malformed_carriers_fail_admission`), and forged-run replay legs (`forged_run_axes_fail_publication_replay`) through `publish_optimization_run`; the custody and corruption legs it also cited from the Rust native differential tests were removed on 2026-09-25
- Differential evidence: PENDING — the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Determinism and bounded-work evidence: `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs::repeated_runs_are_deterministic` and `omega-rust/omega/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager/tests/evidence_matrix/sparse_conditional_constant_propagation.rs::measured_budget_admits_exact_usage_and_refuses_one_less`
- Target matrix evidence: PENDING — target-independent Psi-phase rule; the Rust native differential tests previously cited here were removed on 2026-09-25; end-to-end differential coverage is the corpus native leg (`python3 tools/corpus_gate.py --native`, with `--both` comparing the checked interpreter).
- Measurement evidence: `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-b5ab8878609b.json` — versioned compile-time, peak-memory, code-size, and runtime row measuring `wrapping_square_sum` under the complete six-rule Psi selection with SparseConditionalConstantPropagation disabled, beside the all-enabled baseline `tools/benchmark/records/wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` on linux_x86_64
- Rollback evidence: PENDING — the byte-identical rejoin under `--disable-optimization SparseConditionalConstantPropagation` was witnessed by compiler rollback tests deleted with the compiler test tree on 2026-09-25; a corpus-level rollback check has to replace them.
- Rollback: --disable-optimization SparseConditionalConstantPropagation
