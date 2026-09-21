# RC gate stability repair — z96 partial leg

Repaired the unfenced slice of the `RC-GATE-STABILITY-REPAIR` warning drift on
linux x86-64, against base `3438b0432126` (main tip `3a505ad6ff5e` era).

## Repairs landed in this leg

- `compiler/tests/canary_suite/time_hosts_and_indexed_storage.rs` — the
  `compile_rooted_canary_for_target` import is now
  `#[cfg(any(windows, target_os = "macos"))]`-gated; its only call sites are the
  Windows test at line ~592 and the macOS test at line ~618.
- `compiler/tests/fixture_rosters/atomics_and_target_canaries.rs` — deleted
  `pub const SHARED_RECEIVER_PLAIN_FIELD_WRITE`; the roster references the
  literal in `canary_suite.rs` and the name survives only in a comment here.
- `tests/native-differential/tests/terminal_psi_conditional.rs` — deleted the
  unused `#[cfg(unix)]` tail block: `NEXT_SCRATCH_DIRECTORY`,
  `ScratchDirectory`, its `Drop` impl, and the `PathBuf`/`AtomicU64` imports
  that only existed for it.

## Scoped evidence (linux x86-64)

- `cargo check -p compiler --tests` — clean, 0 warnings.
- `cargo check -p omega-native-differential-test --tests` — clean, 0 warnings.
- `cargo fmt --all -- --check` — my files clean; residual diffs are fenced
  (see below).
- `cargo clippy -p compiler --tests` / `-p omega-native-differential-test
  --tests` — warnings present only in preexisting dependency-lib files
  (`typed-trees-to-checked-trees`, `build-time-evaluation`, etc.); none in the
  touched files.

## Residual warnings — fenced elsewhere, not repaired here

`cargo test --workspace --no-run` drift sites that remain (fence holder noted):

- `omega-rust/omega/packages/manager/tests/package_capability_conflicts.rs`
  (unused self-import) — BUILD-PACKAGES-GATE / companion-fixtures-and-tests,
  Zergling-129, expires 2026-09-21T07:44:05Z.
- `omega-rust/omega/packages/review/evidence/tests/{authority,boundary_supply,
  conformance,contract_expressions,exact_contract_identity,operational,
  operators,proposition_contracts,trait_contracts}.rs` — `use crate::support;`
  with zero `support::` uses — same BUILD-PACKAGES-GATE fence.
- fmt residuals at tip: `prepare_expression.rs` (CRASH-CONTRACT +
  GENERAL-CYCLIC-EXECUTION), `values/evaluation.rs` (CRASH-CONTRACT),
  `checked-interpreter/tests/trait_operators.rs` (NAMED-TRAIT-OPERATORS).

A full-gate re-measure (fmt/clippy/architecture/check/workspace-lib) belongs to
the RC-REPOSITORY sweep once the fences release.
