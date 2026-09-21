# Release record — linux_x86_64 at `5958706064`

Bounded release-matrix record produced under board item RC-RELEASE-CLOSURE-RUN
(sibling legs: RC-RELEASE-RECORD, RC-RELEASE-RECORD-RUN,
RUST-COMPILER-RELEASE-RECORD, RC-RELEASE-RECORD-AND-CLOSURE, and the substrate
RC-RELEASE-RECORD-SUBSTRATE). The record substrate is
`tools/release/release_record.py` (schema `omega-release-record/1`); its
`records/` output directory is fenced to RC-RELEASE-RECORD, so this leg records
the observed gate state as a draft document instead of writing a committed JSON
row — the same convention `release_record_e12b9e8e06.md` used.

- Host: linux x86-64 (`platform.machine()=x86_64`), Python 3.10.12
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`
- Runner: `cargo` (mbx unavailable on this host — same resolution the
  substrate's `runner()` performs)
- Commit: `5958706064e5fb9a63a9ef58b58382015a945651`

## Gate results (linux_x86_64)

| Gate | Status | Evidence |
| --- | --- | --- |
| RC-REPOSITORY | partial — fmt leg **pass** | `cargo fmt --all -- --check` exit 0: zero drifted files at this base (14 drifted at a9fa1a4fe6, 23 at e12b9e8e06 — the fmt drift families landed cleanup). Remaining gate commands (`clippy --workspace --all-targets -D warnings`, `omega-architecture-test --all-targets`, `check --workspace --all-targets`, `nextest --workspace --lib`) not run within this leg's bound. |
| RC-PORTABLE-PSI | **pass** | `cargo nextest run -p compiler --test canary_suite -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` — 1/1 PASS in 25.6s (producer/consumer envelope reload + refusal legs). Unchanged from e12b9e8e06. |
| RC-DIAGNOSTICS | **fail** | `cargo nextest run -p compiler --test canary_suite -E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'` — FAIL in 128.1s; 15 drifted fail canaries (12 fragment-text drift + 3 silent-acceptance regressions, listed below). |
| RC-SOURCE-SEMANTICS | not run | `nextest -p compiler --all-targets` exceeds this leg's bound; RC-DIAGNOSTICS row reports the same canary suite's state. |
| RC-PCC-REPLAY | not run | Not dispatched within this leg's bound — open row, not a pass. |
| RC-BUILD-AND-PACKAGES | not run | Not dispatched within this leg's bound — open row, not a pass. |
| RC-NATIVE-MATRIX | harness compiles; suite not run | `cargo check -p omega-native-differential-test --all-targets` exit 0 in 28.9s — the harness target builds at this base (earlier bases recorded it red on `abstract_publication`/`pipeline_ownership` fixture drift). Gate suite not dispatched within this leg's bound. |
| RC-REPRESENTATIVE-PROGRAMS | not run | `samples_compile` not dispatched within this leg's bound — open row, not a pass. |

## RC-DIAGNOSTICS drift detail

Thirteen fragment-text drifts (expected-fragment text drifted, rejection still
produced): `expressions/indexed_qualified_call_argument_mismatch`,
`providers/provider_selection_outside_build`,
`build/program_entry_binding_outside_build`,
`proofs/proof_bignum_constant_false`, `generics/colon_bound_rejected`,
`generics/const_data_machine_call_requires_zero_arguments`,
`proofs/quotient_representative_admitted_closure_rejected`,
`proofs/quotient_theorem_admitted_closure_rejected`,
`proofs/quotient_theorem_boundary_rejected`,
`proofs/constant_equation_refuted`,
`generics/const_data_machine_call_requires_pure`,
`providers/slot_plan_ambiguous`.

Three silent-acceptance regressions (compiled when the corpus expects a
rejection — the gate's real regressions): `ownership/linear_ambiguous_state_result_mapping`,
`calls/guarded_value_call_terminal_rejected`,
`calls/machine_self_call_recursion_rejected`.

## Closure assessment

Closure stays **open**: the contract requires all eight gates green on one
clean commit plus recorded runs on all four required hosts
(linux_x86_64, linux_arm64/QEMU-named, macos_arm64, windows_x86_64). This host
observes one green gate, one red gate, one partial, and the rest undispatched;
three required hosts are unavailable to this worker.
