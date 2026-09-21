# Release record: 0cc7a2081a

Release-matrix run of `wiki/drafts/rust_compiler_completion.md` against the
Rust reference compiler at commit
`0cc7a2081a6e87e11a417c4c81efe7476c63412a` (2026-09-20). This document records
what the gates did; it does not claim they pass. No code was changed for this
record.

## Run identity

- Commit: `0cc7a2081a6e87e11a417c4c81efe7476c63412a`
- Toolchain: `nightly-2026-09-04` (`rust-toolchain.toml`),
  rustc 1.100.0-nightly (a69a63265 2026-09-03),
  cargo 1.100.0-nightly (b2e9d5f9d 2026-09-02),
  cargo-nextest 0.9.x (via `cargo`, `mbx` unavailable on this host)
- Host: Linux 6.8.0-1061-aws x86_64, Ubuntu 22.04.5, 8 cores
- Checkout: isolated worktree at the recorded commit; no local modifications
- Window: 2026-09-20T14:45Z – 2026-09-21T03:13Z (see per-gate elapsed)

`mbx` is not installed on this runner; per AGENTS.md and the contract, Cargo
was substituted verbatim (`cargo fmt`, `cargo check`, `cargo clippy`,
`cargo nextest run`, `cargo test --doc`). No canary/sample filters were set;
all runs used `--no-fail-fast`.

## Gate results (Linux x86-64 row)

| Gate | Command | Exit | Result | Elapsed |
| --- | --- | --- | --- | --- |
| RC-REPOSITORY / fmt | `cargo fmt --all -- --check` | 1 | **FAIL** — formatting drift in 16 files | 27s |
| RC-REPOSITORY / check | `cargo check --workspace --all-targets` | 101 | **FAIL** — `omega-native-differential-test` test `abstract_publication` does not compile: E0277 `[Optimization; 6]` vs `[Optimization; 7]` | 45s |
| RC-REPOSITORY / clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 101 | **FAIL** — `clippy::permissions_set_readonly_false` in `package-source` lib test (`set_readonly(false)`) | 3s |
| RC-REPOSITORY / architecture | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | 100 | **FAIL** — 547 run, 535 pass, 12 fail | 53s |
| RC-REPOSITORY / lib | `cargo nextest run --workspace --lib --no-fail-fast` | 100 | **FAIL** — 15631 run, 15536 pass, 95 fail, 2 skipped | 1155s |
| RC-SOURCE-SEMANTICS | `cargo nextest run -p compiler --all-targets --no-fail-fast` | 100 | **FAIL** — 3057 run, 1836 pass, 1221 fail | 24814s |
| RC-PCC-REPLAY / nextest | `cargo nextest run -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations --no-fail-fast` | 100 | **FAIL** — 3723 run, 3650 pass, 73 fail | 2118s |
| RC-PCC-REPLAY / doctest | `cargo test --doc -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations` | 0 | **PASS** — 1 doctest | 1s |
| RC-PORTABLE-PSI | `cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` | 0 | **PASS** — 1/1 | 25s |
| RC-BUILD-AND-PACKAGES / nextest | `cargo nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager --no-fail-fast` | 100 | **FAIL** — 1633 run, 1526 pass, 107 fail, 2 skipped | 4818s |
| RC-BUILD-AND-PACKAGES / doctest | `cargo test --doc -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager` | 0 | **PASS** — 2 doctests | 1s |
| RC-BUILD-AND-PACKAGES / compiler legs | `cargo nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast` | 100 | **FAIL** — 350 run, 316 pass, 34 fail (build_target_activation 18, package_compilation_inputs 13, build_config_granted 3) | 1455s |
| RC-NATIVE-MATRIX (linux_x86_64) | `cargo nextest run -p omega-native-differential-test --all-targets --no-fail-fast` | 101 | **FAIL** — test target `pipeline_ownership` does not compile: E0004 non-exhaustive match, `LegalizedScalarTerminator::Crash` not covered | 13s |
| RC-DIAGNOSTICS (as written) | `cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment)'` | 4 | **INEFFECTIVE** — selector matched 0 tests (1425 skipped); "no tests to run" | 1s |
| RC-DIAGNOSTICS (intended test) | same with `-E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'` | 100 | **FAIL** — 1/1 failed in 185s (fail-canary fragment drift, see below) | 185s |
| RC-REPRESENTATIVE-PROGRAMS | `cargo nextest run -p compiler --test samples_compile --no-fail-fast` | 100 | **FAIL** — 33 run, 9 pass, 24 fail | 11748s |

### Selector note (RC-DIAGNOSTICS)

The contract command's filter
`proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment`
matches no test at this commit; the test lives one module deeper at
`proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`.
Both the verbatim result (0 tests, exit 4) and the corrected-selector result
(ran, failed) are recorded above. The verbatim row leaves the gate open per
the "filtered or empty run" rule regardless of the corrected result.

## Failure detail

### RC-SOURCE-SEMANTICS (1221 failures)

~1036 failures share one signature: panics of the form "selected ProgramEntry
establishment rejoins 0 Terminal attachment identities; expected one",
concentrated in `canary_suite` (~1077 of the failures) and `samples_compile`
(~24). Remaining failures are spread across `calling_policy_plans`,
`build_target_activation`, `recast_views`, `package_compilation_inputs`,
`plan_laid_repeated_runtime`, `source_evaluated_native_realization`,
`subslice_runtime_end_bounds`, `service_operational_contracts`,
`layout_plans`, `composed_internal_unit_arguments`, `build_config_granted`,
`private_joint_progress`, `optimizer_opt_in`, plus 8 fail-canary
"missing expected fragment" mismatches and 2 fail-canary sources that
compiled successfully.

