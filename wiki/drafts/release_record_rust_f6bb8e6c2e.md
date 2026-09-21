# Release record — linux_x86_64 at `f6bb8e6c2e`

Bounded release-matrix record produced under board item RUST-RELEASE-RECORD
(sibling legs: RC-RELEASE-RECORD, RC-RELEASE-RECORD-RUN,
RUST-COMPILER-RELEASE-RECORD, RC-RELEASE-CLOSURE-RUN,
RC-RELEASE-RECORD-AND-CLOSURE, substrate RC-RELEASE-RECORD-SUBSTRATE). The
record substrate is `tools/release/release_record.py` (schema
`omega-release-record/1`); its `records/` output directory is fenced to the
substrate claim, so this leg records observed gate state as a draft document
rather than writing a committed JSON row.

- Host: linux x86-64 (`platform.machine()=x86_64`), Python 3.10.12
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`
- Runner: `cargo` (mbx unavailable on this host — same resolution the
  substrate's `runner()` performs)
- Commit: `f6bb8e6c2eb` (omega: demote the competing
  lower_to_target_operations_and_native_callbacks entrance)

## Gate results (linux_x86_64)

| Gate | Status | Evidence |
| --- | --- | --- |
| RC-REPOSITORY | **partial pass** | `cargo fmt --all -- --check` exit 0 — the 23-file drift recorded at `e12b9e8e06` is cleared at this base. Remaining gate commands (clippy workspace, `omega-architecture-test`, `check --workspace`, `nextest --workspace --lib`) not run within this leg's bound. |
| RC-PORTABLE-PSI | **pass** | `cargo nextest run -p compiler --test canary_suite -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` — 1/1 PASS in 26.4s (producer/consumer envelope reload + refusal legs). |
| RC-DIAGNOSTICS | **fail** | `cargo nextest run -p compiler --test canary_suite -E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'` — FAIL in 107.1s; observed regression below. |
| RC-SOURCE-SEMANTICS | not run | `nextest -p compiler --all-targets` exceeds this leg's bound. |
| RC-PCC-REPLAY | not run | Not dispatched within this leg's bound — open row, not a pass. |
| RC-BUILD-AND-PACKAGES | not run | Not dispatched within this leg's bound — open row, not a pass. |
| RC-NATIVE-MATRIX | not run | Per required host; linux_x86_64 leg not dispatched within this leg's bound. |
| RC-REPRESENTATIVE-PROGRAMS | not run | Per required host; `samples_compile` not dispatched within this leg's bound. |

### RC-DIAGNOSTICS regression at `f6bb8e6c2e`

`fail_canaries_reject_with_expected_diagnostic_fragment` fails: the corpus
member `tests/omega/fail/calls/machine_self_call_recursion_rejected` **compiled
successfully where a rejection was expected** (compiled 10 source files from
`main.omg`; wrote_output=false). A silent acceptance of a must-reject corpus
case on the calls surface — a drifted diagnostic or lost machine self-call
recursion refusal. Compare the drifted-diagnostic list recorded at
`e12b9e8e06`; whether this failure is the same red state or a new one is for
the diagnostics item's owner.

## Reading this record

Statuses are observed outcomes at the named commit on one host, not a release
verdict. `not run` / `not completed` rows stay open per the completion matrix;
only a stored `records/` JSON with `check` passing closes a runner row, and
the `records/` surface belongs to the substrate item. This leg's scope was
bounded to the format gate plus the two scoped canary filters the prior
record (`release_record_e12b9e8e06.md`) established as the runnable subset.
