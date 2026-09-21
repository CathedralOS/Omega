# RC native matrix — required host runs

Umbrella record for the `RC-NATIVE-MATRIX` gate's runner table in
[rust_compiler_completion.md](rust_compiler_completion.md#required-platform-runs).
Recorded at revision `e5bbe53956f` (2026-09-20/21), host
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

## Fresh linux_x86_64 sweep at `e5bbe53956f`

The gate's invocation is `mbx nextest run -p omega-native-differential-test
--all-targets --no-fail-fast`. Unlike the `0977a4249e` recording, the
`--all-targets` build succeeds on this revision — `pipeline_ownership`
compiles again (the `LegalizedScalarTerminator::Crash` and `Arc` custody
drift landed) — so this sweep ran the true unpartitioned invocation.

Verdict: **red** — 1018 pass / 109 fail / 1 skipped across 1127 legs
(~1594s wall, 37 slow legs). The suite grew from 726 to 1127 legs since
`0977a4249e` (new `hosted_receiver` target; `pipeline_ownership` now
counts 392 legs; `terminal_psi_indexed_receivers` 79; `terminal_psi_source`
90; `recast_views` 10).

| test target | result |
|-------------|--------|
| `abstract_publication` | 56/56 |
| `build_time` | 4/4 |
| `coverage` | 12/77 |
| `frontend_drop_expectations` | 26/26 |
| `gui_headless` | 0/1 |
| `hosted_receiver` | 7/7 (new target) |
| `ieee_comparisons` | 3/3 (slow, ~241s worst leg) |
| `local_record_receivers` | 3/3 (slow, ~247s) |
| `optimizer_corpus` | 7/7 (very slow, ~276s worst leg) |
| `owned_control_cycles` | 5/5 |
| `physical_child_replay` | 12/12 |
| `pipeline_ownership` | 390/392 (compiles and runs again) |
| `primitive_locals` | 17/17 |
| `primitive_store_return` | 7/7 |
| `real_fs` | 0/10 |
| `recast_views` | 0/10 |
| `scalar_array_results` | 35/35 |
| `scalar_case_results` | 123/125 |
| `scalar_control_cycles` | 24/24 |
| `source_custody_artifact` | 0 run / 1 skipped (host-gated) |
| `terminal_byte_views` | 105/113 |
| `terminal_psi` | 3/3 |
| `terminal_psi_calls` | 3/4 |
| `terminal_psi_conditional` | 2/2 |
| `terminal_psi_debug_spans` | 1/1 |
| `terminal_psi_indexed_receivers` | 79/79 |
| `terminal_psi_record_returns` | 0/6 |
| `terminal_psi_runnable` | 1/5 |
| `terminal_psi_source` | 90/90 (was 88/90 — golden-byte drift closed) |
| `terminal_psi_source_payloadless_optimizer` | 3/3 |

Baseline gates executed for this row: none re-run — the row is a
measurement record, not a code change.

## Failure families (1127-leg sweep)

All 109 failures reduce to these residuals; none are new language behavior,
they are fixture-migration and in-flight-landing residuals on main:

1. **Bare boundary-trait service spelling** (~68 legs, dominant):
   `field X on data Y names bare boundary trait Z in value position; the
   intrinsic Service<R> carrier is the only service value spelling` —
   `coverage` (65 legs) and `terminal_psi_runnable` (3) still spell
   `console: Console` directly instead of the closed `Service<R>` carrier.

2. **Undeclared service-reach residual** (10 legs): `real_fs` — probes now
   clear the value-spelling gate but fail the reach check: "machine
   `Main::main` publishes service reach `<none>` but its checked body
   reaches undeclared service `Console`". The same migration family in
   its next form.

3. **Missing vendored `omega_language_std/` package dirs** (11 legs):
   `recast_views` (10) and `gui_headless` (1) fixtures declare
   `builder.depend(Source::Path { location: "../../source/library/std" })`
   and the compiler resolves the package as `<fixture>/omega_language_std/`,
   which is not materialized on this checkout — fixture environment gap,
   not a compile rejection.

4. **Program-fingerprint drift — `ProofSubjectMismatch`** (7 legs):
   `terminal_psi_record_returns` (6) and `scalar_case_results` (1,
   `joined_record_cannot_move_and_lend_its_child_to_the_same_call`) —
   claimed vs reconstructed `TerminalPsiIdentity.program_fingerprint`
   diverge.

5. **Fixed-fuel expectation drift** (8 legs): `terminal_byte_views`
   `natural_writer` legs — `derive_fixed_entry_fuel` no longer returns
   the pinned `ControlCycle` verdict for these fixtures.

6. **Golden bytes / structural-custody residuals** (2 legs):
   `terminal_psi_calls` (1, `scalar_i32_call_has_exact_exportable_terminal_`
   `bytes` — reviewed-replacement pin stale) and `scalar_case_results` (1,
   `owned_record_parameter_return_survives_an_observable_call` — `composed
   Unit scalar call requires structural call custody`). The `0977a4249e`
   `terminal_psi_source` golden-byte pair is closed.

7. **`pipeline_ownership` in-flight residuals** (2 legs, target now
   builds): `UnsupportedControlFlow(MachineId(3602))` in
   `structural_units::publication` and a pinned `UnsupportedVersion(15)`
   manifest codec in `structural_call` — sibling edits in flight on the
   newly-unblocked target.

8. **`terminal_psi_runnable` unit-closure plan residual** (1 leg):
   `InvalidUnitMachinePlan` — `Root::enter` attached Unit closure is
   missing a checked transitive machine plan (statement 0 call).

## Re-run condition

Re-sweep when (a) the bare-boundary `Service<R>` fixture migrations and
the `real_fs` service-reach declarations land (the dominant families
above), and (b) a linux/arm64 host and a macOS arm64 host each run
`python tools/release/release_record.py run --target <t> --all` and
commit their row records. The `--all-targets` gate invocation now runs
unpartitioned on this host.

## Prior sweep at `0977a4249e`

618 pass / 108 fail across 726 executed legs with `pipeline_ownership`
unbuildable (per-target partition). Failure families were the same shape:
Service<R> spelling dominated (~79), vendored-std dirs (10), fingerprint
drift (7), fixed-fuel pins (8), golden bytes/custody (4), plus the
pipeline_ownership build failure itself.
