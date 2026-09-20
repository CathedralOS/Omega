# RC release-closure run — linux_x86_64

Release record for the closure run defined in
[rust_compiler_completion.md](rust_compiler_completion.md): the eight named
gates executed from a clean checkout of one commit on a single host.

- Commit: `a9fa1a4fe63b2af2fc4e42a430c8facd8b8072b2` (origin/main at run start)
- Pinned toolchain: `nightly-2026-09-04` (rustc 1.100.0-nightly a69a63265 2026-09-03,
  cargo 1.100.0-nightly b2e9d5f9d 2026-09-02), `rust-toolchain.toml` profile
  `minimal` with clippy and rustfmt
- Host: Linux 6.8.0-1061-aws x86_64 (Ubuntu 22.04 userland), 8 cores, 31 GiB
- Cargo substitution: `mbx` is unavailable on this host; `cargo` ran in its
  place per the completion draft's allowance
- Clean checkout: `git worktree` at the recorded commit, no tracked or
  untracked modifications at run start

## Gate results

| Gate | Invocation | Result | Elapsed |
| --- | --- | --- | --- |
| `RC-REPOSITORY` | `cargo fmt --all -- --check` | FAIL — 47 unformatted hunks on the clean checkout | 28s |
| `RC-REPOSITORY` | `cargo clippy --workspace --all-targets -- -D warnings` | FAIL — `clippy::permissions-set-readonly-false` at `omega/packages/sources/acquisition/src/tree/capture/traversal.rs:513` | 7s |
| `RC-REPOSITORY` | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | FAIL — 12/547 fail; all 12 are the recorded baseline family (11 `validation_integration` boundary-ensures/symbolic-walk recast-witness rows + `glob_self_imports_never_grow_per_crate`), unchanged from the base revision | 44s |
| `RC-REPOSITORY` | `cargo check --workspace --all-targets` | FAIL — `omega-native-differential-test` test target `abstract_publication` does not compile: `decision_custody.rs:58` compares `[Optimization; 6]` with `[Optimization; 7]`; the Applied-evidence custody fixture is one `PSI_PASS_CATALOG` row behind the public pass catalog | 37s |
| `RC-REPOSITORY` | `cargo nextest run --workspace --lib --no-fail-fast` | FAIL — 190/15621 fail | 1138s |
| `RC-BUILD-AND-PACKAGES` | `cargo nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager --no-fail-fast` | FAIL — 214/1631 fail + `package-manager::suite semantic_binding_review::macos_entry::target_entry_dependency_discovery_requires_explicit_consumer_acceptance` wedged at 100% CPU for 53 min, SIGTERMed | 4811s |
| `RC-BUILD-AND-PACKAGES` | `cargo test --doc` (same 7 packages) | PASS | 1s |
| `RC-BUILD-AND-PACKAGES` | `cargo nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast` | FAIL — 68 failures across the six-test bundle (incl. the named `package_compilation_inputs` baseline families) | 752s |
| `RC-PCC-REPLAY` | `cargo nextest run -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations --no-fail-fast` | FAIL — 73/3723 fail + `checked-trees-to-lowered-psi::suite nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return` wedged at 583% CPU for 81 min, SIGTERMed | 942s |
| `RC-PCC-REPLAY` | `cargo test --doc` (same 5 packages) | PASS | 1s |
| `RC-PORTABLE-PSI` | `cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` | PASS | 33s |
| `RC-DIAGNOSTICS` | `cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment)'` | FAIL — the doc's literal filter matches 0 tests (stale module path); the current spelling `proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment` runs and FAILS: 10 fail canaries drifted — 2 (`fail/ownership/linear_ambiguous_state_result_mapping`, `fail/calls/guarded_value_call_terminal_rejected`) now compile successfully instead of rejecting, 8 produce diagnostics differing from `expected.txt` | 0s + 180s |
| `RC-SOURCE-SEMANTICS` | `cargo nextest run -p compiler --all-targets --no-fail-fast` | FAIL — bounded at the record deadline: 1351/3057 run (637 passed incl. 147 slow, 714 failed), 1706 not run before SIGTERM; the suite is dominated by `canary_suite` corpus members whose per-test native compiles are prohibitively slow on 8 cores — itself an open-row condition per the closure rule | 9644s |
| `RC-REPRESENTATIVE-PROGRAMS` | `cargo nextest run -p compiler --test samples_compile --no-fail-fast` | FAIL — 18/33 tests fail; 5 more bounded after >80–115 min at ~98% CPU each (`all_samples_reach_checked_trees`, `arithmetic_samples`, `samples_with_documented_exit_run_correctly`, `system_samples`, `text_samples` SIGTERMed — prohibitively slow or wedged, both are open-row conditions) | 7564s |
| `RC-NATIVE-MATRIX` | `cargo nextest run -p omega-native-differential-test --all-targets --no-fail-fast` (linux_x86_64 row) | FAIL — the test crate does not compile on this commit: `abstract_publication` hits the `[Optimization; 6] != [Optimization; 7]` custody-fixture drift and `pipeline_ownership` hits `E0004 non-exhaustive patterns: &mut LegalizedScalarTerminator::Crash { .. }` not covered; zero tests run | 17s |

## Required platform runs

Only the `linux_x86_64` row can run on this host. `linux_arm64`,
`macos_arm64`, and `windows_x86_64` require their matching hosts (or a named
emulator for arm64) and are not expected skips here — they remain unrun rows of
the matrix, not part of this record.

## Expected skips on this host

- `SKIP: source-produced linux_arm64 executable cannot run on linux_x86_64`
  (cross-target execution inside the native-differential suite — the skip is
  correct behavior, but the suite never reached it on this commit because the
  test crate does not compile)
- `linux_arm64`, `macos_arm64`, `windows_x86_64` platform rows: unrun — no
  matching host; they are absent rows, not skips claimed against this record

## Verdict

1 gate green (`RC-PORTABLE-PSI`), both doctest legs green. Every other row is
open on `a9fa1a4fe6` from this host — fmt drift (47 hunks), a `-D warnings`
clippy violation, two separately non-compiling test targets in
`omega-native-differential-test`, 190 workspace-lib failures, 214
package-gate failures, 73 PCC-lane failures, 10 drifted fail-canary
diagnostics including 2 accidental acceptances, 18+ failed
representative-program families, 714 `compiler --all-targets` failures before
the bounded cut, and 3 separately wedged tests SIGTERMed at 53/81/115 min.
The closure contract admits no partial percentage: `RC-RELEASE-CLOSURE-RUN`
stays open.
