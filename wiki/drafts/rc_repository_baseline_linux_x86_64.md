# RC-REPOSITORY baseline — linux x86-64

Record of the `RC-REPOSITORY` release gate block from
[rust_compiler_completion.md](rust_compiler_completion.md#release-matrix) run
on linux x86-64 (this session's VM, 8 CPUs) at commit `f1e9a3733d`
(origin/main, 2026-09-20). Commands were the gate block verbatim, with `cargo`
substituted for `mbx` per AGENTS.md. **Gate does not pass: 4 of 5 commands fail
at this head.**

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | 1 | 57 formatting diffs in 7 files (see below) |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 101 | 1 lint error: `clippy::permissions_set_readonly_false` |
| 3 | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | 100 | 568 tests: 554 passed, 14 failed |
| 4 | `cargo check --workspace --all-targets` | 101 | hard error E0277 (see below) |
| 5 | `cargo nextest run --workspace --lib --no-fail-fast` | 100 | 15,793 tests in 1,120.1s: 15,699 passed (31 slow), 94 failed, 2 skipped |

## 1. fmt diffs (57 hunks, 7 files)

`omega-rust/omega/backend/runtime/external-roots/src/interrupts/interrupt_table/{member_admissions.rs,tests.rs,tests/member_admission_and_publication.rs}`,
`.../stack_and_fuel/stack_demand.rs`,
`omega-rust/omega/compiler/compilation-report/src/terminal_product.rs`,
`omega-rust/omega/compiler/compiler/tests/canary_suite/task_runtime.rs`,
`omega-rust/omega/compiler/compiler/tests/layout_plans/interrupt_descriptor_tables.rs`.

## 2. clippy failure

`packages/sources/acquisition/src/tree/capture/traversal.rs:513`:
`writable.set_readonly(false)` — `clippy::permissions_set_readonly_false`
(on Unix the result is world-writable; use `PermissionsExt`). In lib test
`package-source`.

## 3. architecture failures (14)

Clustered in `omega-architecture-test::validation_integration` (e.g.
`boundary_witness_survives_transitive_disjoint_boundary_frame`,
`checked_progress_retains_provider_receiver_as_build_bound_demand`,
`symbolic_walk_recast_footprint_discharges`, `symbolic_walk_recast_wide_witness_refuses`,
`symbolic_walk_weak_guard_spelling_refuses`) plus the preexisting
entrypoint/layering set documented in `known_baseline_failures.md`.

## 4. check failure

`tests/native-differential/tests/abstract_publication/decision_custody.rs:58`:
`assert_eq!` compares `[Optimization; 6]` fixture against `PSI_PASS_CATALOG`
of 7 — a public Psi pass landed without an Applied-evidence custody fixture
("every public Psi pass must have an Applied-evidence custody fixture").

## 5. workspace lib run

94 failures, 2 skipped, 31 slow markers. Failure clusters (counts):
selected-dispatch 32, checked-trees-to-lowered-psi 30, terminal-codec 10,
abstract-operations-to-abstract-operations 7, package-manager 6,
source-files-to-assembled-syntax 3, calling-conventions 2,
selected-instructions-to-selected-instructions 1, register-environment 1,
native-realization 1, abstract-operations-to-target-operations 1 — the same
clusters as the controlled remeasurement in
[test_cycle_selection_remeasurement.md](test_cycle_selection_remeasurement.md)
at `61da8491a2` (95 fails there).

## Verdict

`RC-REPOSITORY` is open on this host at this commit — fmt, clippy,
architecture tests, and check all fail before the lib run is considered.
The two actionable defects are mechanical: run `cargo fmt` on the seven
listed files, use `PermissionsExt` in the acquisition traversal, and add the
missing decision-custody fixture for the seventh `PSI_PASS_CATALOG` entry.
