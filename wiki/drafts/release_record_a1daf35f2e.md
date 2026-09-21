# Release record — linux_x86_64 at `a1daf35f2e`

Bounded release-matrix record produced under board item
RC-RELEASE-RECORD-AND-CLOSURE (sibling legs: RC-RELEASE-RECORD,
RC-RELEASE-RECORD-RUN, RUST-COMPILER-RELEASE-RECORD,
RC-RELEASE-CLOSURE-RUN, and the substrate RC-RELEASE-RECORD-SUBSTRATE).
The substrate is `tools/release/release_record.py` (schema
`omega-release-record/1`); its `records/` output directory is fenced to the
substrate claim, so this leg records observed gate state as a draft document
and does not commit a JSON row. The run executed against a clean detached
worktree of `a1daf35f2e8a22e988add51deed76338f510dd58` — unlike the prior
record's shared dirty checkout, this measurement is on an unmodified tree.

- Host: linux x86-64 (`platform.machine()=x86_64`), Python 3.10.12
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`
- Runner: `cargo` (mbx unavailable — the substrate's own `runner()` fallback)
- Commit: `a1daf35f2e8a22e988add51deed76338f510dd58`
- Driver: `python3 tools/release/release_record.py run --target linux_x86_64`
  (the docstring's `--all` example is stale — the flag does not exist; the
  default already covers every gate)

## Gate results (linux_x86_64)

| Gate | Status | Evidence |
| --- | --- | --- |
| RC-REPOSITORY | **fail** | All five commands red: `cargo fmt --all -- --check` exit 1 in 26.5s (59 diff hunks across 24 files, list below); `clippy --workspace --all-targets -- -D warnings` exit 101 in 7.3s (first error `application_operations.rs:208` E0027 missing field `exit`; the same sweep then hits `omega-native-differential-test`: 4×E0308 `[Optimization; 6]` vs `[Optimization; 7]` at `abstract_publication/decision_custody.rs` and E0004 uncovered `LegalizedScalarTerminator::Crash` in `pipeline_ownership.rs`); `omega-architecture-test` exit 100 in 48.3s (560/573 — 13 failures: glob-self-import ratchet, 8 boundary-ensures/symbolic-walk recast-witness rejections, 2 stale provider-receiver fixtures, `exit_replay_checks_claimed_records`); `check --workspace --all-targets` exit 101 in 40.7s (same compile errors); `nextest --workspace --lib` exit 100 in 1261.4s (2 skipped). |
| RC-SOURCE-SEMANTICS | **fail** | `nextest -p compiler --all-targets` exit 100 in 25,208,701 ms (7h00m — dominates the record leg; includes the full canary corpus plus every compiler integration target; failure inventory is inside the substrate's captured output tail). |
| RC-PCC-REPLAY | not completed | First command (`nextest` across c2l + terminal-* crates) was still running at this record's cutoff (~01:55Z, started ~01:26Z after the 7h compiler sweep) — open row, not a pass. The earlier PCC publication witness stands separately: `pcc_publication` 22/22 green at `0977a4249e`. |
| RC-PORTABLE-PSI | not run | Not reached within the record leg's cutoff — open row, not a pass. Independent green witness exists from RC-PORTABLE-PSI-GATE at `72125c7156` (32.7s). |
| RC-BUILD-AND-PACKAGES | not run | Not reached within the cutoff — open row. |
| RC-NATIVE-MATRIX | not run | Not reached within the cutoff. The suite does not compile at this base (see RC-REPOSITORY clippy/check rows) — a standalone `--all-targets` leg would fail at compile, not at execution. |
| RC-DIAGNOSTICS | not run | Not reached within the cutoff. Prior red witness at `e12b9e8e06` (10 drifted canaries incl. 2 silent acceptances in 148.6s). |
| RC-REPRESENTATIVE-PROGRAMS | not run | Not reached within the cutoff. Note the leg is heavy: `samples_compile`'s `samples_with_documented_exit_run_correctly` alone ran ~2.5h inside RC-SOURCE-SEMANTICS' compiler sweep and would re-run here. |

### RC-REPOSITORY fmt drift at `a1daf35f2e` (24 files, 59 hunks)

`cargo fmt --all -- --check` reports drift in: `external-roots`
interrupt_table member admissions + tests + stack_demand;
`compilation-report` terminal_product; `compiler` tests
(task_runtime, interrupt_descriptor_tables, module_machine_indices*,
module_constants/lexical_aggregate_values);
`terminal-psi-to-abstract-operations` structural_scalar_fields;
`calling-conventions` lib.rs; `checked-trees-to-lowered-psi`
integer_policy_realization test; `typed-trees-to-checked-trees`
borrowed_windows + linear_obligations + termination progress origins* +
record_locals + structural_scalar_store tests +
token_bound_machine_calls test; `validation` operator application tests +
contract_entailment specification_calls + match_dispatch. All drift sits
in other lanes' in-flight surfaces.

## Platform runners

| Runner | Status |
| --- | --- |
| linux_x86_64 | partially recorded (this document) |
| linux_arm64 | open — no linux/arm64 host on this worker (qemu-aarch64 binfmt present; a named-emulator row remains a separate bounded leg) |
| macos_arm64 | open — no macOS host |
| windows_x86_64 | open — no Windows host |

## Closure

**open.** Open rows: RC-REPOSITORY (all five commands red),
RC-SOURCE-SEMANTICS (compiler sweep red, 7h), RC-PCC-REPLAY (in flight
at cutoff), RC-PORTABLE-PSI / RC-BUILD-AND-PACKAGES /
RC-NATIVE-MATRIX / RC-DIAGNOSTICS / RC-REPRESENTATIVE-PROGRAMS (not run
inside this leg's cutoff), and three of four required platform runs are
host-gated. The substrate's `check` would independently classify this
record as open.
