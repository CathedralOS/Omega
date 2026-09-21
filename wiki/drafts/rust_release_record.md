# Rust compiler release record

One release-evaluation pass against the matrix in
[rust_compiler_completion.md](rust_compiler_completion.md). The contract
closes only when all eight named gates pass from a clean checkout of the same
commit and all four required platform runs are recorded; this record reports
measured results, so a failed, timed-out, or unrun row stays open — it is not
a pass, and rows are not averaged.

## Run header

- Commit: `7c15747568` (origin/main `739e4e81e9` plus one local test-only
  commit, `PIPELINE-CRATE-SWEEP` — an architecture-test module with no effect
  on shipped compiler code)
- Pinned Rust toolchain: `nightly-2026-09-04` (`rustc 1.100.0-nightly
  (a69a63265 2026-09-03)`, from `rust-toolchain.toml`)
- Host OS and architecture: Linux x86-64 container (`x86_64`), kernel 6.x
- Recorded: 2026-09-20T21:07Z – 2026-09-21T02:36Z (UTC)
- Worktree caveat: the run executed in the shared coordination worktree,
  which carries unrelated formatting-only dirty files; no source edits were
  made for this record. Each leg ran under a wall-clock cap, so timed-out
  legs are reported as partial runs rather than passes. `mbx` is not
  installed on this host; `cargo`/`cargo nextest` equivalents were used.

## Gate results — linux_x86_64 host

| Gate | Invocation | Result | Elapsed |
| --- | --- | --- | --- |
| `RC-REPOSITORY` `cargo fmt --all -- --check` | as written | PASS (clean) | 27s |
| `RC-REPOSITORY` `cargo check --workspace --all-targets` | as written | FAIL — sibling test-target compile errors: `omega-native-differential-test` `abstract_publication` (`[Optimization; 6]` vs `[Optimization; 7]`), `pipeline_ownership` non-exhaustive `LegalizedScalarTerminator::Crash` | 15s |
| `RC-REPOSITORY` architecture suite | `nextest -p omega-architecture-test --all-targets --no-fail-fast` | FAIL — 539/551 passed, 12 failed | 3s |
| `RC-REPOSITORY` `cargo clippy --workspace --all-targets -- -D warnings` | as written | FAIL — `clippy::question-mark` at `validation/src/machine_calls/structural_call_custody.rs:151`; `syntax-trees-to-symbol-resolved-trees` lib-test compile error | 8s |
| `RC-REPOSITORY` workspace library tests | `nextest --workspace --lib --no-fail-fast` | FAIL — 15542/15637 passed, 95 failed, 2 skipped | 1147s |
| `RC-SOURCE-SEMANTICS` | `nextest -p compiler --all-targets --no-fail-fast` | PARTIAL (150m cap) — 1504/3059 run: 654 passed, 850 failed; 1555 not run | 9000s (cap) |
| `RC-PCC-REPLAY` | five-crate nextest selection + `cargo test --doc` for the same crates | FAIL — nextest 3674/3747 passed, 73 failed (1511s); doctests PASS (1 passed, rest 0-item) | 1511s + 2s |
| `RC-PORTABLE-PSI` | canary `portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary` | PASS — 1/1 | 25s |
| `RC-BUILD-AND-PACKAGES` | seven-crate nextest + doctests + six named compiler test targets | FAIL — nextest PARTIAL (60m cap): 1515/1633 run: 1417 passed, 98 failed, 2 skipped; doctests PASS (3 passed, rest 0-item); named targets 316/350 passed, 34 failed | 3600s (cap) + 1s + 794s |
| `RC-NATIVE-MATRIX` (linux_x86_64 row) | `nextest -p omega-native-differential-test --all-targets --no-fail-fast` | FAIL — could not compile sibling test target `abstract_publication` (`[Optimization; 6]` vs `[Optimization; 7]`); no tests ran | 6s |
| `RC-DIAGNOSTICS` | canary `fail_canaries_reject_with_expected_diagnostic_fragment` | FAIL — doc-stated filter path is stale and selects 0 tests; corrected path `proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment` ran 1 test in 184s and failed (fail-corpus fixture `guarded_value_call_terminal_rejected` compiled where rejection was expected) | 0s + 184s |
| `RC-REPRESENTATIVE-PROGRAMS` | `nextest -p compiler --test samples_compile --no-fail-fast` | PARTIAL (60m cap) — 28/33 run: 9 passed, 19 failed; 5 killed by signal | 3600s (cap) |

## Required platform runs

| Runner | Status |
| --- | --- |
| `linux_x86_64` | Evaluated — see gate rows above. |
| `linux_arm64` | Open — no AArch64 runner or emulator available on this host. |
| `macos_arm64` | Open — host unavailable. |
| `macos_x86_64` | Open — host unavailable. The entry-bridge emission for this target is exercised replay-side only: the hosted receiver now emits the exact System V x86-64 bridge (`prepare_macos_x86_64`, 36-byte shim returning through the saved loader continuation) instead of falling through to the AArch64 emitter; native dyld `appMain` execution stays unwitnessed on this linux x86-64 host. |
| `windows_x86_64` | Open — host unavailable. |

## Expected skips

- `RC-DIAGNOSTICS` under the exact contract filter runs zero tests (stale
  test-path fragment in the contract doc); recorded above under the corrected
  module path.
- `RC-PCC-REPLAY` nextest leg lost one leg's remainder when the documented
  `nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
  proof-search blowup (known baseline, ~1300s @ ~573% CPU) was terminated to
  release the cap; its failure is counted in the 73.
- Doctest legs report most crates with 0 doc-tests; only crates that have
  them contribute counts (`terminal_psi_to_abstract_operations`,
  `build_evaluation`, `resolver_execution`).
- Workspace `--lib` and `--all-targets` runs report nextest's `skipped` rows
  as counted above; all other skips derive from the runner caps noted
  per-row.

## Closure assessment

**Not closable.** Ten of twelve linux_x86_64 legs fail or are capped-partial;
three of four required platform runs are unhostable here. Per the contract,
this record is a status snapshot, not a green build: every failed, capped,
or unrun row stays open. The highest-signal open rows for future passes:

- `RC-SOURCE-SEMANTICS` is the largest open surface (850 observed failures
  in the first half of the suite, dominated by canary-corpus legs: native
  artifact identity / ProgramEntry attachment diagnostics, trapping
  conversions awaiting runtime policy realization, and `UnsupportedScalarOperation`
  legs such as `SaturatingIntegerMultiply i8`).
- `RC-REPOSITORY.check`, `RC-NATIVE-MATRIX`, and part of clippy fail on
  pre-existing sibling-target compile errors (catalog `[Optimization; 6]` vs
  `[Optimization; 7]` drift and the `LegalizedScalarTerminator::Crash`
  exhaustiveness fallout from `bf8769cce1`) — repair rows tracked elsewhere.
- `RC-REPRESENTATIVE-PROGRAMS` and `RC-BUILD-AND-PACKAGES` nextest legs hit
  their wall-clock caps; a release run needs either larger budgets or the
  documented slow-test remedies.
