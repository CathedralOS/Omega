# Release record — 12dea522b2 (linux_x86_64 row)

Contract: `wiki/drafts/rust_compiler_completion.md`. This row records the gate
invocations and observed results on a Linux x86-64 host. Rows are PASS or FAIL
or OPEN; a missing runner leaves a row open. No partial percentages, no
averaging between rows.

## Provenance

- Commit: `12dea522b2067ee5b6a900b1ad2a634d45cca3ec` (origin/main, clean
  checkout in a fresh worktree)
- Toolchain (pinned `rust-toolchain.toml`): `nightly-2026-09-04`
  - `rustc 1.100.0-nightly (a69a63265c 2026-09-03)`
  - `cargo 1.100.0-nightly (b2e9d5f9d 2026-09-02)`
  - `cargo-nextest 0.9.144 (9718c77af 2026-09-10)`
- Host: `Linux 6.8.0-1061-aws x86_64` (Ubuntu 22.04), 8 vCPU, 31 GiB RAM
- `mbx` is not installed on this host; per AGENTS.md the cargo fallbacks below
  are the same invocations the contract names, with `mbx` read as `cargo`.
  `cargo test --doc` runs the doctest legs (nextest does not run doctests).
- Run date: 2026-09-20 14:40Z .. 2026-09-21 ~03:30Z (UTC); gates run
  sequentially in one nextest harness on a warm shared target dir.

## Gate results — linux_x86_64 row

| Gate | Invocation | Result | Elapsed | Evidence |
|------|-----------|--------|---------|----------|
| RC-REPOSITORY / fmt | `cargo fmt --all -- --check` | FAIL | 27s | 47 files carry rustfmt diffs at this commit (external-roots interrupt_table, compiler layout_plans + module_machine_indices, typed-trees multiplicity checks, validation proof_contracts + value_custody, calling-conventions lib, C2L integer_policy_realization). |
| RC-REPOSITORY / clippy | `cargo clippy --workspace --all-targets -- -D warnings` | FAIL | 4s | `clippy::permissions_set_readonly_false` in `package-source` lib test (`writable.set_readonly(false)`); `-D warnings` rejects. |
| RC-REPOSITORY / arch | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | FAIL | 48s | 547 run / 535 passed / 12 failed: `glob_self_imports_never_grow_per_crate` and 11 `validation_integration` recast-witness/boundary cases. |
| RC-REPOSITORY / check | `cargo check --workspace --all-targets` | FAIL | 52s | E0277 in `omega-native-differential-test` test `abstract_publication`: `can't compare [Optimization; 6] with [Optimization; 7]` — fixture drifted from the Optimization inventory on main. |
| RC-REPOSITORY / lib tests | `cargo nextest run --workspace --lib --no-fail-fast` | FAIL | 1134s | 15627 run / 15532 passed / 95 failed / 2 skipped. By crate: selected-dispatch 32, checked-trees-to-lowered-psi 30, terminal-codec 10, package-manager 7 (semantic/review bindings), abstract-operations-to-abstract-operations 7 (loop_invariant_scalar_motion family), source-files-to-assembled-syntax 3, typed-trees-to-checked-trees 2, calling-conventions 2 (native_policies_and_syscalls), native-realization 1, abstract-operations-to-target-operations 1. |
| RC-SOURCE-SEMANTICS | `cargo nextest run -p compiler --all-targets --no-fail-fast` | FAIL | 25719s | 3057 run / 1835 passed / 1222 failed. Dominant signatures: `selected ProgramEntry establishment rejoins 0 Terminal attachment identities; expected one` (broad entry-binding regression), `UnsupportedScalarOperation` legalization family (WrappingIntegerDivide, ExactIntegerDivide, ExactIntegerRemainder, SaturatingIntegerMultiply, conversion runtime-policy realizations), Utf8 default-domain proof obligations, `requires a selected Fused provider` for Clock/Gui boundaries, `native-artifact production requires one exact selected program entry`. |
| RC-PCC-REPLAY / nextest | `cargo nextest run -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations --no-fail-fast` | FAIL | 1642s | 3723 run / 3649 passed / 74 failed; one proof-search hang in `checked-trees-to-lowered-psi::suite nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return` SIGTERM'd at 1501s after exceeding the earlier baseline's observed bound (~1352s). `terminal-verifier::suite trusted_surface::recorded_digests_match_the_working_tree` also fails (digests vs working tree). |
| RC-PCC-REPLAY / doctests | `cargo test --doc` over the same five crates | PASS | 1s | all doctests pass. |
| RC-PORTABLE-PSI | `cargo nextest run -p compiler --test canary_suite -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` | PASS | 70s | source-free Terminal Psi envelope re-verified and interpreted across the process boundary. |
| RC-BUILD-AND-PACKAGES / nextest | `cargo nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager --no-fail-fast` | FAIL | 6103s | 1631 run / 1523 passed / 108 failed; one package-manager test (`semantic_binding_review::macos_entry::target_entry_dependency_discovery_requires_explicit_consumer_acceptance`) ran unboundedly at ~100% CPU — SIGTERM'd at 3583s (proof/review-search hang signature). 230 tests flagged slow; several exceed 200-900s each. |
| RC-BUILD-AND-PACKAGES / doctests | `cargo test --doc` over the same seven crates | PASS | 1s | all doctests pass. |
| RC-BUILD-AND-PACKAGES / named | `cargo nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast` | FAIL | 977s | 350 run / 316 passed / 34 failed. |
| RC-NATIVE-MATRIX (this host) | `cargo nextest run -p omega-native-differential-test --all-targets --no-fail-fast` | FAIL | 34s | build blocked: E0277 `[Optimization; 6]` vs `[Optimization; 7]` (abstract_publication) plus E0004 non-exhaustive `LegalizedScalarTerminator::Crash` arm (pipeline_ownership). No test binary ran. |
| RC-DIAGNOSTICS | `cargo nextest run -p compiler --test canary_suite -E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'` | FAIL | 259s | ≥8 fail-corpus fixtures report a missing expected diagnostic fragment; `ownership/linear_ambiguous_state_result_mapping` compiled where rejection was expected. |
| RC-REPRESENTATIVE-PROGRAMS | `cargo nextest run -p compiler --test samples_compile --no-fail-fast` | FAIL | 11174s | 33 run / 9 passed / 24 failed. `samples_with_documented_exit_run_correctly` reports 124 samples failing to produce runnable artifacts ("selected ProgramEntry establishment rejoins 0 Terminal attachment identities", UnsupportedScalarOperation legalization refusals, Utf8 proofs, Fused-provider requirements, OperationProofUnavailable, trapping-conversion realization). |