### RC-REPOSITORY detail

- fmt: drift in 16 files (external-roots interrupt_table ×3, stack_demand,
  compiler layout_plans + module_machine_indices ×2, calling-conventions lib,
  checked-trees-to-lowered-psi integer_policy_realization,
  typed-trees-to-checked-trees multiplicity/termination ×5,
  validation domain_weakening, validation match_dispatch).
- check: sole compile error is `omega-native-differential-test`
  `abstract_publication` E0277 (6-vs-7 `Optimization` array arity).
- clippy: sole lint error is `package-source` `permissions_set_readonly_false`.
- architecture (12): 11 `validation_integration` boundary-witness/recast
  cases + `glob_self_imports_never_grow_per_crate`.
- lib (95): largest clusters `selected-dispatch` boundary_dispatch (~32),
  `checked-trees-to-lowered-psi` (~30), `terminal-codec` block_wire (~10),
  `abstract-operations-to-abstract-operations` ranked_rewrites (~7),
  `package-manager` (~6), plus source-files-to-assembled-syntax,
  calling-conventions, typed-trees-to-checked-trees.

### RC-PCC-REPLAY (73 failures)

`checked-trees-to-lowered-psi` suite (~57), `terminal-codec`
`sections::semantic_module::block_wire` (~10), `terminal-interpreter` unit
(~3), `terminal-psi-to-abstract-operations` (~2), `terminal-verifier`
`trusted_surface::recorded_digests_match_the_working_tree` (1).

### RC-BUILD-AND-PACKAGES (107 failures, nextest leg)

`package-evidence` suite (~53) spanning representation_policy,
selected_provider_policy, source_custody, terminal_permission_policy,
trait_contracts; `package-manager` (~53) spanning review/operations/
resolution plus `semantic_binding_review::macos_entry`.

### RC-REPRESENTATIVE-PROGRAMS (24 failures)

All 24 failures share the `selected ProgramEntry establishment rejoins 0
Terminal attachment identities; expected one` panic signature: the eight
`*_samples_compile_from_authored_program_entry_bindings` group tests,
`all_samples_reach_checked_trees`, `samples_with_documented_exit_run_correctly`,
`native_sample_console_acceptance_binds_the_exact_selected_target`, and
per-sample checks including `fletcher_checksum_checks_its_slice_iteration`,
`caesar_cipher_preserves_its_declared_text_carrier`,
`format_number_preserves_its_declared_text_carrier`,
`named_integer_conversion_samples_reach_checked_trees`,
`print_squares_preserves_its_declared_text_carrier`,
`temperature_sample_retains_exact_float_operator_evidence`,
`text_padding_accepts_its_projected_text_argument`, and
`unit_closure::cli_mvp_retains_checked_entry_and_console_call_closure`.

### RC-NATIVE-MATRIX

Compile failure, zero tests ran: `pipeline_ownership` match on
`&mut LegalizedScalarTerminator` lacks a `Crash` arm (E0004). The
linux_x86_64 leg therefore fails without executing; per the contract the gate
also requires RC-SOURCE-SEMANTICS on each host.

## Expected skips (exact list)

- `cargo nextest run --workspace --lib`: 2 skipped —
  `facts fact_plan::contexts::storage::measurements::prepared_entry_group_cost`
  and `facts fact_plan::contexts::storage::measurements::context_index_cost`
  (`#[ignore]` "manual … cost measurement").
- Package/build nextest leg: 2 skipped —
  `package-manager resolution::graph::resolve::tests::offline_git_transport_fixture_requires_unix_shell`
  (`#[ignore]` "offline Git transport counting requires the Unix test-only
  SSH transport") and
  `package-manager operations::prepare_project::tests::locked::offline::offline_git_preparation_keeps_exact_pins_and_never_calls_transport`
  (`#[ignore]` "offline Git transport counter requires a Unix shell").
- All other gates reported 0 skipped tests.

## Platform matrix

| Runner | Product identity | Status |
| --- | --- | --- |
| Linux x86-64 | `linux_x86_64` | Ran — gates above; row **open** (multiple failing gates) |
| Linux AArch64 | `linux_arm64` | **Runner unavailable** on this host — row open, not a pass |
| macOS AArch64 | `macos_arm64` | **Runner unavailable** on this host — row open, not a pass |
| Windows x86-64 | `windows_x86_64` | **Runner unavailable** on this host — row open, not a pass |

## Closure status

Not closed. Seven of eight gates fail or are ineffective on linux_x86_64 at
this commit: RC-REPOSITORY (all five baseline commands red), RC-SOURCE-SEMANTICS,
RC-PCC-REPLAY (nextest leg), RC-BUILD-AND-PACKAGES (nextest and compiler legs),
RC-NATIVE-MATRIX (compile failure, plus three unavailable hosts),
RC-DIAGNOSTICS-as-written (empty selector) with the intended test failing, and
RC-REPRESENTATIVE-PROGRAMS. RC-PORTABLE-PSI passes. The three non-Linux
platform rows remain open pending runner availability.
