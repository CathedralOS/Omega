# RC native matrix — required host runs

Umbrella record for the `RC-NATIVE-MATRIX` gate's runner table in
[rust_compiler_completion.md](rust_compiler_completion.md#required-platform-runs).
Recorded at revision `0977a4249e` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable). The matrix stays open until all four
runner rows are recorded green on one commit; per-row detail lives in the
sibling drafts linked below.

## Runner rows

| Runner | Product identity | Row record | Status |
| --- | --- | --- | --- |
| Linux x86-64 | `linux_x86_64` | [rc_native_matrix_linux_x86_64.md](rc_native_matrix_linux_x86_64.md) | **red** — witnessed; see fresh sweep below |
| Linux AArch64 | `linux_arm64` | [rc_native_matrix_linux_arm64.md](rc_native_matrix_linux_arm64.md) | **red/open** — cross-emit only on this host (11 pass / 75 fail across 86 legs at `e76d715c8e`); real execution legs need a linux/arm64 host |
| macOS AArch64 | `macos_arm64` | — none — | **open, unrecorded** — no runner has executed emitted Mach-O programs; no row draft exists yet |
| Windows x86-64 | `windows_x86_64` | [rc_native_matrix_windows_x86_64.md](rc_native_matrix_windows_x86_64.md) | **open** — recorded procedure for a Windows runner; no execution yet |

No `tools/release/records/` row exists for any target on this commit —
the substrate is in place but no runner record has been committed.

## Fresh linux_x86_64 sweep at `0977a4249e`

The gate's invocation is `mbx nextest run -p omega-native-differential-test
--all-targets --no-fail-fast`. On this revision the `--all-targets` build
fails: the `pipeline_ownership` test target does not compile on main
(`LegalizedScalarTerminator::Crash` uncovered match arm in
`fixtures/ordinary_graph_controls.rs` plus `Arc<ValidatedOptimizedTarget-
Operations>` custody drift — in-flight sibling API changes). The sweep
below runs each test target individually so the one unbuildable target
does not void the row.

Verdict: **red** — 618 pass / 108 fail across 726 executed legs, one
target unbuildable (`pipeline_ownership`), one leg host-skipped
(`source_custody_artifact`, 0 run / 1 skipped).

| test target | result |
|-------------|--------|
| `abstract_publication` | 56/56 |
| `build_time` | 4/4 |
| `coverage` | 12/77 |
| `frontend_drop_expectations` | 26/26 |
| `gui_headless` | 0/1 |
| `ieee_comparisons` | 3/3 (slow, ~393s) |
| `local_record_receivers` | 3/3 (slow, ~328s) |
| `optimizer_corpus` | 7/7 (very slow, ~904s — needs >600s per leg) |
| `owned_control_cycles` | 5/5 |
| `physical_child_replay` | 12/12 |
| `pipeline_ownership` | **does not compile on `0977a4249e`** |
| `primitive_locals` | 17/17 |
| `primitive_store_return` | 7/7 |
| `real_fs` | 0/10 |
| `recast_views` | 0/9 |
| `scalar_array_results` | 35/35 |
| `scalar_case_results` | 123/125 |
| `scalar_control_cycles` | 24/24 |
| `source_custody_artifact` | 0 run / 1 skipped (host-gated) |
| `terminal_byte_views` | 105/113 |
| `terminal_psi` | 3/3 |
| `terminal_psi_calls` | 3/4 |
| `terminal_psi_conditional` | 2/2 |
| `terminal_psi_debug_spans` | 1/1 |
| `terminal_psi_indexed_receivers` | 78/78 |
| `terminal_psi_record_returns` | 0/6 |
| `terminal_psi_runnable` | 1/5 |
| `terminal_psi_source` | 88/90 |
| `terminal_psi_source_payloadless_optimizer` | 3/3 |

Baseline gates executed for this row:

| leg | command | result |
|-----|---------|--------|
| Bootstrap topology/path hygiene | `sh tools/bootstrap/check-chain-hygiene.sh` | pass (exit 0) |

## Failure families (726-leg sweep)

All 108 failures reduce to these residuals; none are new language behavior,
they are fixture-migration and in-flight-landing residuals on main:

1. **Bare boundary-trait service spelling** (~79 legs, dominant):
   `field X on data Y names bare boundary trait Z in value position; the
   intrinsic Service<R> carrier is the only service value spelling` —
   fixtures in `coverage` (the filesystem/std-module and value-returning
   legs), `real_fs` (10), and `terminal_psi_runnable` (4) still spell
   `fs: FilesystemHost` / `console: Console` directly instead of the closed
   `Service<R>` carrier. Same family recorded as the Service<R> residual
   in the linux_x86_64 row's family 1.

2. **Missing vendored `omega_language_std/` package dirs** (10 legs):
   `recast_views` (9) and `gui_headless` (1) fixtures declare
   `builder.depend(Source::Path { location: "../../source/library/std" })`
   and the compiler resolves the package as `<fixture>/omega_language_std/`,
   which is not materialized on this checkout — fixture environment gap,
   not a compile rejection.

3. **Program-fingerprint drift — `ProofSubjectMismatch`** (7 legs):
   `terminal_psi_record_returns` (6) and `scalar_case_results` (1) —
   claimed vs reconstructed `TerminalPsiIdentity.program_fingerprint`
   diverge, consistent with the `0977a4249e` PlaceAliases analysis landing
   shifting program identity after the fixtures' pinned fingerprints.

4. **Fixed-fuel expectation drift** (8 legs): `terminal_byte_views`
   `natural_writer`/`mutable_writes` legs —
   `derive_fixed_entry_fuel` no longer returns the pinned
   `ControlCycle` verdict for these fixtures.

5. **Golden bytes / structural-custody residuals** (4 legs):
   `terminal_psi_calls` (1, terminal bytes drifted — reviewed-replacement
   pin stale), `terminal_psi_source` (2 — emitted program bytes grew a
   call+exit epilogue after `b972133cad`'s verified repair; sibling
   emission change), `scalar_case_results` (1 — `composed Unit scalar
   call requires structural call custody`).

6. **`pipeline_ownership` build failure** (target, uncounted legs):
   non-exhaustive `LegalizedScalarTerminator::Crash` match and
   `Arc<ValidatedOptimizedTargetOperations>` argument type drift —
   in-flight sibling edits to the legalized-operations representation.

## Re-run condition

Re-sweep when (a) the bare-boundary `Service<R>` fixture migrations land
(the dominant family above), (b) a linux/arm64 host and a macOS arm64 host
each run `python tools/release/release_record.py run --target <t> --all`
and commit their row records, and (c) `pipeline_ownership` compiles again
so the true `--all-targets` invocation can run unpartitioned.
