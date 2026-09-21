# Release record — linux_x86_64 at `832c55e69b`

Bounded release-matrix record produced under board item
RUST-COMPILER-RELEASE-RECORD (sibling legs: RC-RELEASE-RECORD,
RC-RELEASE-RECORD-RUN, RUST-RELEASE-RECORD, RC-RELEASE-CLOSURE-RUN,
RC-RELEASE-RECORD-AND-CLOSURE, substrate RC-RELEASE-RECORD-SUBSTRATE). The
record substrate is `tools/release/release_record.py` (schema
`omega-release-record/1`); its `records/` output directory is fenced to the
runner lanes this wave, so this leg records observed gate state as a draft
document rather than writing a committed JSON row — same disposition as the
prior records at `f6bb8e6c2e` and `b5e4c7c5f8`.

- Host: linux x86-64 (`platform.machine()=x86_64`), Python 3.10.12
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`
- Runner: `cargo` (mbx unavailable on this host — same resolution the
  substrate's `runner()` performs)
- Commit: `832c55e69b76` (tip at record time)

## Gate results (linux_x86_64)

| Gate | Status | Evidence |
| --- | --- | --- |
| RC-REPOSITORY | **partial pass** | `cargo fmt --all -- --check` exit 0 — format gate green. Remaining gate commands (clippy workspace, `omega-architecture-test`, `check --workspace`, `nextest --workspace --lib`) not run within this leg's bound; the sibling gate ledger at `771d0469a1` records clippy + architecture reds attributed to fenced repair rows. |
| RC-PORTABLE-PSI | **pass** | `cargo nextest run -p compiler --test canary_suite -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` — 1/1 PASS in 29.0s. |
| RC-DIAGNOSTICS | **pass** | `cargo nextest run -p compiler --test canary_suite -E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'` — 1/1 PASS in 157.3s. The `machine_self_call_recursion_rejected` refusal restored at `b5e4c7c5f8` stays green. |
| RC-SOURCE-SEMANTICS | not run | `nextest -p compiler --all-targets` exceeds this leg's bound. |
| RC-PCC-REPLAY | not run | Not dispatched within this leg's bound — open row, not a pass. |
| RC-BUILD-AND-PACKAGES | not run | Not dispatched within this leg's bound — open row, not a pass. |
| RC-NATIVE-MATRIX | not run | Per required host; linux_x86_64 leg not dispatched within this leg's bound. |
| RC-REPRESENTATIVE-PROGRAMS | not run | Per required host; `samples_compile` not dispatched within this leg's bound. |

## Delta vs `release_record_rust_b5e4c7c5f8.md`

No gate moved: RC-REPOSITORY format leg, RC-PORTABLE-PSI and RC-DIAGNOSTICS
remain green; all other gates unchanged (open rows, not run within this
bound). The diagnostics regression the `f6bb8e6c2e` record caught stays
cleared for a second consecutive record.

## Reading this record

Statuses are observed outcomes at the named commit on one host, not a release
verdict. `not run` / `not completed` rows stay open per the completion matrix;
only a stored `records/` JSON with `check` passing closes a runner row, and
the `records/` surface belongs to the substrate item (fenced to the runner
lanes this wave). This leg's scope was bounded to the format gate plus the
two scoped canary filters the prior records established as the runnable
subset.
