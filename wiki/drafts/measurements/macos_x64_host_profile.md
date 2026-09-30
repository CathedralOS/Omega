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
  (`omega-rust/omega/compilation/backend/target/src/target_profile.rs`); the
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
   `omega-rust/omega/compilation/driver/build-evaluation/src/admission/selection.rs`.

Because checked compilation cannot produce a `macos_x86_64` package, no
Mach-O object, hosted receiver, import pairing, entry/exit or provider
behavior could be exercised on this base. Those legs are **justified
skips**, not passes.

## Attribution

- Gap 1 (boundary-row selection for `ConsoleNativeProvider::exit_process`)
  and gap 2 (`macos_x86_64::ProgramEntry` root-slot binding) both live in
  the provider-settlement / boundary-binding capability — the
  `omega-rust/omega/compilation/driver/build-evaluation` admission surface and the
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

## Rosetta host observations (2026-09-30)

Second leg, after the Intel Mach-O backend, hosted Darwin syscall leaves and
the `std/console.omg` provider selection landed on the MACOS-X64-HOST-PROFILE
branch (based on `f17a009cdd`).

- Host: macOS Darwin 24.5.0 on Apple silicon (arm64) with Rosetta 2.
  `arch -x86_64 <image>` runs the emitted x86-64 Mach-O image; Rosetta
  translates the instructions and the Darwin `syscall` entries. These
  observations exercise the emitted bytes, the Mach-O loader path and the
  kernel ABI, but they are translated execution, not an Intel CPU.
- Compiler: `target/debug/omega` (`cargo build -p omega`, dev profile).
- Standard-library probes were copies of the named fixture or sample with an
  absolute `Source::Path` to `source/library/std`, reviewed with
  `omega update --project <dir> --target macos_x86_64`, every pending
  decision edited to `accept`, then `omega update --resume --project <dir>`.
  Each image came from `omega --target macos_x86_64 --build-dir <out>
  <dir>/main.omg`; `omega run --target` compiles without running, so the
  image ran as `arch -x86_64 <out>/omega-program`.

| # | Program | Observation |
| --- | --- | --- |
| 1 | `tests/omega/pass/calls/state_and_machine_names_exit` (no dependency, free Unit entry) | `Mach-O 64-bit executable x86_64`; exit 0 |
| 1b | `samples/cli/basics/standalone` with the `macos_x86_64` binding | exit 0 through the free Unit entry adapter |
| 2 | `samples/cli/basics/cli_mvp` with the `macos_x86_64` binding | exit 0; stdout `Hello, Omega.` and `[press Enter to close]` (hosted receiver bridge, Console `write_line`, `read_line`, `exit_process`) |
| 3 | `tests/omega/pass/host/runtime_console_byte_echo_exit`, stdin `AB` | exit 201; stdout `AB`. Empty stdin: exit 70. Closed stdin: exit 132 (SIGILL from the leaf's `ud2`) |
| 4 | `tests/omega/pass/host/process_exit_i32_status_mapped` | exit 212 (`ProcessExit` status -44, low byte) |
| 5 | `tests/omega/pass/providers/external_boundary_requirement_via_exit` with `macos_x86_64` leaves | exit 21 through dyld-bound `libSystem` `_abs` and `_exit` imports |
| 6 | Corpus native leg, `OMEGA_CORPUS_NATIVE=1 OMEGA_CORPUS_NATIVE_TARGET=macos_x86_64`, filter `pass/host/`, `serviceful_borrowed_scalar_exit`, `runtime_bounded_carrier_normal_status_exit`, `state_and_machine_names_exit`, `runtime_static_entry_context_exit`, `external_boundary_requirement_via_exit` | Every row equals the `macos_arm64` native golden: `exit:0` twice, `exit:70` three times (two with `stdout:match`), `exit:20` once, `exit:21` once; `process_exit_i32_status_*` built; both GUI fixtures rejected for the missing `Gui` Fused provider, as on arm64 |
| 7 | `python3 tools/corpus_gate.py --native-target macos_x86_64` over four of those fixtures, scratch golden | 4 built rows recorded; the authored exit oracles 70 and 21 held |
| 8 | Parity experiment, not committed: every `tests/omega` `build.omg` binding `macos_arm64` temporarily also bound `macos_x86_64` (583 files), then the runner's `macos_x86_64` leg over the 556 pass/run fixtures among them that are not expected to trap, compared with the same tree's `macos_arm64` native leg | 345 executed with identical exit and stdout facts (3 heavy float fixtures needed a 500 s compile cap on rerun); 69 built on both without executing; 132 rejected on both (131 with the same first diagnostic); 3 timed out on both; 7 built only on arm64, each for authored target-scoped source (Fused providers written for `macos_arm64`, helpers declared only for `linux_arm64`, x86 FMA needing AVX/FMA3 evidence). No executed fixture diverged and none crashed |

Trap-expected fixtures were left out of row 8: a trapping process goes
through the crash reporter, which on this host held such processes in
uninterruptible exit and stalled the runner's workers.

A closed stdout traps in the write leaf on both ISAs; under this host's
crash reporter the trapping process then waits in uninterruptible exit for
minutes. The arm64 image behaves the same way, so this is host crash-report
latency, not an emitted-code difference.

## Remaining legs

- A native Intel host run: `python3 tools/corpus_gate.py --native` now
  resolves `macos_x86_64` on Intel macOS and needs a recorded
  `tests/omega/corpus_native_macos_x86_64.txt`. Rosetta runs use
  `--native-target macos_x86_64` on Apple silicon.
- 579 fixture and 146 sample `build.omg` files bind `macos_arm64` without
  `macos_x86_64`; each new binding needs an Intel or Rosetta observation
  before it counts.
- `omega inspect-terminal --machine <qualified> --target macos_x86_64`.
- Reconcile `tools/benchmark`'s `HOST_LEGS` "native realization pending"
  label against this record (BENCHMARKS, TASKS_OPTIMIZER.md).
