# Release record — linux_x86_64 at `75650d2e94`

Bounded release-matrix record produced under board item
RUST-COMPILER-RELEASE-RECORD (sibling legs: RC-RELEASE-RECORD,
RC-RELEASE-RECORD-RUN, RUST-RELEASE-RECORD, RC-RELEASE-CLOSURE-RUN,
RC-RELEASE-RECORD-AND-CLOSURE, substrate RC-RELEASE-RECORD-SUBSTRATE). The
record substrate is `tools/release/release_record.py` (schema
`omega-release-record/1`); its `records/` output directory is fenced to the
runner lanes this wave (RC-RELEASE-RECORD holds `records` +
`release_record_ea025447fe.md` at ~20:53Z), so this leg records observed
gate state as a draft document rather than writing a committed JSON row —
same disposition as the prior `rust_` records.

- Host: linux x86-64 (`platform.machine()=x86_64`), Python 3.10.12
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`
- Runner: `cargo` (mbx unavailable on this host — same resolution the
  substrate's `runner()` performs)
- Commit: `75650d2e94` (tip at record time, 2026-09-21 ~13:0xZ)

## Gate results (linux_x86_64)

| Gate | Status | Evidence |
| --- | --- | --- |
| RC-REPOSITORY | **fail** | `cargo fmt --all -- --check` exit 1 — the format leg moved red since the `832c55e69b` record: two drifted files, `typed-trees-to-checked-trees/src/checks/ranges/indexes/validation.rs` (2 hunks, introduced by `9df5b14892`) and `typed-trees-to-checked-trees/src/facts/crash_entry_values/mutable.rs` (1 hunk, introduced by `0dae28ea1a`; that file sits under the live NEW-CC-ENTRY-PROVENANCE-PRISTINE-WHOLE-STORAGE fence). Remaining gate commands (clippy workspace, `omega-architecture-test`, `check --workspace`, `nextest --workspace --lib`) not run within this leg's bound. |
| RC-PORTABLE-PSI | **pass** | `cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` — 1/1 PASS in 25.4s. |
| RC-DIAGNOSTICS | **pass** | `cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'` — 1/1 PASS in 101.6s. Selector caveat below. |
| RC-SOURCE-SEMANTICS | not run | `nextest -p compiler --all-targets` exceeds this leg's bound. |
| RC-PCC-REPLAY | not run | Not dispatched within this leg's bound — open row, not a pass. |
| RC-BUILD-AND-PACKAGES | not run | Not dispatched within this leg's bound — open row, not a pass. |
| RC-NATIVE-MATRIX | not run | Per required host; linux_x86_64 leg not dispatched within this leg's bound (the committed `30ed858582` JSON already records this lane's hosted_receiver observation). |
| RC-REPRESENTATIVE-PROGRAMS | not run | Per required host; `samples_compile` not dispatched within this leg's bound. |

## Findings for the substrate owner

- **RC-DIAGNOSTICS manifest selector drifted.** The gate manifest in
  `tools/release/release_record.py` selects
  `test(=proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment)`,
  which matches zero tests at this revision (exit 4, "no tests to run" —
  1458 skipped). The test lives at
  `proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`;
  this record ran the corrected selector. A substrate `run` today would
  record RC-DIAGNOSTICS red on an empty selection, not on the corpus —
  the manifest line needs updating alongside the next records write
  (fenced to the release-record lanes, not repaired here).
- **Committed record validates.** `python3 tools/release/release_record.py
  check tools/release/records/linux_x86_64__30ed858582__20260921T000306Z.json`
  → `valid (open)` — the first committed `omega-release-record/1` row
  (RC-PORTABLE-PSI pass + hosted_receiver native-execution observation)
  re-validates against the contract, and closure stays correctly open.

## Delta vs the folded `832c55e69b` record

One gate moved: RC-REPOSITORY's format leg went green → red on the two
files named above. RC-PORTABLE-PSI and RC-DIAGNOSTICS stay green (the
`machine_self_call_recursion_rejected` refusal restored at `b5e4c7c5f8`
holds for a third consecutive record). All other gates unchanged — open
rows, not run within this bound.

## Reading this record

Statuses are observed outcomes at the named commit on one host, not a
release verdict. `not run` rows stay open per the completion matrix; only
a stored `records/` JSON with `check` passing closes a runner row, and the
`records/` surface belongs to the substrate item. This leg's scope was
bounded to the format gate plus the two scoped canary filters the prior
records established as the runnable subset.