## Platform runs

| Row | Result | Evidence |
|-----|--------|----------|
| linux_x86_64 | OPEN | Two direct attempts: `omega --target linux_x86_64 samples/cli/basics/print_number/main.omg` fails at checked semantics in 879s (`cannot prove default-domain field requirement for return from Main::main: self.out requires [u8; N]::Utf8`); `cli_mvp` exceeded a 900s budget without emitting. The `samples_with_documented_exit_run_correctly` sweep shows all 124 documented-exit samples failing to produce artifacts. No emitted ELF existed to execute; the row stays open. |
| linux_arm64 | OPEN | no runner on this host; no emulator installed. |
| macos_arm64 | OPEN | no runner. |
| windows_x86_64 | OPEN | no runner. |

## Expected-skip list

None claimed. Unexecuted rows above are OPEN, not policy-skipped.

## Determinism notes (evidence for the board's determinism-bounds item)

- Two distinct unbounded proof/review searches observed on this commit:
  `package-manager` `target_entry_dependency_discovery_requires_explicit_consumer_acceptance`
  (SIGTERM'd at 3583s) and `checked-trees-to-lowered-psi`
  `mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
  (SIGTERM'd at 1501s; matches the earlier ~1352s baseline hang).
- `samples_with_documented_exit_run_correctly` single test: 9257s (~2.6h).
  `arithmetic_samples_compile_from_authored_program_entry_bindings`: 9020s
  (~2.5h). Full gate elapsed >3h.
- `omega` CLI compile of a single CLI sample reached ~15min without emitting
  (`cli_mvp`, timed out at 900s).

## Board coordination

`TASKS.md` stayed claim-fenced for the whole run window, so this record is
published as its own artifact; the RC-RELEASE-RECORD-RUN board row text lands
when a TASKS.md window opens.
