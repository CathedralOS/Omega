# Release record — linux_x86_64 at `e12b9e8e06`

Bounded release-matrix record produced under board item RC-RELEASE-RECORD
(sibling legs: RC-RELEASE-RECORD-RUN, RUST-COMPILER-RELEASE-RECORD,
RC-RELEASE-CLOSURE-RUN, RC-RELEASE-RECORD-AND-CLOSURE, and the substrate
RC-RELEASE-RECORD-SUBSTRATE). The record substrate is
`tools/release/release_record.py` (schema `omega-release-record/1`); its
`records/` output directory is fenced to the substrate claim, so this leg
records the observed gate state as a draft document instead of writing a
committed JSON row.

- Host: linux x86-64 (`platform.machine()=x86_64`), Python 3.10.12
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`
- Runner: `cargo` (mbx unavailable on this host — same resolution the
  substrate's `runner()` performs)
- Commit: `e12b9e8e065695dc9b43633cac21bf12a236e6f5`

## Gate results (linux_x86_64)

| Gate | Status | Evidence |
| --- | --- | --- |
| RC-REPOSITORY | **fail** | `cargo fmt --all -- --check` exit 1: 23 files drifted at this base (listed below). Remaining gate commands (clippy workspace, `omega-architecture-test`, `check --workspace`, `nextest --workspace --lib`) not run within this leg. |
| RC-PORTABLE-PSI | **pass** | `cargo nextest run -p compiler --test canary_suite -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` — 1/1 PASS in 25.9s (producer/consumer envelope reload + refusal legs). |
| RC-DIAGNOSTICS | **fail** | `cargo nextest run -p compiler --test canary_suite -E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'` — FAIL in 148.6s; drifted diagnostics and two silent-acceptance regressions below. |
| RC-SOURCE-SEMANTICS | not run | `nextest -p compiler --all-targets` exceeds this leg's bound; see RC-DIAGNOSTICS row for the same canary suite's red state. |
| RC-PCC-REPLAY | not completed | Dispatched at this base; did not reach a verdict within this leg's bound — open row, not a pass. |
| RC-BUILD-AND-PACKAGES | not completed | Dispatched; package-manager suite was mid-run at cutoff (355/1662 executed, all observed PASS) — open row, not a pass. |
| RC-NATIVE-MATRIX | not run | Per required host; linux_x86_64 leg not dispatched within this leg's bound. |
| RC-REPRESENTATIVE-PROGRAMS | not run | Per required host; `samples_compile` not dispatched within this leg's bound. |

### RC-REPOSITORY fmt drift at `e12b9e8e06` (23 files)

`cargo fmt --all -- --check` reports drift in:
`external-roots` interrupt-table member admissions + tests + stack_demand,
`compilation-report` terminal_product, `compiler` tests (task_runtime,
interrupt_descriptor_tables, module_machine_indices*, lexical_aggregate_values),
`terminal-psi-to-abstract-operations` structural_scalar_fields,
`calling-conventions` lib.rs, `checked-trees-to-lowered-psi` tests,
`typed-trees-to-checked-trees` borrowed_windows + linear_obligations +
termination progress origins* + structural_scalar_store tests +
token_bound_machine_calls tests. All drift is in other lanes' in-flight
surfaces; none of it is in this leg's changed files.

### RC-DIAGNOSTICS drifted canaries at `e12b9e8e06`

- `generics/const_data_machine_call_requires_zero_arguments` — diagnostic
  wording drift ("constant call argument count differs from its exact entry").
- `generics/const_data_machine_call_requires_pure` — diagnostic wording drift
  (admission enumeration appended).
- `domains/boundary_operator_mutation_invalidates_domain` — wording drift
  (borrow-access diagnostic replaced the domain contract fragment).
- `providers/slot_plan_ambiguous` — fails earlier at entry-root binding
  ("no bound required root slot `linux_x86_64::ProgramEntry`"), masking the
  pinned ambiguity fragment.
- `ownership/linear_ambiguous_state_result_mapping` — **silent acceptance**:
  compiled to checked semantics where a rejection is pinned.
- `calls/guarded_value_call_terminal_rejected` — **silent acceptance**:
  compiled cleanly where a rejection is pinned.

(Plus the same wording-drift family recorded under
`wiki/drafts/rc_diagnostics_linux_x86_64.md`; this run's log is the
148.6s canary execution at this base.)

## Platform runners

| Runner | Status |
| --- | --- |
| linux_x86_64 | partially recorded (this document) |
| linux_arm64 | open — no linux/arm64 host on this worker |
| macos_arm64 | open — no macOS host on this worker |
| windows_x86_64 | open — no Windows host on this worker |

## Closure

**open.** Open rows: RC-REPOSITORY (fmt drift + unrun commands),
RC-DIAGNOSTICS (drifted/red), RC-PCC-REPLAY and RC-BUILD-AND-PACKAGES
(dispatched, no verdict at cutoff), RC-SOURCE-SEMANTICS, RC-NATIVE-MATRIX,
RC-REPRESENTATIVE-PROGRAMS (not run at this base), and three of the four
required platform runs are structurally host-gated on this worker. The
substrate's `check` would independently classify this record as open.
