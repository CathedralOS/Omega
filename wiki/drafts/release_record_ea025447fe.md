# Release record — linux_x86_64 at `ea025447fe`

Bounded release-matrix record produced under board item RC-RELEASE-RECORD
(sibling legs: RC-RELEASE-RECORD-RUN, RUST-COMPILER-RELEASE-RECORD,
RC-RELEASE-CLOSURE-RUN, RC-RELEASE-RECORD-AND-CLOSURE, and the substrate
RC-RELEASE-RECORD-SUBSTRATE). The record substrate is
`tools/release/release_record.py` (schema `omega-release-record/1`). This leg's
claim owns `records/`, so the JSON row is committed at
`tools/release/records/linux_x86_64__ea025447fe__20260921T130518Z.json` — the
current substrate (which requires a passing `--native-execution` observation for
a `recorded` lane) was driven against a detached worktree at `ea025447fe`; the
substrate at that commit predates the observation requirement.

- Host: linux x86-64 (`platform.machine()=x86_64`), Python 3.10.12
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`
- Runner: `cargo` (mbx unavailable on this host — same resolution the
  substrate's `runner()` performs)
- Commit: `ea025447fed493ac67bcec7bfefc18fdabdf6e84`
- Recorded: 2026-09-21T13:05:18Z
- Revision note: the wave assigned the `ea025447fe` record; `origin/main` had
  advanced to `75650d2e94` (~800 first-parent commits) by record time — this
  evidence describes the named revision, not current main.

## Gate results (linux_x86_64)

| Gate | Status | Evidence |
| --- | --- | --- |
| RC-PORTABLE-PSI | **pass** | `mbx nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` exit 0 in 81.3s — producer/consumer envelope reload plus truncated/mutated/trailing-byte refusal legs. |
| RC-REPOSITORY | not run | Not dispatched within this bounded leg. |
| RC-SOURCE-SEMANTICS | not run | Not dispatched within this bounded leg. |
| RC-PCC-REPLAY | not run | Not dispatched within this bounded leg. |
| RC-BUILD-AND-PACKAGES | not run | Not dispatched within this bounded leg. |
| RC-NATIVE-MATRIX | not run | Per required host; not dispatched within this bounded leg. |
| RC-DIAGNOSTICS | not measurable at this base | The contract's exact-match selector `proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment` does not match this revision's module path (`proof_and_float_suites::proof_and_domain_canaries::…`); a recorded `fail` would measure selector drift, not diagnostic behavior — left open rather than mis-recorded. |
| RC-REPRESENTATIVE-PROGRAMS | not run | Per required host; not dispatched within this bounded leg. |

## Platform runners

| Runner | Status |
| --- | --- |
| linux_x86_64 | **recorded** — `--native-execution "cargo nextest run -p omega-native-differential-test --test hosted_receiver -E test(~hosted_receiver)"` exit 0 in 39.1s: `hosted_receiver_bridge_binds_emits_and_replays_on_all_hosted_targets` emits all four hosted containers and runs the Linux x86-64 ELF as a real process (exit 0); `hosted_receiver_binding_fails_closed_on_substituted_custody` passes. |
| linux_arm64 | open — no linux/arm64 host on this worker |
| macos_arm64 | open — no macOS host on this worker |
| windows_x86_64 | open — no Windows host on this worker |

## Closure

**open.** Open rows: RC-REPOSITORY, RC-SOURCE-SEMANTICS, RC-PCC-REPLAY,
RC-BUILD-AND-PACKAGES, RC-NATIVE-MATRIX, RC-DIAGNOSTICS,
RC-REPRESENTATIVE-PROGRAMS (not measured in this bounded leg) and three of the
four required platform runs are structurally host-gated on this worker.
`python3 tools/release/release_record.py check` validates the committed JSON
(exit 0, `closure: open`).
