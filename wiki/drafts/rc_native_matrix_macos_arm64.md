# RC native matrix — macos_arm64 row

Row status: **open** — no macOS AArch64 runner has executed the emitted
Mach-O programs. Recorded at revision
`beaa8e1c1be701d915336d90e84e4022242dbcba` (2026-09-21) from a Linux
x86-64 host; this file exists to name the open row, the exact procedure a
macOS AArch64 runner follows, and the cross-target coverage already
witnessed. Per `wiki/drafts/rust_compiler_completion.md`, cross-target
byte generation on a different host cannot replace matching-host
execution, and a missing runner leaves the row open; nothing below is a
pass claim.

## Required observation (per the contract runner table)

| Runner | Product identity | Required native observation |
| --- | --- | --- |
| macOS AArch64 | `macos_arm64` | Directly execute the emitted Mach-O AArch64 programs. |

Unlike the linux_arm64 row, this runner has no emulator escape hatch:
execution must be direct on a macOS AArch64 host.

## Runner procedure

From a clean checkout of the same commit on macOS AArch64, with the
pinned toolchain from `rust-toolchain.toml` (rustup selects it) and
Python 3:

```sh
python3 tools/release/release_record.py run --target macos_arm64 --all
```

The record lands at `tools/release/records/macos_arm64__<commit>__<stamp>.json`;
`python3 tools/release/release_record.py check <file>` re-validates it.
`--expect-skip GATE|TEST|REASON` must declare every test the run skips;
an unlisted skip keeps the closure open. If a gate set is run in pieces,
repeat `run --target macos_arm64 --gate <NAME>` per gate; the row is
recorded only when every named gate passed on that host.

Cross-reference for the Linux x86-64 row's per-leg detail and the known
fixture-migration failure families (expected to replay on macOS):
`wiki/drafts/rc_native_matrix_linux_x86_64.md`.

## Cross-target coverage already witnessed on Linux x86-64

Useful coverage, not a row substitute:

- The macos_arm64 backend emits Mach-O artifacts from Linux: the
  dependency-free benchmark subject `samples/cli/arithmetic/wrapping_square_sum`
  compiles and publishes on `macos_arm64`
  (`records/wrapping_square_sum__macos_arm64__default.json`, ~24.5 s
  compile, 16,640-byte image, `runtime_ms` skipped — recorded
  2026-09-20, per `wiki/drafts/benchmark_compile_only_rows.md`).
- The `omega-native-differential-test` crate exercises the macos_arm64
  pipeline cross-target on any host. Its host-agnostic legs run on
  Linux today but its Mach-O execution legs remain unobserved by
  definition here:
  - `hosted_receiver::hosted_receiver_bridge_binds_emits_and_replays_on_all_hosted_targets`
    compiles, binds, emits, and replays the `MacosArm64` profile —
    asserting Mach-O magic (`cf fa ed fe`) on the emitted image — while
    only its linux_x86_64 leg runs the emitted image as a real process
    (cfg-gated to the matching host).
  - `hosted_receiver_darwin_image_replay_rejects_mutated_bridge_bytes`
    pins the Darwin bridge's sixteen fixed A64 dyld entry words against
    substitution.
  - `ieee_comparisons`, `local_record_receivers`, and the
    `pipeline_ownership` callee-save legs parametrize
    `NativeTarget::macos_arm64()` / `FrameAbiPreservationConvention::DarwinAapcs64`
    cross-target.

## Substrate integrity note for the row's owner

`tools/release/release_record.py` `command_run` does not currently check
that the running host can satisfy the requested runner before marking the
row `recorded`: `run --target macos_arm64` executed on a Linux host would
write a record claiming the row, and `check` would accept it (host fields
are recorded but not validated against the runner). Until that guard
lands — owned by the RC-RELEASE-RECORD-SUBSTRATE lane — a macos_arm64
record's `host.os`/`host.machine` fields (expected `Darwin` / `arm64`)
are the only evidence that execution was direct, and reviewers must read
them explicitly.
