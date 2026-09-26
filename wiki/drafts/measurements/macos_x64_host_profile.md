# macOS x86-64 host profile — Mach-O route exercise

Purpose: first recorded leg of **MACOS-X64-HOST-PROFILE** (TASKS.md) —
"exercise emitted Mach-O entry/exit, receiver and provider/import behavior,
recording exact commands, commit, observations and justified skips" on an
Intel macOS host. This document is the observation record; repairs belong
to the capability owners named under Attribution.

## Host and revision

- Host: macOS Darwin 24.6.0, x86_64, 16 CPUs, 16 GiB (Intel).
- Compiler binary: `target/debug/omega` built from checkout `b725fb771e`
  (`mbx build -p omega`, dev profile, 2026-09-23). Measurements ran in the
  main checkout; origin/main moved to `3d3acf2d5c` during the session.
- `TargetProfile::host()` resolves `macos_x86_64` on this host
  (`omega-rust/omega/representations/target/src/target_profile.rs`); the
  declared target name `macos_x86_64` is accepted by `--target`.

## Commands and observations

All commands from the repository root with the dev binary above.

| # | Command | Observation |
| --- | --- | --- |
| 1 | `./target/debug/omega --check samples/cli/basics/cli_mvp/main.omg` | Host resolved to `macos_x86_64`; **fail** in package `omega-language-std`: `machine ConsoleNativeProvider::exit_process has no implementation for the selected target -- target-scoped implementations exist for: linux_arm64, linux_x86_64, macos_arm64, windows_x86_64` |
| 2 | `./target/debug/omega --target macos_x86_64 --check samples/cli/basics/cli_mvp/main.omg` | Identical diagnostic to #1 — explicit target selection and host resolution agree |
| 3 | `./target/debug/omega --target linux_x86_64 --check samples/cli/basics/cli_mvp/main.omg` | `omega-language-std` **passed**; **fail** in package `cli-mvp`: `call to read_line has operational envelope neither suspension nor blocking but acknowledges block` |
| 4 | `./target/debug/omega --target linux_x86_64 --check samples/cli/basics/print_number/main.omg` | **fail** in package `print-number`: `cannot prove default-domain field requirement for return from Main::main ... [u8; N]::Utf8` |
| 5 | `./target/debug/omega --target linux_x86_64 --check samples/cli/basics/standalone/main.omg` | **pass** — "compiled 10 source file(s) ... wrote_output=false" |
| 6 | `./target/debug/omega --target linux_x86_64 --check samples/cli/basics/text_greeting/main.omg` | **fail** in package `text-greeting`: `selected ProgramEntry establishment rejoins 0 Terminal attachment identities; expected one` |
| 7 | `./target/debug/omega --target macos_x86_64 --check samples/cli/basics/standalone/main.omg` | **fail** in package `standalone`: `selected target macos_x86_64 has no bound required root slot macos_x86_64::ProgramEntry` |
| 8 | `./target/debug/omega run --target macos_x86_64 samples/cli/basics/standalone/main.omg` | Same diagnostic as #7 (`native compile FAILED: cannot compile fresh package review`) |
| 9 | `./target/debug/omega run samples/cli/basics/standalone/main.omg` | Same diagnostic as #7 — bare `run` resolves host to `macos_x86_64` and hits the same wall |

## Read of the observations

Host/target resolution for `macos_x86_64` works end to end: bare commands
resolve the host profile, explicit `--target macos_x86_64` is accepted, and
the toolchain is functional (the `linux_x86_64` control compiles
`standalone` cleanly on this machine).

Every `macos_x86_64` leg is blocked **before artifact emission**, inside
checked compilation / fresh package review, by two distinct admission gaps:

1. `omega-language-std` is rejected for `macos_x86_64` because the authored
   `macos_x86_64 boundary machine ConsoleNativeProvider::exit_process` in
   `source/library/std/targets/macos_x86_64/console_impl.omg` is not
   selected while the four other targets' rows are. The sibling console
   rows (`read_byte`, `write_byte`, `provider_defaults`) in the same file
   are selected — only the `exit_process` boundary row fails selection.
2. `standalone` (which checks clean under `linux_x86_64`) is rejected with
   `no bound required root slot macos_x86_64::ProgramEntry` — the target's
   program-entry root-slot binding does not establish, although
   `AcceptedSemanticBindingRole::MacosX64ProgramEntry` and the exact
   physical-contract digest exist in
   `omega-rust/omega/build/build-evaluation/src/admission/selection.rs`.

Because checked compilation cannot produce a `macos_x86_64` package, no
Mach-O object, hosted receiver, import pairing, entry/exit or provider
behavior could be exercised on this base. Those legs are **justified
skips**, not passes.

## Attribution

- Gap 1 (boundary-row selection for `ConsoleNativeProvider::exit_process`)
  and gap 2 (`macos_x86_64::ProgramEntry` root-slot binding) both live in
  the provider-settlement / boundary-binding capability — the
  `omega-rust/omega/build/build-evaluation` admission surface and the
  package binding it feeds. **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**
  (TASKS.md) owns that surface and was in flight in the same wave; this
  measurement makes no repair.
- The `linux_x86_64` sample failures (#3, #4, #6) are unrelated
  baseline/sample-level diagnostics observed identically under the working
  control target; they are recorded here for attribution only, not
  repaired in this slot.
- The local-swarm prompt block for Intel macOS ("`TargetProfile::host()`
  has no macos_x86_64 profile") is stale — the profile resolves; the
  blocker is admission, not host recognition.

## Remaining legs for the next session

- Re-run this table after the boundary-settlement/root-slot binding lands
  for `macos_x86_64`; a passing `--check` unlocks `run`, emitted-Mach-O
  entry/exit, hosted receiver and import-pairing observations.
- `omega inspect-terminal --machine <qualified> --target macos_x86_64`
  legs once a package checks.
- Reconcile `tools/benchmark`'s `HOST_LEGS` "native realization pending"
  label against this record (BENCHMARKS, TASKS_OPTIMIZER.md).
